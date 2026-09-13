//! Trace sample type, run options, and the trace-slicing helpers.

use crate::body::Mode;
use crate::lif::DT_MS;
use crate::sim::World;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Trace

#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct Sample {
    pub t: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub speed: f32,
    pub yaw: f32,
    pub yaw_rate: f32,
    pub roll: f32,
    pub pitch: f32,
    pub wing_amp: f32,
    pub mode: u8,
    /// Smoothed motor read-outs, 0..1 normalised.
    pub pow_l: f32,
    pub pow_r: f32,
    pub steer_l: f32,
    pub steer_r: f32,
    pub walk_l: f32,
    pub walk_r: f32,
    pub land_l: f32,
    pub land_r: f32,
    pub mn9: f32,
    /// Descending read-outs, 0..1 normalised.
    pub dn02: f32,
    pub dn07: f32,
    pub sapp: f32,
    pub land_dn: f32,
    pub dn_filt: f32,
    /// Sensory drive actually presented to the connectome.
    pub odor_l: f32,
    pub odor_r: f32,
    pub flow_l: f32,
    pub flow_r: f32,
    pub loom: f32,
    pub taste: f32,
    /// Horizontal distance to the nearest wall, mm.
    pub wall_dist: f32,
    /// Rotational state, instrumented the way the pitch investigation needed
    /// it. `tau_aero`/`tau_damp` are the aerodynamic and damping torques in
    /// the body frame; `wroll`/`wpitch`/`wyaw` are the body-frame angular
    /// rates, so a turning rate can be attributed to a torque rather than
    /// guessed at from the Euler angles alone.
    pub tau_aero: [f32; 3],
    pub tau_damp: [f32; 3],
    /// The yaw moment split into the component the stroke-plane tilt
    /// differential commands and the component the wing-amplitude differential
    /// commands. Both are exact, by the identity in `yaw_torque_split`.
    pub tau_yaw_tilt: f32,
    pub tau_yaw_amp: f32,
    pub wroll: f32,
    pub wpitch: f32,
    pub wyaw: f32,
    /// Stroke-plane tilt per wing, radians, and the body's tilt from level.
    pub tilt_l: f32,
    pub tilt_r: f32,
    pub tilt_deg: f32,
    /// Vertical wing force in the body and world frames, mg*mm/s^2.
    pub fz_body: f32,
    pub fz_world: f32,
    /// Raw (unsmoothed) per-neuron rate of the b1/b2/b3 steering pools, Hz.
    /// The smoothed 0..1 readouts in `steer_l`/`steer_r` are clamped by their
    /// normalising divisor, so the raw rates are the only place a saturated
    /// steering read-out is visible.
    pub steer_l_hz: f32,
    pub steer_r_hz: f32,
    /// Wall/ceiling contact latch this window: [x, y, z].
    pub touch: [bool; 3],
    pub wall_hits: u32,
    pub takeoffs: u32,
    pub landings: u32,
    pub eats: u32,
    pub win_spikes: u32,
    pub tot_spikes: u64,
}

pub struct Options {
    pub seconds: f64,
    pub seed: u64,
    /// Keep one sample every N control windows.
    pub sample_every: u64,
    pub out: PathBuf,
}

/// Split the yaw moment `WING_DY * (fr[0] - fl[0])` into the part carried by
/// the stroke-plane tilt difference and the part carried by the wing-amplitude
/// difference.
///
/// The two wings' force magnitudes are `f_l`, `f_r` and their stroke planes
/// are tilted by `t_l`, `t_r`, so the forward forces are `f_l sin t_l` and
/// `f_r sin t_r`. With `fm = (f_l + f_r)/2` and `fd = f_r - f_l`,
///
///   f_r sin t_r - f_l sin t_l = fm (sin t_r - sin t_l)
///                             + (fd/2) (sin t_r + sin t_l),
///
/// which is exact, not a linearisation. The first term is the steering the
/// b1/b2/b3 pools command through the stroke plane; the second is the yaw that
/// falls out of a left/right wing-power difference. They are different causes
/// and want to be read separately.
pub(crate) fn yaw_torque_split(b: &crate::body::Body) -> (f32, f32) {
    let airspeed = b.airspeed();
    let fl = crate::wing::wing_force_vector(b.wing_amp_l, crate::wing::WINGBEAT_HZ, airspeed, b.tilt_l);
    let fr = crate::wing::wing_force_vector(b.wing_amp_r, crate::wing::WINGBEAT_HZ, airspeed, b.tilt_r);
    // The force magnitude is independent of the tilt (the tilt only rotates the
    // resultant), so recover it from the vertical component, which is
    // `f cos tilt`. Tilt is bounded by STEER_TILT_MAX = 0.52 rad, where cos is
    // 0.87, so this never divides by anything small.
    let f_l = fl[2] / b.tilt_l.cos();
    let f_r = fr[2] / b.tilt_r.cos();
    let (fm, fd) = (0.5 * (f_l + f_r), f_r - f_l);
    let (sl, sr) = (b.tilt_l.sin(), b.tilt_r.sin());
    let dy = crate::wing::WING_DY;
    (dy * fm * (sr - sl), dy * 0.5 * fd * (sr + sl))
}

/// Mean per-neuron firing rate of a named group in the last control window, Hz.
/// Returns 0 for a group that is absent from the annotation, so a missing pool
/// shows up as a zero rate rather than a panic.
pub(crate) fn group_hz(w: &World, name: &str) -> f32 {
    let gi = w.groups.idx(name);
    if gi == usize::MAX {
        return 0.0;
    }
    *w.rates.rate_hz.get(gi).unwrap_or(&0.0)
}

pub(crate) fn mode_code(m: Mode) -> u8 {
    match m {
        Mode::Ground => 0,
        Mode::Takeoff => 1,
        Mode::Cruise => 2,
        Mode::Landing => 3,
        Mode::Feeding => 4,
    }
}

pub(crate) fn sample_of(w: &World) -> Sample {
    let b = &w.body;
    let m = w.motors();
    let r = &w.rates;
    let s = w.sensors();
    let p = b.pos;
    let wall_dist = (p[0] - w.room.x[0])
        .min(w.room.x[1] - p[0])
        .min(p[1] - w.room.y[0])
        .min(w.room.y[1] - p[1]);
    Sample {
        t: w.step as f32 * DT_MS / 1000.0,
        x: p[0],
        y: p[1],
        z: p[2],
        speed: b.speed(),
        yaw: b.yaw,
        yaw_rate: w.yaw_rate_rad_s(),
        roll: b.roll,
        pitch: b.pitch,
        wing_amp: b.wing_amp,
        mode: mode_code(b.mode),
        pow_l: m.flight_power_l,
        pow_r: m.flight_power_r,
        steer_l: m.flight_steer_l,
        steer_r: m.flight_steer_r,
        walk_l: m.walk_l,
        walk_r: m.walk_r,
        land_l: m.land_l,
        land_r: m.land_r,
        mn9: m.mn9,
        dn02: w.o_dn02.norm(r),
        dn07: w.o_dn07.norm(r),
        sapp: w.o_sapp.norm(r),
        land_dn: w.o_land_dn.norm(r),
        dn_filt: w.dn_filter(),
        odor_l: s.odor_l,
        odor_r: s.odor_r,
        flow_l: s.flow_l,
        flow_r: s.flow_r,
        loom: s.loom,
        taste: s.taste_hz,
        wall_dist,
        tau_aero: b.tau_aero,
        tau_damp: b.tau_damp,
        tau_yaw_tilt: { let (t, _) = yaw_torque_split(b); t },
        tau_yaw_amp: { let (_, a) = yaw_torque_split(b); a },
        wroll: b.omega[0],
        wpitch: b.omega[1],
        wyaw: b.omega[2],
        tilt_l: b.tilt_l,
        tilt_r: b.tilt_r,
        tilt_deg: b.tilt_deg(),
        fz_body: b.fz_body,
        fz_world: b.fz_world,
        steer_l_hz: group_hz(w, "motor_flight_steering_left"),
        steer_r_hz: group_hz(w, "motor_flight_steering_right"),
        touch: b.touching,
        wall_hits: b.wall_hits,
        takeoffs: b.takeoffs,
        landings: b.landings,
        eats: b.eats,
        win_spikes: w.window_spikes.len() as u32,
        tot_spikes: w.lif.total_spikes,
    }
}

/// Sub-slice helper: values of `s` where `keep(sample)` holds.
pub(crate) fn sel<F: Fn(&Sample) -> bool>(s: &[Sample], f: F) -> Vec<Sample> {
    s.iter().copied().filter(|x| f(x)).collect()
}

pub(crate) fn col<F: Fn(&Sample) -> f32>(s: &[Sample], f: F) -> Vec<f32> {
    s.iter().map(f).collect()
}