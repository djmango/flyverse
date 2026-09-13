//! Yaw instrumentation.
//!
//! The pitch investigation worked because the same questions could be asked of
//! a torque that was already being recorded. Yaw is instrumented here in the
//! same shape: the whole yaw moment `tau_aero[2]`, the damping moment
//! opposing it, the body-frame rate it produces, and the motor asymmetry
//! actually arriving from the connectome, split into the heading component the
//! stroke-plane tilt commands and the component that falls out of a
//! wing-power difference (`trace::yaw_torque_split`).
//!
//! It reports, per control window:
//!
//! * `tau_yaw` - the whole yaw moment, `WING_DY * (fr[0] - fl[0])`.
//! * `tau_tilt` / `tau_amp` - that moment split by cause.
//! * `tau_damp` - `damp[2] * omega[2]`, the moment the wings take back.
//! * `wyaw` - the body-frame yaw rate; `yawW` - its world-vertical projection,
//!   which is the quantity every turning number in the analysis comes from.
//! * `steerL` / `steerR` - the raw b1/b2/b3 pool rates in Hz, and their
//!   smoothed 0..1 read-out, so a saturated read-out is visible as such.
//!
//! and then, over the whole run: how one-signed the yaw moment is, how much of
//! it each cause carries, whether any axis is sitting on `MAX_OMEGA`, and
//! whether the wings can produce a yaw moment without producing a roll moment
//! at the same time (the actuator-capability question).

use crate::body::Mode;
use crate::sim::{World, WINDOW_S};
use crate::wing;
use anyhow::Result;
use std::path::Path;

use super::stats::mean64;
use super::trace::{group_hz, yaw_torque_split};

pub struct YawOptions {
    pub seconds: f64,
    pub seed: u64,
    /// Print one line every N control windows.
    pub every: u64,
}

/// Runs of consecutive samples of the same sign, ignoring samples inside
/// `+-dead`. Returns (number of runs, mean run length in samples, longest run).
fn sign_runs(v: &[f64], dead: f64) -> (usize, f64, usize) {
    let (mut runs, mut cur, mut n) = (0usize, 0i32, 0usize);
    let mut lens: Vec<usize> = Vec::new();
    for &x in v {
        let s = if x > dead {
            1
        } else if x < -dead {
            -1
        } else {
            0
        };
        if s == 0 {
            continue;
        }
        if s == cur {
            n += 1;
        } else {
            if cur != 0 {
                lens.push(n);
            }
            cur = s;
            n = 1;
            runs += 1;
        }
    }
    if cur != 0 {
        lens.push(n);
    }
    if lens.is_empty() {
        return (0, 0.0, 0);
    }
    let mean = lens.iter().sum::<usize>() as f64 / lens.len() as f64;
    (runs, mean, *lens.iter().max().unwrap())
}

fn pearson64(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let (ma, mb) = (mean64(&a[..n]), mean64(&b[..n]));
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let x = a[i] - ma;
        let y = b[i] - mb;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    let d = (da * db).sqrt();
    if d <= 1e-12 {
        0.0
    } else {
        num / d
    }
}

/// Can the wing pair produce a yaw moment without also producing a roll
/// moment? Sweeps the steering differential at a fixed operating point and
/// prints both moments, plus the pitching moment, so the coupling between the
/// three axes is a measured number rather than a claim about the model.
fn actuator_sweep(amp: f32, airspeed: f32, common: f32) {
    println!("\n=== ACTUATOR SWEEP: what the two-wing model can command ===");
    println!(
        "stroke amplitude {amp:.2} rad, airspeed {airspeed:.0} mm/s, common tilt {:.3} rad",
        wing::STEER_TILT_MAX * common
    );
    println!("  steer diff   tiltL   tiltR   tau_yaw   tau_roll  tau_pitch   yaw/roll");
    let mut d = -1.0f32;
    while d <= 1.0001 {
        let tl = wing::STEER_TILT_MAX * (common + 0.5 * d).clamp(-1.0, 1.0);
        let tr = wing::STEER_TILT_MAX * (common - 0.5 * d).clamp(-1.0, 1.0);
        let fl = wing::wing_force_vector(amp, wing::WINGBEAT_HZ, airspeed, tl);
        let fr = wing::wing_force_vector(amp, wing::WINGBEAT_HZ, airspeed, tr);
        let (dy, dz) = (wing::WING_DY, wing::WING_DZ);
        let tau_yaw = dy * (fr[0] - fl[0]);
        let tau_roll = dy * (fl[2] - fr[2]);
        let tau_pitch = dz * (fl[0] + fr[0]);
        let ratio = if tau_roll.abs() > 1e-9 {
            tau_yaw / tau_roll
        } else {
            f32::NAN
        };
        println!(
            "  {d:>+10.2}   {tl:>+6.3}  {tr:>+6.3}  {tau_yaw:>+9.2}  {tau_roll:>+9.2}  {tau_pitch:>+9.2}   {ratio:>+8.2}"
        );
        d += 0.25;
    }
    println!(
        "  A differential that reaches `STEER_TILT_MAX` clamps: with common = {:.3} the\n  \
         tilt can never be driven to zero on either wing, so no differential can\n  \
         itself null the mean forward tilt.",
        common
    );

    // Amplitude-only actuation, the other handle the model has.
    println!("\n  amplitude-only: power diff   ampL    ampR    tau_yaw   tau_roll");
    let mut pd = -0.2f32;
    while pd <= 0.2001 {
        let al = amp * (1.0 + 0.5 * pd).clamp(0.0, 1.5);
        let ar = amp * (1.0 - 0.5 * pd).clamp(0.0, 1.5);
        let tm = wing::STEER_TILT_MAX * common;
        let fl = wing::wing_force_vector(al, wing::WINGBEAT_HZ, airspeed, tm);
        let fr = wing::wing_force_vector(ar, wing::WINGBEAT_HZ, airspeed, tm);
        let (dy, _) = (wing::WING_DY, wing::WING_DZ);
        println!(
            "  {pd:>+10.2}                    {al:>6.3}  {ar:>6.3}  {:>+9.2}  {:>+9.2}",
            dy * (fr[0] - fl[0]),
            dy * (fl[2] - fr[2])
        );
        pd += 0.1;
    }
}

pub fn yaw_probe(pack: &Path, o: &YawOptions) -> Result<()> {
    let mut w = World::new(pack, o.seed, None)?;
    let windows = ((o.seconds / WINDOW_S as f64) as u64).max(1);
    let every = o.every.max(1);

    println!(
        "yaw-probe: {} neurons, {} edges, {:.0}s, seed {}, every {} windows ({:.0} ms)",
        w.conn.n,
        w.conn.m,
        o.seconds,
        o.seed,
        every,
        every as f32 * WINDOW_S * 1000.0
    );
    println!(
        "  haltere channel {}  retina {}  optic-flow proxy {}",
        if w.haltere_on { "ON" } else { "SILENCED" },
        if w.retina.on { "ON" } else { "SILENCED" },
        if w.flow_on && !w.retina.on { "ON" } else { "not delivered" }
    );
    println!(
        "\n{:>6} {:>4} {:>7} {:>6} {:>6} {:>6} {:>6} {:>6} {:>7} {:>7} {:>7} {:>8} {:>8} {:>8} {:>8} {:>8} {:>5}",
        "t", "mode", "airs", "z", "tiltL", "tiltR", "steerL", "steerR", "sLHz", "sRHz",
        "tsm", "tauYaw", "tauTilt", "tauAmp", "tauDamp", "yawW", "wall"
    );

    // Airborne-only accumulators, plus mode/wall bookkeeping over the whole run.
    let mut v_tau: Vec<f64> = Vec::new();
    let mut v_tilt: Vec<f64> = Vec::new();
    let mut v_amp: Vec<f64> = Vec::new();
    let mut v_damp: Vec<f64> = Vec::new();
    let mut v_wyaw: Vec<f64> = Vec::new();
    let mut v_yaww: Vec<f64> = Vec::new();
    let mut v_sdiff: Vec<f64> = Vec::new();
    let mut v_sdl: Vec<f64> = Vec::new();
    let mut v_sdr: Vec<f64> = Vec::new();
    let mut v_sl_hz: Vec<f64> = Vec::new();
    let mut v_sr_hz: Vec<f64> = Vec::new();
    // The reported "yaw rate" is `u . omega`, the world-vertical projection of
    // the body-frame rate. That equals the yaw rate only while the body is
    // level, so the three projections are accumulated separately: if the roll
    // and pitch terms are comparable to the yaw term the reported figure is not
    // a yaw rate.
    let mut v_proj_roll: Vec<f64> = Vec::new();
    let mut v_proj_pitch: Vec<f64> = Vec::new();
    let mut v_proj_yaw: Vec<f64> = Vec::new();
    // The honest horizontal turn rate: the rate of change of the horizontal
    // direction of the body's own nose axis.
    let mut v_head_rate: Vec<f64> = Vec::new();
    let mut v_tilt_deg: Vec<f64> = Vec::new();
    let mut v_hall: Vec<f64> = Vec::new();
    let mut v_hall_l: Vec<f64> = Vec::new();
    let mut v_hall_r: Vec<f64> = Vec::new();
    let mut prev_head: Option<f64> = None;
    // The body's dorsal (up) axis in world coordinates: this is the axis the
    // body-frame yaw rate rotates ABOUT, so it decides whether a body-frame
    // spin is a turn in the horizontal plane or a roll/pitch oscillation.
    let (mut v_u0, mut v_u1, mut v_u2) = (Vec::new(), Vec::new(), Vec::new());
    let mut v_tilt_l: Vec<f64> = Vec::new();
    let mut v_tilt_r: Vec<f64> = Vec::new();
    let mut v_amp_l: Vec<f64> = Vec::new();
    let mut v_amp_r: Vec<f64> = Vec::new();
    let mut v_airspeed: Vec<f64> = Vec::new();
    let mut v_hspeed: Vec<f64> = Vec::new();
    let mut v_x: Vec<f64> = Vec::new();
    let mut v_y: Vec<f64> = Vec::new();
    let mut v_z: Vec<f64> = Vec::new();
    let (mut n_air, mut n_parked, mut n_corner, mut n_wall, mut n_ceil) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut clamp_roll, mut clamp_pitch, mut clamp_yaw, mut clamp_yaww) = (0u64, 0u64, 0u64, 0u64);
    let (mut sat_l, mut sat_r) = (0u64, 0u64);
    // Raw (unsmoothed) read-out: the clip is applied here, before the 30 ms
    // smoothing, so this is where a mis-ranged anchor shows up.
    let mut v_raw_sl: Vec<f64> = Vec::new();
    let mut v_raw_sr: Vec<f64> = Vec::new();
    let mut v_cnt_l: Vec<f64> = Vec::new();
    let mut v_cnt_r: Vec<f64> = Vec::new();
    let (mut raw_sat_l, mut raw_sat_r) = (0u64, 0u64);
    let gi_l = w.groups.idx("motor_flight_steering_left");
    let gi_r = w.groups.idx("motor_flight_steering_right");
    let n_l = if gi_l == usize::MAX { 0 } else { w.groups.group_size[gi_l] };
    let n_r = if gi_r == usize::MAX { 0 } else { w.groups.group_size[gi_r] };

    for k in 0..windows {
        w.advance();
        let b = &w.body;
        let m = w.motors();
        let airspeed = b.airspeed();
        let (tl, ta) = yaw_torque_split(b);
        let tau_yaw = b.tau_aero[2];
        let tau_damp = b.tau_damp[2];
        let wyaw = b.omega[2];
        let yaw_w = w.yaw_rate_rad_s();
        let sl = group_hz(&w, "motor_flight_steering_left");
        let sr = group_hz(&w, "motor_flight_steering_right");
        let airborne = matches!(b.mode, Mode::Takeoff | Mode::Cruise | Mode::Landing);

        if k % every == 0 {
            println!(
                "{:>6.2} {:>4} {:>7.0} {:>6.1} {:>+6.3} {:>+6.3} {:>6.3} {:>6.3} {:>7.1} {:>7.1} {:>+7.3} {:>+8.1} {:>+8.1} {:>+8.1} {:>+8.1} {:>+8.2} {:>5}",
                w.step as f32 * crate::lif::DT_MS / 1000.0,
                b.mode.as_str(),
                airspeed,
                b.pos[2],
                b.tilt_l,
                b.tilt_r,
                m.flight_steer_l,
                m.flight_steer_r,
                sl,
                sr,
                m.flight_steer_r - m.flight_steer_l,
                tau_yaw,
                tl,
                ta,
                tau_damp,
                yaw_w,
                if b.touching[0] || b.touching[1] { 1 } else { 0 }
            );
        }

        if airborne {
            n_air += 1;
            v_tau.push(tau_yaw as f64);
            v_tilt.push(tl as f64);
            v_amp.push(ta as f64);
            v_damp.push(tau_damp as f64);
            v_wyaw.push(wyaw as f64);
            v_yaww.push(yaw_w as f64);
            v_sdiff.push((m.flight_steer_r - m.flight_steer_l) as f64);
            v_sdl.push(m.flight_steer_l as f64);
            v_sdr.push(m.flight_steer_r as f64);
            v_sl_hz.push(sl as f64);
            v_sr_hz.push(sr as f64);
            // Split the reported world-vertical projection into its three
            // body-axis contributions: u is the body's up axis in world, so
            // u . omega = u[0]*w_roll + u[1]*w_pitch + u[2]*w_yaw.
            let u = b.up();
            v_u0.push(u[0] as f64);
            v_u1.push(u[1] as f64);
            v_u2.push(u[2] as f64);
            v_proj_roll.push((u[0] * b.omega[0]) as f64);
            v_proj_pitch.push((u[1] * b.omega[1]) as f64);
            v_proj_yaw.push((u[2] * b.omega[2]) as f64);
            v_tilt_deg.push(b.tilt_deg() as f64);
            let head = {
                let f = b.forward();
                (f[1] as f64).atan2(f[0] as f64)
            };
            if let Some(p) = prev_head {
                let mut d = head - p;
                while d > std::f64::consts::PI {
                    d -= std::f64::consts::TAU;
                }
                while d < -std::f64::consts::PI {
                    d += std::f64::consts::TAU;
                }
                v_head_rate.push(d / WINDOW_S as f64);
            }
            prev_head = Some(head);
            v_hall.push(0.5 * (b.haltere_l + b.haltere_r) as f64);
            v_hall_l.push(b.haltere_l as f64);
            v_hall_r.push(b.haltere_r as f64);
            v_tilt_l.push(b.tilt_l as f64);
            v_tilt_r.push(b.tilt_r as f64);
            v_amp_l.push(b.wing_amp_l as f64);
            v_amp_r.push(b.wing_amp_r as f64);
            v_airspeed.push(airspeed as f64);
            v_hspeed.push(b.speed() as f64);
            v_x.push(b.pos[0] as f64);
            v_y.push(b.pos[1] as f64);
            v_z.push(b.pos[2] as f64);
            if b.speed() < 10.0 {
                n_parked += 1;
            }
            if b.touching[0] && b.touching[1] {
                n_corner += 1;
            }
            if b.touching[0] || b.touching[1] {
                n_wall += 1;
            }
            if b.touching[2] {
                n_ceil += 1;
            }
            let lim = 0.999 * crate::body::MAX_OMEGA;
            if b.omega[0].abs() >= lim {
                clamp_roll += 1;
            }
            if b.omega[1].abs() >= lim {
                clamp_pitch += 1;
            }
            if b.omega[2].abs() >= lim {
                clamp_yaw += 1;
            }
            if yaw_w.abs() >= lim {
                clamp_yaww += 1;
            }
            if m.flight_steer_l >= 0.999 {
                sat_l += 1;
            }
            if m.flight_steer_r >= 0.999 {
                sat_r += 1;
            }
            // The read-out is smoothed over 30 ms (MOTOR_TAU_S), so the smoothed
            // value lags the clip: count the RAW read-out too, which is what the
            // neuron group actually delivers to the body each 2 ms window.
            let raw = w.raw_motors();
            v_raw_sl.push(raw.flight_steer_l as f64);
            v_raw_sr.push(raw.flight_steer_r as f64);
            if raw.flight_steer_l >= 0.999 {
                raw_sat_l += 1;
            }
            if raw.flight_steer_r >= 0.999 {
                raw_sat_r += 1;
            }
            let (cl, cr) = (w.rates.counts[gi_l], w.rates.counts[gi_r]);
            v_cnt_l.push(cl as f64);
            v_cnt_r.push(cr as f64);
        }
    }

    let na = n_air.max(1) as f64;
    let pos = v_tau.iter().filter(|v| **v > 0.0).count() as f64 / na;
    let neg = v_tau.iter().filter(|v| **v < 0.0).count() as f64 / na;

    println!("\n=== YAW ATTRIBUTION ===");
    println!(
        "  airborne windows            {} of {} ({:.1}%)",
        n_air,
        windows,
        100.0 * n_air as f64 / windows.max(1) as f64
    );
    println!(
        "  tau_yaw        mean {:>+9.1}  mean|.| {:>9.1}  min {:>+9.1}  max {:>+9.1}",
        mean64(&v_tau),
        mean64(&v_tau.iter().map(|v| v.abs()).collect::<Vec<_>>()),
        v_tau.iter().cloned().fold(f64::INFINITY, f64::min),
        v_tau.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!(
        "  one-signed?    tau_yaw > 0 in {:.1}% of airborne windows, < 0 in {:.1}%",
        100.0 * pos,
        100.0 * neg
    );
    let (runs, run_len, run_max) = sign_runs(&v_tau, 1.0);
    println!(
        "  sign runs      {} runs, mean {:.1} windows ({:.2} s), longest {} windows ({:.2} s)",
        runs,
        run_len,
        run_len * WINDOW_S as f64,
        run_max,
        run_max as f64 * WINDOW_S as f64
    );
    println!(
        "  cause split    tilt-differential component mean {:>+9.1}   amplitude-differential component mean {:>+9.1}",
        mean64(&v_tilt),
        mean64(&v_amp)
    );
    println!(
        "  damping        tau_damp mean {:>+9.1} (opposes tau_yaw)",
        mean64(&v_damp)
    );
    println!(
        "  rates          body-frame yaw rate mean {:>+8.3} rad/s   world-vertical mean {:>+8.3} rad/s",
        mean64(&v_wyaw),
        mean64(&v_yaww)
    );
    println!(
        "  mean |rates|   body {:>8.3}   world {:>8.3} rad/s",
        mean64(&v_wyaw.iter().map(|v| v.abs()).collect::<Vec<_>>()),
        mean64(&v_yaww.iter().map(|v| v.abs()).collect::<Vec<_>>())
    );
    // What the reported turning number is actually made of.
    println!(
        "  MEASUREMENT    the reported rate is u . omega, the world-vertical projection of the\n\
         \x20                body-frame rate. It equals the yaw rate only while the body is level.\n\
         \x20                mean tilt from level {:.1} deg ({:.0}% of airborne windows above 20 deg)",
        mean64(&v_tilt_deg),
        100.0 * v_tilt_deg.iter().filter(|t| **t > 20.0).count() as f64 / na
    );
    println!(
        "  projection     |u0*wroll| {:>8.3}   |u1*wpitch| {:>8.3}   |u2*wyaw| {:>8.3}   (roll/pitch terms are attitude leakage, not turning)",
        mean64(&v_proj_roll.iter().map(|v| v.abs()).collect::<Vec<_>>()),
        mean64(&v_proj_pitch.iter().map(|v| v.abs()).collect::<Vec<_>>()),
        mean64(&v_proj_yaw.iter().map(|v| v.abs()).collect::<Vec<_>>())
    );
    println!(
        "  projection     corr(body-frame yaw rate, reported rate) = {:>+6.3}",
        pearson64(&v_wyaw, &v_yaww)
    );
    println!(
            "  true turn      horizontal heading rate mean {:>+7.3} rad/s, mean|.| {:>7.3} rad/s",
            mean64(&v_head_rate),
            mean64(&v_head_rate.iter().map(|v| v.abs()).collect::<Vec<_>>())
        );
        // The body spins about its own dorsal axis. Whether that is a turn in the
        // horizontal plane depends entirely on where that axis points.
        println!(
            "  spin axis      body dorsal axis in world  mean u = ({:>+6.3}, {:>+6.3}, {:>+6.3})  ->  yaw component u2 = {:.3}",
            mean64(&v_u0),
            mean64(&v_u1),
            mean64(&v_u2),
            mean64(&v_u2)
        );
    println!(
        "  haltere        mean rate {:>6.1} Hz (base {:.0})  L->R difference mean {:>+6.2} Hz",
        mean64(&v_hall),
        wing::HALTERE_BASE_HZ,
        mean64(&v_hall_l) - mean64(&v_hall_r)
    );
    println!(
        "  stroke plane   tiltL mean {:>+7.4}  tiltR mean {:>+7.4}  differential mean {:>+7.4} rad",
        mean64(&v_tilt_l),
        mean64(&v_tilt_r),
        mean64(&v_tilt_l.iter().zip(&v_tilt_r).map(|(a, b)| b - a).collect::<Vec<_>>())
    );
    println!(
        "  wing power     ampL mean {:>6.3}  ampR mean {:>6.3} rad  (full stroke {:.3})",
        mean64(&v_amp_l),
        mean64(&v_amp_r),
        wing::STROKE_AMP_MAX
    );
    println!(
        "  motor drive    steer_l mean {:.4}  steer_r mean {:.4}  differential mean {:>+7.4}",
        mean64(&v_sdl),
        mean64(&v_sdr),
        mean64(&v_sdiff)
    );
    println!(
        "  raw pools      steering L {:.1} Hz  steering R {:.1} Hz  difference {:>+6.1} Hz  (read-out normaliser 110 Hz)",
        mean64(&v_sl_hz),
        mean64(&v_sr_hz),
        mean64(&v_sl_hz) - mean64(&v_sr_hz)
    );
    println!(
        "  saturation     read-out at 1.0: left {:.1}%  right {:.1}% of airborne windows",
        100.0 * sat_l as f64 / na,
        100.0 * sat_r as f64 / na
    );
    // The clip happens BEFORE the 30 ms smoothing (sim.rs `smooth_motors`), so
    // the smoothed figure above understates it badly. What the body actually
    // receives each 2 ms window is the raw read-out, and that is what is counted
    // here alongside the spike count that produced it.
    println!(
        "  RAW read-out   L mean {:.4} at 1.0 in {:.1}% of windows ({} neurons, {:.2} spikes/window mean)",
        mean64(&v_raw_sl),
        100.0 * raw_sat_l as f64 / windows.max(1) as f64,
        n_l,
        mean64(&v_cnt_l)
    );
    println!(
        "  RAW read-out   R mean {:.4} at 1.0 in {:.1}% of windows ({} neurons, {:.2} spikes/window mean)",
        mean64(&v_raw_sr),
        100.0 * raw_sat_r as f64 / windows.max(1) as f64,
        n_r,
        mean64(&v_cnt_r)
    );
    println!(
        "  clip           raw rate that saturates the read-out = 110 Hz; measured pool rate {:.1} Hz = {:.2}x the anchor",
        mean64(&v_sl_hz),
        mean64(&v_sl_hz) / 110.0
    );
    println!(
        "  agencyless?    horizontal speed < 10 mm/s in {:.1}% of airborne windows; touching two walls {:.1}%, a wall {:.1}%, ceiling {:.1}%",
        100.0 * n_parked as f64 / na,
        100.0 * n_corner as f64 / na,
        100.0 * n_wall as f64 / na,
        100.0 * n_ceil as f64 / na
    );
    println!(
        "  position       mean ({:.1}, {:.1}, {:.1}) mm   airspeed mean {:.0} mm/s  horizontal mean {:.0} mm/s",
        mean64(&v_x),
        mean64(&v_y),
        mean64(&v_z),
        mean64(&v_airspeed),
        mean64(&v_hspeed)
    );
    println!(
        "  clamps (MAX_OMEGA = {:.0})  roll {:.2}%  pitch {:.2}%  yaw-body {:.2}%  yaw-world {:.2}%",
        crate::body::MAX_OMEGA,
        100.0 * clamp_roll as f64 / na,
        100.0 * clamp_pitch as f64 / na,
        100.0 * clamp_yaw as f64 / na,
        100.0 * clamp_yaww as f64 / na
    );
    println!(
        "  loop sign      corr(steer differential, world yaw rate) = {:>+6.3}   (negative = counter-steering, positive = positive feedback)",
        pearson64(&v_sdiff, &v_yaww)
    );
    println!(
        "  loop sign      corr(steer differential, tau_yaw)         = {:>+6.3}",
        pearson64(&v_sdiff, &v_tau)
    );
    println!(
        "  loop sign      corr(tau_yaw, body yaw rate)              = {:>+6.3}   (nothing restores yaw; the wing damping is the only brake)",
        pearson64(&v_tau, &v_wyaw)
    );

    let amp_mean = mean64(&v_amp_l) as f32;
    let air_mean = mean64(&v_airspeed) as f32;
    let common_mean = (mean64(&v_sdl) + mean64(&v_sdr)) as f32 * 0.5;
    actuator_sweep(amp_mean, air_mean, common_mean);
    Ok(())
}