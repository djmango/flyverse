//! Closed loop: connectome activity drives a body in a room, and the room
//! feeds sensory neurons back into the connectome.
//!
//! Engineered pieces, not recovered from the connectome: the body integrator
//! (`body.rs`), the odour field (`room.rs`), the optic-flow and loom proxies,
//! and the choice of drive rates. What is real is the wiring: 166,700 neurons
//! and 24,469,412 signed synapses stepping at 0.1 ms, with motor and
//! descending read-outs taken only from named neuron groups.

use crate::body::{Body, Mode, Motors};
use crate::brain::Somas;
use crate::groups::{Drive, GroupRates, Groups};
use crate::httpd;
use crate::lif::Lif;
use crate::pack::Connectome;
use crate::room::{self, Room, V3};
use crate::vision::{self, Retina};
use anyhow::{Context, Result};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Neurons step every 0.1 ms; the body is integrated at 2 ms.
pub const WINDOW_STEPS: u32 = 20;
pub const WINDOW_S: f32 = WINDOW_STEPS as f32 * crate::lif::DT_MS / 1000.0;
/// How often a frame is pushed down each SSE stream.
pub const STREAM_MS: u64 = 40;
/// Roughly 40 s of wall-clock spikes at the measured firing rate.
const SPIKE_RING: usize = 4_000_000;
/// Most spikes a single `/api/spikes` response may carry. See `SpikeRing::since`.
const SPIKE_MAX: usize = 24_000;

/// The reference stimulus set: 6,370 `vnc_sensory` body IDs picked from the
/// published annotation table by `prep/prep_inputs.py`. This is the same
/// working point the NumPy reference engine was driven at.
///
/// Relative to the working directory, so a checkout works without editing
/// paths. Override with `FLYVERSE_VNC_TARGETS`.
pub const VNC_TARGETS: &str = "data/targets_vnc_sensory.u64";

/// Resolve the vnc_sensory target set: `$FLYVERSE_VNC_TARGETS`, else VNC_TARGETS.
pub fn vnc_targets_path() -> PathBuf {
    std::env::var("FLYVERSE_VNC_TARGETS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(VNC_TARGETS))
}

/// Motor read-outs are smoothed: a DLM/DVM pool is 12 neurons, so a 2 ms
/// window is all-or-nothing, while real muscle activation is continuous.
const MOTOR_TAU_S: f32 = 0.030;

fn read_u64s(path: &Path) -> Result<Vec<u64>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.len() % 8 != 0 {
        anyhow::bail!("{} is not a multiple of 8 bytes", path.display());
    }
    Ok(bytes
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect())
}

/// A population read-out: a named neuron group plus the per-neuron rate that
/// counts as "fully on" for it.
pub struct Out {
    gi: usize,
    full_hz: f32,
}

impl Out {
    fn new(g: &Groups, name: &str, full_hz: f32) -> Out {
        Out { gi: g.idx(name), full_hz }
    }
    pub fn norm(&self, r: &GroupRates) -> f32 {
        r.norm(self.gi, self.full_hz)
    }
    /// Population mean rate in Hz for this read-out.
    pub fn hz(&self, r: &GroupRates) -> f32 {
        *r.rate_hz.get(self.gi).unwrap_or(&0.0)
    }
}

/// The drive quantities the connectome actually sees in a given window, for
/// analysis and telemetry. Rates are per-neuron Hz.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Sensors {
    pub odor_l: f32,
    pub odor_r: f32,
    pub flow_l: f32,
    pub flow_r: f32,
    pub loom: f32,
    pub taste_hz: f32,
    pub vnc_hz: f32,
}

pub struct World {
    pub conn: Arc<Connectome>,
    pub lif: Lif,
    pub groups: Groups,
    pub rates: GroupRates,
    pub somas: Option<Somas>,
    pub room: Room,
    pub body: Body,
    pub food: V3,
    pub step: u64,
    /// Size of the reference vnc_sensory stimulus set.
    pub vnc_targets: usize,
    /// Number of mechanosensory groups merged in from the inventory.
    pub mech_groups: usize,
    /// Whether the haltere afferents are wired to the body. Off is the
    /// control condition for asking whether the connectome does anything with
    /// the rotation signal it is given.
    pub haltere_on: bool,
    /// Whether the food-odour channel is delivered. FLYVERSE_NO_ODOR=1 silences
    /// it. This is the control for the question "is the fly's endpoint the fixed
    /// food source": the odour drives are the ONLY path from `food` into the
    /// fly's motion (the body integrator is handed `food` and never reads it),
    /// so silencing them removes the fixed attractor and nothing else. With the
    /// flag unset the gate multiplies by exactly 1.0, which is bit-identical, so
    /// a normal run is unchanged.
    pub odor_on: bool,
    /// Whether the optic-flow channel is delivered. FLYVERSE_NO_FLOW=1 silences
    /// it. Unlike the haltere channel this one is an engineered proxy, and it
    /// responds to rotation immediately through its turn term, so silencing it
    /// is how the perturbation probe tells a connectome response apart from a
    /// response to the surrogate.
    pub flow_on: bool,
    /// The retinotopic visual front end: one ray per optic lobe column, driving
    /// that column's photoreceptors with the light it finds. FLYVERSE_NO_RETINA=1
    /// falls back to the optic-flow proxy alone.
    pub retina: Retina,
    /// Model indices that fired during the last control window.
    pub window_spikes: Vec<u32>,

    // Sensory drive channels; rates are per-neuron Hz.
    d_vnc: Drive,
    d_olf_l: Drive,
    d_olf_r: Drive,
    d_taste: Drive,
    d_vis_l: Drive,
    d_vis_r: Drive,
    d_loom_l: Drive,
    d_loom_r: Drive,
    d_hal_l: Drive,
    d_hal_r: Drive,
    d_legtac_l: Drive,
    d_legtac_r: Drive,

    // Motor and descending read-outs.
    o_pow_l: Out,
    o_pow_r: Out,
    o_steer_l: Out,
    o_steer_r: Out,
    o_walk_l: Out,
    o_walk_r: Out,
    o_land_l: Out,
    o_land_r: Out,
    pub o_mn9: Out,
    pub o_dn02: Out,
    pub o_dn07: Out,
    pub o_sapp: Out,
    pub o_land_dn: Out,

    /// Leaky integrators: the connectome is instantaneous, the body is not.
    dn_filt: f32,
    land_filt: f32,
    /// Smoothed motor read-outs, in the order of `Motors`.
    sm: [f32; 9],

    // Sensor samples, for the HUD.
    odor_l: f32,
    odor_r: f32,
    flow_l: f32,
    flow_r: f32,

    prev_yaw: f32,
    yaw_rate: f32,
    stim: Vec<(u32, u32)>,
}

impl World {
    pub fn new(pack: &Path, seed: u64, somas: Option<Somas>) -> Result<World> {
        let conn = Arc::new(Connectome::load(pack)?);
        let mut groups = Groups::load(&conn, &Groups::io_path())?;
        // Join the mechanosensory inventory so the body can be felt as well as
        // seen and smelled. Missing file is a hard error: a silently absent
        // sense organ would look exactly like a fly that cannot feel rotation.
        let mech = groups.merge_file(&conn, &Groups::mechano_path())?;
        eprintln!(
            "[flyverse] body mechanosensation: merged {mech} groups from {}",
            Groups::mechano_path().display()
        );
        let lif = Lif::new(&conn);
        let rates = GroupRates::new(groups.names.len(), WINDOW_STEPS);

        let mk = |name: &str, hz: f64, tag: u64| -> Drive {
            Drive::new(
                groups.get(name).to_vec(),
                hz,
                seed ^ tag.wrapping_mul(0x9E37_79B9_7F4A_7C15),
            )
        };

        // 150 Hz on the reference vnc_sensory set reproduces the working point
        // the NumPy engine was run at. Every other channel sits at 0 Hz until
        // the room says otherwise.
        let vnc_ids = {
            let mut ids = read_u64s(&vnc_targets_path())
                .with_context(|| "the vnc_sensory target set; run prep/prep_inputs.py")?;
            ids.sort_unstable();
            ids.dedup();
            conn.indices_of_sorted(&ids)
        };
        if vnc_ids.is_empty() {
            anyhow::bail!("vnc_sensory target set matched no neurons in the pack");
        }
        let d_vnc = Drive::new(vnc_ids, 150.0, seed ^ 0x51ED_2701);
        let vnc_targets = d_vnc.targets.len();
        let d_olf_l = mk("olfaction_left", 0.0, 2);
        let d_olf_r = mk("olfaction_right", 0.0, 3);
        let d_taste = mk("taste_sugar", 0.0, 4);
        let d_vis_l = mk("visual_motion_left", 0.0, 5);
        let d_vis_r = mk("visual_motion_right", 0.0, 6);
        let d_loom_l = mk("visual_loom_left", 0.0, 7);
        let d_loom_r = mk("visual_loom_right", 0.0, 8);

        // Body mechanosensation. These are driven from the body's own state
        // every window (see `sense`), so the connectome can tell that it is
        // rotating and whether its feet are loaded. Before this, the fly had
        // no way to know it was moving at all except through the eyes.
        let d_hal_l = mk("haltere_proprioceptive_L", 0.0, 9);
        let d_hal_r = mk("haltere_proprioceptive_R", 0.0, 10);
        let d_legtac_l = mk("leg_tactile_L", 0.0, 11);
        let d_legtac_r = mk("leg_tactile_R", 0.0, 12);

        let room = room::ROOM;
        let food = room.food_home();
        let mut body = Body::new();
        body.reset();
        body.pos = [-220.0, -150.0, 0.0];
        body.set_yaw(0.6);

        // The visual front end. Missing asset is a hard error for the same
        // reason a missing sense organ is: a retina that silently sees nothing
        // looks exactly like a connectome that cannot see.
        let retina = Retina::load(&conn, &Retina::asset_path(), seed)?;
        eprintln!(
            "[flyverse] retina: {} columns, {} photoreceptors, {} ({}..{} Hz)",
            retina.columns(),
            retina.photons,
            if retina.on { "sampling the room" } else { "silenced by FLYVERSE_NO_RETINA" },
            vision::BASE_HZ as u32,
            (vision::BASE_HZ + vision::GAIN_HZ) as u32,
        );

        Ok(World {
            o_pow_l: Out::new(&groups, "motor_flight_power_left", 90.0),
            o_pow_r: Out::new(&groups, "motor_flight_power_right", 90.0),
            o_steer_l: Out::new(&groups, "motor_flight_steering_left", 110.0),
            o_steer_r: Out::new(&groups, "motor_flight_steering_right", 110.0),
            o_walk_l: Out::new(&groups, "motor_walking_left", 90.0),
            o_walk_r: Out::new(&groups, "motor_walking_right", 90.0),
            o_land_l: Out::new(&groups, "motor_landing_left", 90.0),
            o_land_r: Out::new(&groups, "motor_landing_right", 90.0),
            o_mn9: Out::new(&groups, "feeding_mn9", 90.0),
            o_dn02: Out::new(&groups, "flight_dng02_left", 70.0),
            o_dn07: Out::new(&groups, "flight_dng07_left", 70.0),
            o_sapp: Out::new(&groups, "flight_state_sapp_left", 70.0),
            o_land_dn: Out::new(&groups, "landing_dn_left", 70.0),
            d_vnc,
            d_olf_l,
            d_olf_r,
            d_taste,
            d_vis_l,
            d_vis_r,
            d_loom_l,
            d_loom_r,
            d_hal_l,
            d_hal_r,
            d_legtac_l,
            d_legtac_r,
            dn_filt: 0.0,
            land_filt: 0.0,
            sm: [0.0; 9],
            odor_l: 0.0,
            odor_r: 0.0,
            flow_l: 0.0,
            flow_r: 0.0,
            prev_yaw: body.yaw,
            yaw_rate: 0.0,
            stim: Vec::with_capacity(512),
            window_spikes: Vec::with_capacity(4096),
            vnc_targets,
            mech_groups: mech,
            haltere_on: std::env::var("FLYVERSE_NO_HALTERE").is_err(),
            odor_on: std::env::var("FLYVERSE_NO_ODOR").is_err(),
            flow_on: std::env::var("FLYVERSE_NO_FLOW").is_err(),
            retina,
            conn,
            lif,
            groups,
            rates,
            somas,
            room,
            body,
            food,
            step: 0,
        })
    }

    /// Impose an angular velocity on the body. This is the perturbation probe's
    /// hammer: the body is given a rotation it did not generate, and the question
    /// is whether the network drives the wings to oppose it. It changes only the
    /// body's state, never the stimulus, so any motor response has to travel
    /// through the connectome.
    pub fn inject_rotation(&mut self, axis: usize, magnitude: f32) {
        if axis < 3 {
            self.body.omega[axis] += magnitude;
        }
    }

    /// Haltere afferent rates the network is currently being driven with, in Hz.
    /// Read back so a probe can confirm the perturbation actually reached the
    /// sense organ rather than being silently clipped.
    pub fn haltere_rates(&self) -> (f32, f32) {
        (self.body.haltere_l, self.body.haltere_r)
    }

    /// Advance one 2 ms control window: sense, step the connectome, read out,
    /// integrate the body.
    pub fn advance(&mut self) {
        self.sense();

        self.window_spikes.clear();
        let mut s = std::mem::take(&mut self.stim);
        for _ in 0..WINDOW_STEPS {
            s.clear();
            s.extend_from_slice(self.d_vnc.events());
            s.extend_from_slice(self.d_olf_l.events());
            s.extend_from_slice(self.d_olf_r.events());
            s.extend_from_slice(self.d_taste.events());
            s.extend_from_slice(self.d_vis_l.events());
            s.extend_from_slice(self.d_vis_r.events());
            s.extend_from_slice(self.d_loom_l.events());
            s.extend_from_slice(self.d_loom_r.events());
            s.extend_from_slice(self.d_hal_l.events());
            s.extend_from_slice(self.d_hal_r.events());
            s.extend_from_slice(self.d_legtac_l.events());
            s.extend_from_slice(self.d_legtac_r.events());
            // The retina's drives are per column, so they are pushed as a block
            // rather than one line each.
            self.retina.push_events(&mut s);
            self.lif.step(&self.conn, &s);
            self.window_spikes.extend_from_slice(&self.lif.fired);
            self.retina.observe(&self.lif.fired);
            for &i in &self.lif.fired {
                let g = self.groups.neuron_group[i as usize];
                self.rates.observe(g);
            }
        }
        self.stim = s;
        self.rates.commit(&self.groups);
        self.smooth_motors();
        self.step += WINDOW_STEPS as u64;

        // Descending commands, filtered: the body has mass, the DNs do not.
        let a_dn = 1.0 - (-WINDOW_S / 0.060).exp();
        let a_land = 1.0 - (-WINDOW_S / 0.120).exp();
        let climb = self.o_dn02.norm(&self.rates);
        let sink = self.o_dn07.norm(&self.rates);
        // The DLM/DVM wing-power motor pool is direct evidence that the flight
        // motor program is running, and DNg02 is its annotated descending
        // activator. Both feed the climb command; DNg07 subtracts.
        let power_mn = 0.5 * (self.sm[0] + self.sm[1]);
        let alt = (climb + 0.6 * power_mn - sink * 0.5 - 0.15).clamp(-1.0, 1.0);
        self.dn_filt += (alt - self.dn_filt) * a_dn;
        let land = self.o_land_dn.norm(&self.rates);
        self.land_filt += (land - self.land_filt) * a_land;

        let m = self.motors();
        let dn = self.dn_filt;
        let lf = self.land_filt;
        let (room, food) = (self.room, self.food);
        self.body.update(&room, food, &m, dn, lf, WINDOW_S);

        // Yaw rate from the body's angular velocity, not from a differenced
        // Euler angle: once the body tumbles the Euler chart wraps and the
        // quotient invents rates that the body never had.
        self.yaw_rate = self.body.yaw_rate_world();
        self.prev_yaw = self.body.yaw;
    }

    /// Sample the room at the antennae and set the sensory drive rates.
    fn sense(&mut self) {
        let head = self.body.head();
        let hd = self.body.heading();
        // Ground-plane lateral axis, so the antennae straddle the body axis and
        // see a real left/right odour difference as the fly turns.
        let lat: V3 = [-hd[1], hd[0], 0.0];
        let a = 0.9;
        let pl = [head[0] + lat[0] * a, head[1] + lat[1] * a, head[2]];
        let pr = [head[0] - lat[0] * a, head[1] - lat[1] * a, head[2]];
        let og = if self.odor_on { 1.0f32 } else { 0.0f32 };
        self.odor_l = self.room.odor(pl, self.food) * og;
        self.odor_r = self.room.odor(pr, self.food) * og;
        // Antennal olfactory receptor neurons run up to about 120 Hz on strong
        // odour; below that the rate is proportional to concentration.
        self.d_olf_l.rate_hz = self.odor_l as f64 * 120.0;
        self.d_olf_r.rate_hz = self.odor_r as f64 * 120.0;

        // Taste: only while the mouthparts are on the food cube.
        let d = room::len(room::sub(head, self.food));
        let on_food = d < 3.5 && matches!(self.body.mode, Mode::Feeding);
        self.d_taste.rate_hz = if on_food { 160.0 } else { 0.0 };

        // Optic flow: an engineered proxy for retinal slip, translation plus
        // rotation, scaled so a fast cruise saturates the channel. This is now
        // only a fallback. While the retina is on it drives the photoreceptors
        // and the connectome has to derive motion for itself, so leaving this
        // running as well would hand the lobula plate the answer it is being
        // asked to compute and mask whatever the retina contributed.
        let sp = self.body.speed();
        let turn = (self.yaw_rate * 26.0).clamp(-1.0, 1.0);
        let fwd = (sp / 300.0).clamp(0.0, 1.0);
        let raw_l = (0.5 * fwd + 0.5 * turn.max(0.0)).clamp(0.0, 1.0);
        let raw_r = (0.5 * fwd + 0.5 * (-turn).max(0.0)).clamp(0.0, 1.0);
        // Gated by the same `vg` that scales the channel below, so the stored
        // proxy is what was ACTUALLY delivered, not the phantom that would have
        // been delivered if the retina were off. When the retina is on (the
        // default) the proxy is not delivered at all and these store zero. This
        // mirrors the probe's `optic_flow_proxy_delivered` column: recording the
        // ungated value would archive a visual drive that never reached the brain.
        let vg = if self.flow_on && !self.retina.on { 1.0 } else { 0.0 };
        self.flow_l = raw_l * vg;
        self.flow_r = raw_r * vg;
        self.d_vis_l.rate_hz = self.flow_l as f64 * 90.0;
        self.d_vis_r.rate_hz = self.flow_r as f64 * 90.0;

        // The retina proper: one ray per optic lobe column, from the position
        // and attitude the body actually has this window.
        self.retina.update(self.body.pos, self.body.quat(), &self.room);

        // Loom: how fast the nearest wall ahead is filling the field of view.
        let loom = self.loom();
        self.d_loom_l.rate_hz = loom as f64 * 60.0;
        self.d_loom_r.rate_hz = loom as f64 * 60.0;

        // Haltere afferents: the only route by which the connectome can feel
        // that the body is rotating. Driven from the body's own angular
        // velocity through the Coriolis model, one window behind the rotation
        // it reports, because a sense organ cannot lead the body it senses.
        // FLYVERSE_NO_HALTERE=1 is the control condition: the body still
        // rotates and the haltere model still computes what the afferents
        // would say, but the spikes never reach the network. Comparing the two
        // runs is how we find out whether the connectome does anything at all
        // with the rotation signal.
        let hg = if self.haltere_on { 1.0 } else { 0.0 };
        self.d_hal_l.rate_hz = self.body.haltere_l as f64 * hg;
        self.d_hal_r.rate_hz = self.body.haltere_r as f64 * hg;

        // Leg tactile bristles report the load the legs are carrying: the
        // weight the wings are not holding up. Zero in the air.
        let grounded = matches!(self.body.mode, Mode::Ground | Mode::Feeding);
        let load = if grounded {
            let w = self.body.weight();
            if w > 0.0 {
                ((w - self.body.wing_lift()) / w).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        };
        self.d_legtac_l.rate_hz = load as f64 * 120.0;
        self.d_legtac_r.rate_hz = load as f64 * 120.0;
    }

    fn loom(&self) -> f32 {
        let p = self.body.pos;
        let hd = self.body.heading();
        let mut best = 1e9f32;
        for a in 0..3 {
            let (lo, hi) = match a {
                0 => (self.room.x[0], self.room.x[1]),
                1 => (self.room.y[0], self.room.y[1]),
                _ => (self.room.z[0], self.room.z[1]),
            };
            let dir = hd[a];
            if dir > 1e-3 {
                best = best.min((hi - p[a]) / dir);
            } else if dir < -1e-3 {
                best = best.min((lo - p[a]) / dir);
            }
        }
        let ttc = best.max(0.0) / self.body.speed().max(20.0);
        (1.0 - (ttc / 0.6).clamp(0.0, 1.0)).clamp(0.0, 1.0)
    }

    /// Current sensory drive snapshot. This is the entire interface between the
    /// room and the connectome: if a behaviour is not driven by one of these
    /// channels, the brain cannot know about it.
    pub fn sensors(&self) -> Sensors {
        Sensors {
            odor_l: self.odor_l,
            odor_r: self.odor_r,
            flow_l: self.flow_l,
            flow_r: self.flow_r,
            loom: self.loom(),
            taste_hz: self.d_taste.rate_hz as f32,
            vnc_hz: 150.0,
        }
    }

    /// Filtered descending climb command, for the HUD and the probe.
    pub fn dn_filter(&self) -> f32 {
        self.dn_filt
    }

    /// Body turn rate over the last control window, rad/s.
    pub fn yaw_rate_rad_s(&self) -> f32 {
        self.yaw_rate
    }

    pub fn motors(&self) -> Motors {
        Motors {
            flight_power_l: self.sm[0],
            flight_power_r: self.sm[1],
            flight_steer_l: self.sm[2],
            flight_steer_r: self.sm[3],
            walk_l: self.sm[4],
            walk_r: self.sm[5],
            land_l: self.sm[6],
            land_r: self.sm[7],
            mn9: self.sm[8],
        }
    }

    /// Instantaneous motor read-out, before smoothing.
    pub fn raw_motors(&self) -> Motors {
        let r = &self.rates;
        Motors {
            flight_power_l: self.o_pow_l.norm(r),
            flight_power_r: self.o_pow_r.norm(r),
            flight_steer_l: self.o_steer_l.norm(r),
            flight_steer_r: self.o_steer_r.norm(r),
            walk_l: self.o_walk_l.norm(r),
            walk_r: self.o_walk_r.norm(r),
            land_l: self.o_land_l.norm(r),
            land_r: self.o_land_r.norm(r),
            mn9: self.o_mn9.norm(r),
        }
    }

    fn smooth_motors(&mut self) {
        let raw = self.raw_motors();
        let a = 1.0 - (-WINDOW_S / MOTOR_TAU_S).exp();
        let v = [
            raw.flight_power_l,
            raw.flight_power_r,
            raw.flight_steer_l,
            raw.flight_steer_r,
            raw.walk_l,
            raw.walk_r,
            raw.land_l,
            raw.land_r,
            raw.mn9,
        ];
        for i in 0..9 {
            self.sm[i] += (v[i] - self.sm[i]) * a;
        }
    }

    /// 12 region bars for the HUD: mean normalised rate per region bucket.
    pub fn region_bars(&self) -> Vec<f32> {
        let mut num = [0f32; 12];
        let mut den = [0f32; 12];
        for gi in 0..self.groups.names.len() {
            let reg = self.groups.group_region[gi] as usize;
            if reg >= 12 {
                continue;
            }
            num[reg] += self.rates.norm(gi, 80.0);
            den[reg] += 1.0;
        }
        (0..12)
            .map(|i| if den[i] > 0.0 { num[i] / den[i] } else { 0.0 })
            .collect()
    }

    fn airborne(&self) -> bool {
        matches!(self.body.mode, Mode::Takeoff | Mode::Cruise | Mode::Landing)
    }

    /// The frame the browser renders. Field names are the front-end contract.
    pub fn frame_json(&self, rt: f64, paused: bool) -> String {
        let b = &self.body;
        let m = self.motors();
        let head = b.head();
        let d = room::len(room::sub(head, self.food));
        let sim_s = self.step as f64 * crate::lif::DT_MS as f64 / 1000.0;
        serde_json::json!({
            "seq": self.step / WINDOW_STEPS as u64,
            "sim_ms": sim_s * 1000.0,
            "wall_rt": rt,
            "paused": paused,
            "stimulus": "vnc_sensory 150 Hz + odour/flow/loom from the room",
            "body": {
                "pos": b.pos,
                "quat": b.quat(),
                "mode": b.mode.as_str(),
                "airborne": self.airborne(),
                "speed": b.speed(),
                "pitch": b.pitch,
                "roll": b.roll,
                "yaw": b.yaw,
                "wing_amp": b.wing_amp,
                "legs_supported": b.legs_supported,
                // Visual stroke/gait phases. These advance at WINGBEAT_VISUAL_HZ
                // (19 Hz), NOT the real 200 Hz wingbeat: the real rate aliases at
                // display frame rates. The client resyncs its animation to them.
                "wing_phase": b.wing_phase,
                "gait_phase": b.gait_phase,
            },
            "food": { "pos": self.food, "dist": d },
            "motors": {
                "flight_power_l": m.flight_power_l,
                "flight_power_r": m.flight_power_r,
                "flight_steer_l": m.flight_steer_l,
                "flight_steer_r": m.flight_steer_r,
                "walk_l": m.walk_l,
                "walk_r": m.walk_r,
                "land_l": m.land_l,
                "land_r": m.land_r,
                "mn9": m.mn9,
            },
            "neural": {
                "spikes_window": self.window_spikes.len(),
                "spikes_total": self.lif.total_spikes,
                "active_neurons": self.lif.active_neurons,
                "mean_rate_hz": self.rates.rate_hz.iter().sum::<f32>() as f64 / self.conn.n as f64,
                "takeoff_drive": self.dn_filt,
                "hall": self.o_sapp.norm(&self.rates),
                "odor_l": self.odor_l,
                "odor_r": self.odor_r,
                // The optic-flow proxy drive ACTUALLY delivered to the visual
                // channels (see `step`): zero whenever the retina is on, because
                // the retina, not the proxy, is then driving the connectome.
                // Named for what it is; the old `retina_l`/`retina_r` keys were a
                // misnomer for this proxy and no retina quantity was ever sent.
                "flow_l": self.flow_l,
                "flow_r": self.flow_r,
                "regions": self.region_bars(),
            },
            "events": {
                "eats": b.eats,
                "takeoffs": b.takeoffs,
                "landings": b.landings,
                "wall_hits": b.wall_hits,
            },
        })
        .to_string()
    }
}

#[inline]
fn wrap_pi(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let mut x = a % t;
    if x > std::f32::consts::PI {
        x -= t;
    } else if x < -std::f32::consts::PI {
        x += t;
    }
    x
}

// ---------------------------------------------------------------- sharing

struct SpikeRing {
    buf: Vec<u32>,
    base: u64,
    total: u64,
}

impl SpikeRing {
    fn new() -> SpikeRing {
        SpikeRing { buf: Vec::with_capacity(1 << 20), base: 0, total: 0 }
    }
    fn push(&mut self, idx: u32) {
        self.buf.push(idx);
        self.total += 1;
        if self.buf.len() > SPIKE_RING {
            let drop = self.buf.len() - SPIKE_RING / 2;
            self.buf.copy_within(drop.., 0);
            let len = self.buf.len() - drop;
            self.buf.truncate(len);
            self.base += drop as u64;
        }
    }
    /// Spikes with sequence number >= `from`, as little-endian u32 bytes, and
    /// the sequence number just past the end.
    ///
    /// Capped at `SPIKE_MAX` per response. The network emits ~2.8M spikes/s;
    /// shipping all of them to a browser at 8 Hz is ~6 MB/s of JSON plus a
    /// char-by-char base64 decode, which starves the page's main thread (the
    /// render loop stops getting turns). The brain panel only needs "which
    /// somata fired lately", so a caller that has fallen behind gets the most
    /// recent `SPIKE_MAX` and `to` = the current end; the skipped spikes are
    /// never re-sent.
    fn since(&self, from: u64) -> (u64, Vec<u8>) {
        let end = self.base + self.buf.len() as u64;
        let mut start = from.saturating_sub(self.base);
        if start >= self.buf.len() as u64 {
            return (end, Vec::new());
        }
        let behind = self.buf.len() as u64 - start;
        if behind > SPIKE_MAX as u64 {
            start = self.buf.len() as u64 - SPIKE_MAX as u64;
        }
        let slice = &self.buf[start as usize..];
        let mut out = Vec::with_capacity(slice.len() * 4);
        for &v in slice {
            out.extend_from_slice(&v.to_le_bytes());
        }
        (end, out)
    }
}

struct Shared {
    frame: Mutex<Arc<String>>,
    spikes: Mutex<SpikeRing>,
    paused: AtomicBool,
    reset: AtomicBool,
}

impl Shared {
    fn new() -> Shared {
        Shared {
            frame: Mutex::new(Arc::new("{}".to_string())),
            spikes: Mutex::new(SpikeRing::new()),
            paused: AtomicBool::new(false),
            reset: AtomicBool::new(false),
        }
    }
}

pub fn serve(
    pack: PathBuf,
    port: u16,
    seconds: f64,
    _rate: f64,
    seed: u64,
    rt: Option<f64>,
) -> Result<()> {
    let n = Connectome::load(&pack)?.n;
    let somas = Somas::load(&Somas::path(), n).ok();
    if somas.is_none() {
        eprintln!("[flyverse] warning: no soma positions, brain panel will be empty");
    }
    let mut world = World::new(&pack, seed, somas)?;
    let n = world.conn.n;
    let m = world.conn.m;
    let shared = Arc::new(Shared::new());

    let cap = if seconds > 0.0 { Some(seconds) } else { None };
    let sim = Arc::clone(&shared);
    let sim_thread = std::thread::spawn(move || {
        let t0 = Instant::now();
        let mut sim_s = 0.0f64;
        let mut last_push = Instant::now();
        let mut rt_ema = 0.0f64;
        loop {
            if sim.reset.swap(false, Ordering::Relaxed) {
                world.lif.reset();
                world.body.reset();
                world.body.pos = [-220.0, -150.0, 0.0];
                world.body.yaw = 0.6;
                world.prev_yaw = 0.6;
                world.dn_filt = 0.0;
                world.land_filt = 0.0;
                world.sm = [0.0; 9];
                world.step = 0;
                sim_s = 0.0;
                let mut sp = sim.spikes.lock().unwrap();
                sp.buf.clear();
                sp.base = sp.total;
            }
            if sim.paused.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(15));
                continue;
            }
            let before = Instant::now();
            world.advance();
            sim_s += WINDOW_S as f64;
            {
                let mut sp = sim.spikes.lock().unwrap();
                for &i in &world.window_spikes {
                    sp.push(i);
                }
            }
            let el = before.elapsed().as_secs_f64().max(1e-9);
            let inst = WINDOW_S as f64 / el;
            rt_ema = if rt_ema == 0.0 { inst } else { rt_ema * 0.9 + inst * 0.1 };

            if last_push.elapsed().as_millis() as u64 >= STREAM_MS {
                last_push = Instant::now();
                let f = world.frame_json(rt_ema, sim.paused.load(Ordering::Relaxed));
                *sim.frame.lock().unwrap() = Arc::new(f);
            }
            if let Some(c) = cap {
                if sim_s >= c {
                    break;
                }
            }
            if let Some(r) = rt {
                let target = sim_s / r;
                let spent = t0.elapsed().as_secs_f64();
                if spent < target {
                    std::thread::sleep(Duration::from_secs_f64(target - spent));
                }
            }
        }
        eprintln!("[flyverse] sim loop finished after {sim_s:.1}s of model time");
    });

    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("bind 127.0.0.1:{port}"))?;
    let root = httpd::static_root();
    let shutdown = Arc::new(AtomicBool::new(false));
    eprintln!("[flyverse] {n} neurons, {m} edges; http on http://127.0.0.1:{port}");

    let s2 = Arc::clone(&shared);
    httpd::serve(listener, shutdown, move |stream, req| {
        route(stream, req, &s2, &root, n, m)
    })?;
    let _ = sim_thread;
    Ok(())
}

fn route(
    stream: &mut std::net::TcpStream,
    req: httpd::Request,
    sim: &Shared,
    root: &Path,
    n: usize,
    m: usize,
) -> Result<()> {
    match req.path.as_str() {
        "/api/health" => {
            let body = serde_json::json!({
                "ok": true,
                "neurons": n,
                "edges": m,
                "paused": sim.paused.load(Ordering::Relaxed),
                "spikes_total": sim.spikes.lock().unwrap().total,
            });
            httpd::json_response(stream, &body.to_string())
        }
        "/api/brain/positions" => {
            let bytes = std::fs::read(Somas::path())?;
            httpd::write_response(
                stream,
                200,
                "OK",
                "application/octet-stream",
                &bytes,
                &[("Cache-Control", "public, max-age=300")],
            )
        }
        "/api/spikes" => {
            let from: u64 = req.param("from").and_then(|v| v.parse().ok()).unwrap_or(0);
            let (to, bytes) = sim.spikes.lock().unwrap().since(from);
            let body = serde_json::json!({
                "from": from,
                "to": to,
                "dt": httpd::base64(&bytes),
            });
            httpd::json_response(stream, &body.to_string())
        }
        "/api/control" => {
            let cmd = req.param("cmd").unwrap_or_default();
            match cmd.as_str() {
                "toggle_pause" => {
                    let p = sim.paused.load(Ordering::Relaxed);
                    sim.paused.store(!p, Ordering::Relaxed);
                }
                "reset" => sim.reset.store(true, Ordering::Relaxed),
                _ => {}
            }
            let body = serde_json::json!({
                "cmd": cmd,
                "paused": sim.paused.load(Ordering::Relaxed),
            });
            httpd::json_response(stream, &body.to_string())
        }
        "/api/stream" => {
            httpd::open_sse(stream)?;
            httpd::sse_event(stream, "hello", &format!("{{\"neurons\":{n},\"edges\":{m}}}"))?;
            let mut last: Option<Arc<String>> = None;
            loop {
                std::thread::sleep(Duration::from_millis(20));
                let cur = Arc::clone(&*sim.frame.lock().unwrap());
                if last.as_ref().map(|l| Arc::ptr_eq(l, &cur)).unwrap_or(false) {
                    continue;
                }
                if httpd::sse_event(stream, "frame", &cur).is_err() {
                    break;
                }
                last = Some(cur);
            }
            Ok(())
        }
        _ => httpd::serve_static(stream, root, &req.path),
    }
}