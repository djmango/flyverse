//! The hand: what the retina and the loom channel did with it, and whether the
//! fly turned away from it.
//!
//! Split in two on purpose, because the two halves answer different questions
//! and only the first one can be a delivery check:
//!
//! 1. **The stimulus side.** Columns on the palm, the fraction of each eye it
//!    covers, the nearest palm distance each eye reports, the per-eye luminance
//!    it produces, the loom each eye is given, and the firing rate of the two
//!    `visual_loom` pools. If the palm does not reach the retina, nothing about
//!    the fly's behaviour is informative and this is where that shows up.
//! 2. **The response side.** Signed bearing to the palm against the turn
//!    command and the yaw rate; whether the fly's own motion is away from the
//!    palm; and whether it accelerates or changes mode. The static-hand control
//!    run produces the same numbers from the same code, so the two are directly
//!    comparable window for window.
//!
//! Nothing in this module feeds back into the simulation. It reads, it counts,
//! it writes.

use crate::lif::DT_MS;
use crate::room::{self, HandTraj, V3};
use crate::sim::World;

use super::stats::mean;
use super::trace::{group_hz, mode_code};

// ---------------------------------------------------------------------------
// Trace

/// One window's worth of hand telemetry. Hand-written rather than folded into
/// `trace::Sample` so that `trace.csv` keeps its exact schema and a run with no
/// hand in it stays byte-for-byte what it was.
#[derive(Clone, Copy, Default)]
pub struct HandSample {
    pub t: f32,
    /// Body centre, mm.
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Palm centre, mm.
    pub hx: f32,
    pub hy: f32,
    pub hz: f32,
    /// Distance from the body centre to the palm centre, mm.
    pub hand_dist: f32,
    /// Distance from the body centre to the palm's nearest surface, mm. Zero
    /// means the palm is on the fly.
    pub hand_surf: f32,
    /// Signed bearing of the palm relative to the body's horizontal heading,
    /// rad, positive to the LEFT (the same convention as `fruit_az`), so
    /// `hand_az * yaw_rate < 0` is a sample that is turning away from it.
    pub hand_az: f32,
    /// Elevation of the palm above the body's horizontal plane, rad.
    pub hand_el: f32,
    /// The body's OWN velocity projected on the unit vector from the palm to
    /// the body, mm/s. Positive = the fly is moving away from the palm under its
    /// own power, negative = it is closing with it. This is the quantity that
    /// separates "the fly moved away" from "the hand was moving".
    pub hand_radial_v: f32,
    /// The palm's speed along its own trajectory, mm/s (0 in the static control).
    pub hand_speed: f32,
    /// Columns looking at the palm, per eye, and the fraction of that eye's
    /// columns that is.
    pub cols_l: u32,
    pub cols_r: u32,
    pub frac_l: f32,
    pub frac_r: f32,
    /// Nearest palm-surface distance reported by that eye's own columns, mm
    /// (`INFINITY` when no column of that eye is on the palm).
    pub near_l: f32,
    pub near_r: f32,
    /// Mean luminance-channel catch per eye: the palm darkens whatever it
    /// covers, so this is where the darkening shows.
    pub lum_l: f32,
    pub lum_r: f32,
    /// Loom delivered to each eye's `visual_loom` drive this window, 0..1.
    pub loom_l: f32,
    pub loom_r: f32,
    /// Firing rate of the two `visual_loom` pools, Hz. These are single cells;
    /// their rate is the direct read-out of the looming channel.
    pub loom_hz_l: f32,
    pub loom_hz_r: f32,
    /// Steering pools: the smoothed 0..1 command and the raw Hz.
    pub steer_l: f32,
    pub steer_r: f32,
    pub steer_hz_l: f32,
    pub steer_hz_r: f32,
    pub yaw_rate: f32,
    pub speed: f32,
    pub mode: u8,
    pub wall_dist: f32,
    pub takeoffs: u32,
    pub tot_spikes: u64,
}

pub fn sample_of_hand(w: &World) -> Option<HandSample> {
    let h = w.hand()?;
    let b = &w.body;
    let p = b.pos;
    let d = room::sub(h.c, p);
    let dist = room::len(d);
    let hd = b.heading();
    let cross = hd[0] * d[1] - hd[1] * d[0];
    let dotv = hd[0] * d[0] + hd[1] * d[1];
    let az = cross.atan2(dotv);
    let horiz = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let el = d[2].atan2(horiz);
    // The body's own motion away from the palm.
    let u = room::norm(d);
    let radial = room::dot(b.vel, u);
    let (cols, frac) = w.retina.hand_columns_by_eye();
    let near = w.retina.hand_distance_by_eye();
    let (ml, mr) = w.retina.mean_lum_by_eye();
    let (ll, lr) = w.loom_pair();
    let m = w.motors();
    Some(HandSample {
        t: w.step as f32 * DT_MS / 1000.0,
        x: p[0],
        y: p[1],
        z: p[2],
        hx: h.c[0],
        hy: h.c[1],
        hz: h.c[2],
        hand_dist: dist,
        hand_surf: h.surface_dist(p),
        hand_az: az,
        hand_el: el,
        hand_radial_v: radial,
        hand_speed: w.hand_traj.map(|t| t.speed_mm_s()).unwrap_or(0.0),
        cols_l: cols[0] as u32,
        cols_r: cols[1] as u32,
        frac_l: frac[0],
        frac_r: frac[1],
        near_l: near[0],
        near_r: near[1],
        lum_l: ml,
        lum_r: mr,
        loom_l: ll,
        loom_r: lr,
        loom_hz_l: group_hz(w, "visual_loom_left"),
        loom_hz_r: group_hz(w, "visual_loom_right"),
        steer_l: m.flight_steer_l,
        steer_r: m.flight_steer_r,
        steer_hz_l: group_hz(w, "motor_flight_steering_left"),
        steer_hz_r: group_hz(w, "motor_flight_steering_right"),
        yaw_rate: w.yaw_rate_rad_s(),
        speed: b.speed(),
        mode: mode_code(b.mode),
        wall_dist: (p[0] - w.room.x[0])
            .min(w.room.x[1] - p[0])
            .min(p[1] - w.room.y[0])
            .min(w.room.y[1] - p[1]),
        takeoffs: b.takeoffs,
        tot_spikes: w.lif.total_spikes,
    })
}

/// CSV of the hand trace, one row per sampled window.
pub fn write_csv(path: &std::path::Path, s: &[HandSample]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        f,
        "t,x,y,z,hx,hy,hz,hand_dist,hand_surf,hand_az,hand_el,hand_radial_v,hand_speed,\
cols_l,cols_r,frac_l,frac_r,near_l,near_r,lum_l,lum_r,loom_l,loom_r,loom_hz_l,loom_hz_r,\
steer_l,steer_r,steer_l_hz,steer_r_hz,yaw_rate,speed,mode,takeoffs,tot_spikes"
    )?;
    for x in s {
        writeln!(
            f,
            "{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:.4},{:.2},\
{},{},{:.6},{:.6},{:.4},{:.4},{:.6},{:.6},{:.6},{:.6},{:.4},{:.4},{:.6},{:.6},{:.4},{:.4},\
{:.6},{:.4},{},{},{}",
            x.t,
            x.x,
            x.y,
            x.z,
            x.hx,
            x.hy,
            x.hz,
            x.hand_dist,
            x.hand_surf,
            x.hand_az,
            x.hand_el,
            x.hand_radial_v,
            x.hand_speed,
            x.cols_l,
            x.cols_r,
            x.frac_l,
            x.frac_r,
            x.near_l,
            x.near_r,
            x.lum_l,
            x.lum_r,
            x.loom_l,
            x.loom_r,
            x.loom_hz_l,
            x.loom_hz_r,
            x.steer_l,
            x.steer_r,
            x.steer_hz_l,
            x.steer_hz_r,
            x.yaw_rate,
            x.speed,
            x.mode,
            x.takeoffs,
            x.tot_spikes
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Statistics

/// Two-sided exact binomial p for `k` successes in `n` trials at p = 1/2.
///
/// Reported with the same caveat the earlier notes attach to their sign tests:
/// consecutive windows are not independent draws of a stationary process, so
/// this p is a guide to whether the split is lopsided, not a real p-value. It is
/// quoted next to the fraction so the fraction is not read as though 60/100
/// meant something it does not.
pub(crate) fn binom_two_sided(n: usize, k: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let mut lnf = vec![0.0f64; n + 1];
    for i in 2..=n {
        lnf[i] = lnf[i - 1] + (i as f64).ln();
    }
    let ln = |i: usize| lnf[n] - lnf[i] - lnf[n - i] - (n as f64) * std::f64::consts::LN_2;
    let pk = ln(k);
    let mut p = 0.0f64;
    for i in 0..=n {
        let v = ln(i);
        if v <= pk + 1e-12 {
            p += v.exp();
        }
    }
    p.min(1.0)
}

fn med(v: &[f32]) -> f32 {
    super::stats::pct(v, 0.5)
}

fn colf<F: Fn(&HandSample) -> f32>(s: &[HandSample], f: F) -> Vec<f32> {
    s.iter().map(f).collect()
}

fn selw<F: Fn(&HandSample) -> bool>(s: &[HandSample], f: F) -> Vec<HandSample> {
    s.iter().copied().filter(|x| f(x)).collect()
}

/// The three measurement windows, in seconds: before the launch, the launch
/// itself (the expansion), and the half second after contact.
struct Windows {
    pre: Vec<HandSample>,
    pulse: Vec<HandSample>,
    post: Vec<HandSample>,
}

fn windows(s: &[HandSample], t: &HandTraj) -> Windows {
    let a = t.start_s;
    let b = t.start_s + t.dur_s;
    Windows {
        pre: selw(s, |x| x.t >= a - 0.5 && x.t < a),
        pulse: selw(s, |x| x.t >= a && x.t < b),
        post: selw(s, |x| x.t >= b && x.t <= b + 0.5),
    }
}

/// Human-facing console summary of the hand block, printed by `analyze` when a
/// hand is in the room. Short on purpose: the JSON has everything.
pub fn headline(h: &serde_json::Value) -> String {
    let g = |a: &str, b: &str| -> f64 { h[a][b].as_f64().unwrap_or(f64::NAN) };
    let g2 = |a: &str, b: &str, c: &str| -> f64 { h[a][b][c].as_f64().unwrap_or(f64::NAN) };
    let s = |a: &str, b: &str| -> String {
        match h[a][b].as_str() {
            Some(v) => v.to_string(),
            None => "-".into(),
        }
    };
    let launch = match h["trajectory"]["taken_at_ms"].as_f64() {
        Some(t) => format!(
            "aimed at the fly's position at t={:.0} ms, {:.0} mm away, {:.0} mm/s, aim error {:.1} mm",
            t,
            h["trajectory"]["launch_distance_mm"].as_f64().unwrap_or(f64::NAN),
            h["trajectory"]["launch_speed_mm_s"].as_f64().unwrap_or(f64::NAN),
            h["trajectory"]["aim_error_mm"].as_f64().unwrap_or(f64::NAN),
        ),
        None => "static: palm parked, no launch".to_string(),
    };
    format!(
        "HAND ({})\n  \
         retinal      {:.0}% of the pulse had the palm on the retina, peak {} L / {} R columns \
         ({:.1}% / {:.1}% of an eye), first seen at {} ms, nearest palm surface {:.1} mm\n  \
         loom         delivered L {:.3} -> {:.3} (max {:.3}); R {:.3} -> {:.3} (max {:.3}); peak at {:.0} ms\n  \
         loom pools   L {:.1} -> {:.1} Hz (max {:.1}); R {:.1} -> {:.1} Hz (max {:.1})\n  \
         launch       {}\n  \
         turn away    yaw: {:.1}% of {} samples, p {:.3}; steer(detrended): {:.1}% of {}, p {:.3}\n  \
         radial       body's own motion away from the palm: mean {:+.1} mm/s (pre {:+.1}), {:.1}% of samples away\n  \
         distance     palm centre {:.1} -> {:.1} mm over the pulse (run min {:.1}); post-last {:.1}\n  \
         acceleration {:.1} -> {:.1} mm/s (delta {:+.1}); takeoffs in pulse {}, in the 500 ms after {}",
        s("stimulus", "mode"),
        g("retina", "pct_pulse_windows_with_palm_on_retina"),
        g("retina", "peak_cols_l"),
        g("retina", "peak_cols_r"),
        g("retina", "peak_frac_l") * 100.0,
        g("retina", "peak_frac_r") * 100.0,
        g("retina", "first_seen_ms"),
        g("retina", "nearest_palm_surface_mm_over_run"),
        g2("retina", "loom_delivered_l", "pre_mean"),
        g2("retina", "loom_delivered_l", "pulse_mean"),
        g2("retina", "loom_delivered_l", "pulse_max"),
        g2("retina", "loom_delivered_r", "pre_mean"),
        g2("retina", "loom_delivered_r", "pulse_mean"),
        g2("retina", "loom_delivered_r", "pulse_max"),
        g("retina", "peak_loom_ms"),
        g2("retina", "loom_pool_hz_left", "pre_mean"),
        g2("retina", "loom_pool_hz_left", "pulse_mean"),
        g2("retina", "loom_pool_hz_left", "pulse_max"),
        g2("retina", "loom_pool_hz_right", "pre_mean"),
        g2("retina", "loom_pool_hz_right", "pulse_mean"),
        g2("retina", "loom_pool_hz_right", "pulse_max"),
        launch,
        g2("response", "turn_away_yaw", "pct_away"),
        g2("response", "turn_away_yaw", "samples"),
        g2("response", "turn_away_yaw", "binom_two_sided_p"),
        g2("response", "turn_away_steer_detrended", "pct_away"),
        g2("response", "turn_away_steer_detrended", "samples"),
        g2("response", "turn_away_steer_detrended", "binom_two_sided_p"),
        g2("response", "radial_velocity_away_mm_s", "pulse_mean"),
        g2("response", "radial_velocity_away_mm_s", "pre_mean"),
        g2("response", "radial_velocity_away_mm_s", "pct_samples_away"),
        g2("response", "distance_to_palm_mm", "pulse_first"),
        g2("response", "distance_to_palm_mm", "pulse_last"),
        g2("response", "distance_to_palm_mm", "run_min"),
        g2("response", "distance_to_palm_mm", "post_last"),
        g2("response", "acceleration", "mean_speed_pre_mm_s"),
        g2("response", "acceleration", "mean_speed_pulse_mm_s"),
        g2("response", "acceleration", "speed_change_mm_s"),
        g2("response", "mode_change", "takeoffs_in_pulse"),
        g2("response", "mode_change", "takeoffs_in_post_500ms"),
    )
}

// ---------------------------------------------------------------------------
/// The aim point the trajectory was actually launched with, in mm, and the sim
/// time in ms at which it was taken -- plus the fly's own position at that
/// instant, so the aim error is exact rather than inferred.
pub fn aim_block(w: &World) -> serde_json::Value {
    let r1 = |x: f32| (x * 10.0).round() / 10.0;
    let a1 = |v: V3| v.map(r1);
    match (w.hand_aim, w.hand_traj) {
        (Some((aim, t_ms, fly)), Some(tr)) => serde_json::json!({
            "aim_point_mm": a1(aim),
            "taken_at_ms": r1(t_ms),
            "fly_at_launch_mm": a1(fly),
            "aim_error_mm": r1(room::len(room::sub(aim, fly))),
            "aimed_at_fly": tr.aimed_at_fly,
            "launch_origin_mm": a1(tr.origin),
            "launch_distance_mm": r1(room::len(room::sub(aim, tr.origin))),
            "launch_speed_mm_s": r1(tr.speed_mm_s()),
        }),
        (None, Some(tr)) => serde_json::json!({
            "aimed_at_fly": false,
            "note": "static hand: no launch, palm parked at the trajectory origin",
            "parked_at_mm": a1(tr.origin),
        }),
        _ => serde_json::Value::Null,
    }
}

/// The measurement

/// Everything the hand run is asked: did the stimulus arrive, and did the fly
/// answer it. One JSON block, written into the run's `summary.json` under
/// `hand` and into `hand.json` on its own.
pub fn analyze(s: &[HandSample], t: &HandTraj) -> serde_json::Value {
    let w = windows(s, t);
    let at = |v: &Vec<HandSample>| v.len().max(1) as f64;

    // ---- 1. the stimulus side -------------------------------------------
    let peak_loom = w
        .pulse
        .iter()
        .map(|x| x.loom_l.max(x.loom_r))
        .fold(0.0f32, f32::max);
    let peak_loom_t = w
        .pulse
        .iter()
        .max_by(|a, b| a.loom_l.max(a.loom_r).partial_cmp(&b.loom_l.max(b.loom_r)).unwrap())
        .map(|x| x.t)
        .unwrap_or(0.0);
    let first_seen = w
        .pulse
        .iter()
        .find(|x| x.cols_l + x.cols_r > 0)
        .map(|x| x.t * 1000.0);
    let first_loom_up = w
        .pulse
        .iter()
        .find(|x| x.loom_l.max(x.loom_r) > 0.5)
        .map(|x| x.t * 1000.0);
    let near_min = s.iter().map(|x| x.hand_surf).fold(f32::INFINITY, f32::min);

    // ---- 2. the response side -------------------------------------------
    // The sign test: `hand_az > 0` means the palm is to the LEFT, and a
    // positive yaw rate turns left, so turning AWAY is `hand_az * yaw_rate < 0`.
    // The same test on the steering command, which the steering-differential
    // note established is one-signed in this specimen (steer_r - steer_l < 0 in
    // 100 % of samples), so the RAW command test is reported with its baseline
    // and the detrended version is the one that can carry information.
    let steer_base = mean(&colf(&w.pre, |x| x.steer_l - x.steer_r));
    let sign = |v: &Vec<HandSample>, raw: bool| -> serde_json::Value {
        let mut away = 0usize;
        let mut toward = 0usize;
        let mut zero = 0usize;
        for x in v {
            let turn = if raw {
                x.yaw_rate
            } else {
                x.steer_l - x.steer_r
            };
            let v2 = x.hand_az * turn;
            if v2 < 0.0 {
                away += 1;
            } else if v2 > 0.0 {
                toward += 1;
            } else {
                zero += 1;
            }
        }
        let n = away + toward;
        serde_json::json!({
            "sign": if raw { "az * yaw_rate" } else { "az * (steer_l - steer_r)" },
            "samples": n,
            "away": away,
            "toward": toward,
            "zero": zero,
            "pct_away": if n == 0 { 0.0 } else { 100.0 * away as f64 / n as f64 },
            "pct_toward": if n == 0 { 0.0 } else { 100.0 * toward as f64 / n as f64 },
            "binom_two_sided_p": binom_two_sided(n, away),
        })
    };
    // Detrended: does the steering command move in the away direction relative
    // to its own pre-stimulus mean? A one-signed channel can only be read this
    // way.
    let detrend = {
        let (mut away, mut toward) = (0usize, 0usize);
        for x in &w.pulse {
            let d = (x.steer_l - x.steer_r) - steer_base;
            let v = x.hand_az * d;
            if v < 0.0 {
                away += 1;
            } else if v > 0.0 {
                toward += 1;
            }
        }
        let n = away + toward;
        serde_json::json!({
            "samples": n,
            "away": away,
            "toward": toward,
            "pct_away": if n == 0 { 0.0 } else { 100.0 * away as f64 / n as f64 },
            "pre_baseline_steer_l_minus_r": steer_base,
            "pulse_mean_steer_l_minus_r": mean(&colf(&w.pulse, |x| x.steer_l - x.steer_r)),
            "binom_two_sided_p": binom_two_sided(n, away),
        })
    };

    let rad_pulse = colf(&w.pulse, |x| x.hand_radial_v);
    let rad_pre = colf(&w.pre, |x| x.hand_radial_v);
    let n_away_v = rad_pulse.iter().filter(|v| **v > 0.0).count();
    let dist_first = w.pulse.first().map(|x| x.hand_dist).unwrap_or(f32::NAN);
    let dist_last = w.pulse.last().map(|x| x.hand_dist).unwrap_or(f32::NAN);
    let speed_pulse = mean(&colf(&w.pulse, |x| x.speed));
    let speed_pre = mean(&colf(&w.pre, |x| x.speed));
    let takeoffs_pulse = w
        .pulse
        .last()
        .zip(w.pulse.first())
        .map(|(a, b)| a.takeoffs.saturating_sub(b.takeoffs))
        .unwrap_or(0);
    let takeoffs_post = w
        .post
        .last()
        .zip(w.post.first())
        .map(|(a, b)| a.takeoffs.saturating_sub(b.takeoffs))
        .unwrap_or(0);
    let modes_pulse: Vec<u8> = w.pulse.iter().map(|x| x.mode).collect();
    let mode_set: Vec<u8> = {
        let mut v = modes_pulse.clone();
        v.dedup();
        v
    };

    serde_json::json!({
        "stimulus": {
            "mode": t.motion(),
            "description": t.describe(),
            "palm_half_mm": [t.half[0], t.half[1], t.half[2]],
            "origin_mm": [t.origin[0], t.origin[1], t.origin[2]],
            "contact_mm": [t.target[0], t.target[1], t.target[2]],
            "start_ms": t.start_s * 1000.0,
            "duration_ms": t.dur_s * 1000.0,
            "palm_speed_mm_s": t.speed_mm_s(),
            "approach_dir": [t.axes[2][0], t.axes[2][1], t.axes[2][2]],
            "reflectance": "human skin diffuse reflectance (Dawson/Anderson), scaled by the \
                            palm's ambient shading factor; dark against the room",
            "shading_factor": crate::vision::HAND_SHADING,
        },
        "retina": {
            "note": "the stimulus side: this is the delivery check, and it comes before any \
                     behavioural reading",
            "windows_s": {
                "pre": [t.start_s - 0.5, t.start_s],
                "pulse": [t.start_s, t.start_s + t.dur_s],
                "post": [t.start_s + t.dur_s, t.start_s + t.dur_s + 0.5],
            },
            "samples": {"pre": w.pre.len(), "pulse": w.pulse.len(), "post": w.post.len()},
            "pulse_windows_with_palm_on_retina": w.pulse.iter().filter(|x| x.cols_l + x.cols_r > 0).count(),
            "pct_pulse_windows_with_palm_on_retina":
                100.0 * w.pulse.iter().filter(|x| x.cols_l + x.cols_r > 0).count() as f64 / at(&w.pulse),
            "peak_cols_l": w.pulse.iter().map(|x| x.cols_l).max().unwrap_or(0),
            "peak_cols_r": w.pulse.iter().map(|x| x.cols_r).max().unwrap_or(0),
            "peak_frac_l": w.pulse.iter().map(|x| x.frac_l).fold(0.0f32, f32::max),
            "peak_frac_r": w.pulse.iter().map(|x| x.frac_r).fold(0.0f32, f32::max),
            "peak_frac_of_eye": w.pulse.iter().map(|x| x.frac_l.max(x.frac_r)).fold(0.0f32, f32::max),
            "mean_cols_pulse": mean(&colf(&w.pulse, |x| (x.cols_l + x.cols_r) as f32)),
            "mean_cols_pre": mean(&colf(&w.pre, |x| (x.cols_l + x.cols_r) as f32)),
            "first_seen_ms": first_seen,
            "nearest_palm_surface_mm_over_run": if near_min.is_finite() { near_min } else { -1.0 },
            "nearest_palm_surface_mm_pulse_end":
                w.pulse.last().map(|x| x.hand_surf).unwrap_or(f32::NAN),
            "mean_lum_l": {"pre": mean(&colf(&w.pre, |x| x.lum_l)), "pulse": mean(&colf(&w.pulse, |x| x.lum_l))},
            "mean_lum_r": {"pre": mean(&colf(&w.pre, |x| x.lum_r)), "pulse": mean(&colf(&w.pulse, |x| x.lum_r))},
            "loom_delivered_l": {
                "pre_mean": mean(&colf(&w.pre, |x| x.loom_l)),
                "pulse_mean": mean(&colf(&w.pulse, |x| x.loom_l)),
                "pulse_max": w.pulse.iter().map(|x| x.loom_l).fold(0.0f32, f32::max),
            },
            "loom_delivered_r": {
                "pre_mean": mean(&colf(&w.pre, |x| x.loom_r)),
                "pulse_mean": mean(&colf(&w.pulse, |x| x.loom_r)),
                "pulse_max": w.pulse.iter().map(|x| x.loom_r).fold(0.0f32, f32::max),
            },
            "peak_loom_any_eye": peak_loom,
            "peak_loom_ms": peak_loom_t * 1000.0,
            "first_loom_gt_0p5_ms": first_loom_up,
            "lateral_loom": {
                "pre_mean_abs_L_minus_R": mean(&colf(&w.pre, |x| (x.loom_l - x.loom_r).abs())),
                "pulse_mean_abs_L_minus_R": mean(&colf(&w.pulse, |x| (x.loom_l - x.loom_r).abs())),
                "pulse_max_abs_L_minus_R":
                    w.pulse.iter().map(|x| (x.loom_l - x.loom_r).abs()).fold(0.0f32, f32::max),
            },
            "loom_pool_hz_left": {
                "pre_mean": mean(&colf(&w.pre, |x| x.loom_hz_l)),
                "pulse_mean": mean(&colf(&w.pulse, |x| x.loom_hz_l)),
                "pulse_max": w.pulse.iter().map(|x| x.loom_hz_l).fold(0.0f32, f32::max),
            },
            "loom_pool_hz_right": {
                "pre_mean": mean(&colf(&w.pre, |x| x.loom_hz_r)),
                "pulse_mean": mean(&colf(&w.pulse, |x| x.loom_hz_r)),
                "pulse_max": w.pulse.iter().map(|x| x.loom_hz_r).fold(0.0f32, f32::max),
            },
            "palm_eye_distance_mm": {
                "pulse_min_l": w.pulse.iter().map(|x| x.near_l).fold(f32::INFINITY, f32::min),
                "pulse_min_r": w.pulse.iter().map(|x| x.near_r).fold(f32::INFINITY, f32::min),
            },
        },
        "response": {
            "note": "the response side. Nothing here is a control term: it is a measurement of \
                     what the connectome did while a dark edge expanded over the retina.",
            "turn_away_yaw": sign(&w.pulse, true),
            "turn_away_steer_raw": sign(&w.pulse, false),
            "turn_away_steer_detrended": detrend,
            "turn_away_yaw_pre": sign(&w.pre, true),
            "radial_velocity_away_mm_s": {
                "pulse_mean": mean(&rad_pulse),
                "pulse_median": med(&rad_pulse),
                "pre_mean": mean(&rad_pre),
                "pct_samples_away": 100.0 * n_away_v as f64 / at(&w.pulse),
                "max": rad_pulse.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
                "min": rad_pulse.iter().cloned().fold(f32::INFINITY, f32::min),
            },
            "distance_to_palm_mm": {
                "pulse_first": dist_first,
                "pulse_last": dist_last,
                "pulse_change": dist_last - dist_first,
                "run_min": s.iter().map(|x| x.hand_dist).fold(f32::INFINITY, f32::min),
                "post_last": w.post.last().map(|x| x.hand_dist).unwrap_or(f32::NAN),
            },
            "acceleration": {
                "mean_speed_pre_mm_s": speed_pre,
                "mean_speed_pulse_mm_s": speed_pulse,
                "speed_change_mm_s": speed_pulse - speed_pre,
                "max_speed_pulse_mm_s": w.pulse.iter().map(|x| x.speed).fold(0.0f32, f32::max),
                "mean_speed_post_mm_s": mean(&colf(&w.post, |x| x.speed)),
            },
            "mode_change": {
                "pulse_mode_codes": mode_set,
                "takeoffs_in_pulse": takeoffs_pulse,
                "takeoffs_in_post_500ms": takeoffs_post,
                "takeoffs_whole_run": s.last().map(|x| x.takeoffs).unwrap_or(0),
                "altitude_max_post_mm": w.post.iter().map(|x| x.z).fold(f32::NEG_INFINITY, f32::max),
            },
            "steering_pool_hz": {
                "pre_l": mean(&colf(&w.pre, |x| x.steer_hz_l)),
                "pre_r": mean(&colf(&w.pre, |x| x.steer_hz_r)),
                "pulse_l": mean(&colf(&w.pulse, |x| x.steer_hz_l)),
                "pulse_r": mean(&colf(&w.pulse, |x| x.steer_hz_r)),
                "pre_diff_l_minus_r": mean(&colf(&w.pre, |x| x.steer_hz_l - x.steer_hz_r)),
                "pulse_diff_l_minus_r": mean(&colf(&w.pulse, |x| x.steer_hz_l - x.steer_hz_r)),
            },
            "tot_spikes_pulse": w.pulse.last().map(|x| x.tot_spikes).unwrap_or(0)
                .saturating_sub(w.pulse.first().map(|x| x.tot_spikes).unwrap_or(0)),
        },
    })
}