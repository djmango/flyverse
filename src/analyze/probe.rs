//! Rotation perturbation probe.
//!
//! The on/off comparison in the main analysis cannot settle whether the haltere
//! afferents do anything, because the system is chaotic: two runs differing in
//! any input diverge, and that divergence alone produces differences of the size
//! observed. This probe answers the question a different way.
//!
//! The body is given an angular velocity it did not generate, and the same
//! perturbation is applied with both signs. The network state at the moment of
//! perturbation is not controlled, so the sign is drawn at random and the
//! response is averaged separately for each sign. Chaotic variation is
//! independent of the sign and averages out; a reflex is a function of the sign
//! and survives. That sign-conditioned difference is the measurement.
//!
//! A second design point: the probe only ever touches the body's angular
//! velocity. It never injects spikes or alters the stimulus, so any motor
//! response has to travel through the connectome and back out.

use crate::lif::DT_MS;
use crate::sim::{World, WINDOW_S};
use anyhow::Result;
use std::path::Path;

use super::stats::{balanced_sign_schedule, mean64, sem64, welch_t};
use super::trace::Options;

/// Parameters for the haltere reflex probe.
pub struct ProbeOptions {
    /// Simulated seconds before the first perturbation, so the fly is in its own
    /// behaviour rather than at the spawn state.
    pub warmup_s: f64,
    /// Total perturbations. Split evenly across the probed axes.
    pub trials: u32,
    /// Simulated seconds between perturbations, so one has decayed first.
    pub interval_s: f64,
    /// Response window length, in milliseconds.
    pub response_ms: f64,
    /// Magnitude of the imposed angular velocity, rad/s.
    pub amplitude: f32,
}

/// Yaw then roll. Yaw drives the two halteres in common, roll drives them
/// differentially, which is the split the real pair gives.
const PROBED_AXES: [usize; 2] = [2, 0];
const AXIS_NAME: [&str; 3] = ["roll", "pitch", "yaw"];

pub fn rotation_probe(pack: &Path, o: &Options, p: &ProbeOptions) -> Result<()> {
    let mut w = World::new(pack, o.seed, None)?;
    let resp_windows = (((p.response_ms / 1000.0) / WINDOW_S as f64).round() as usize).max(1);
    let iti_windows = ((p.interval_s / WINDOW_S as f64) as u64).max(1);
    let warmup_windows = ((p.warmup_s / WINDOW_S as f64) as u64).max(1);

    println!(
        "probe: haltere afferents {}, optic flow {}",
        if w.haltere_on {
            "CONNECTED"
        } else {
            "DISCONNECTED (control)"
        },
        // Report *effective* delivery, not the env flag: when the retina is on
        // the gating in `step` zeroes this channel, so `flow_on` alone would
        // claim a visual drive that is not actually reaching the brain.
        if w.flow_on && !w.retina.on {
            "delivered"
        } else if w.flow_on {
            "SUPERSEDED (retina is the visual drive)"
        } else {
            "SILENCED (control)"
        }
    );
    // Record which visual channel is live. Since the retina drives the same
    // tangential cells the optic-flow proxy used to, a probe result is only
    // interpretable alongside this line.
    println!(
        "probe: retina {}",
        if w.retina.on {
            format!("SAMPLING THE ROOM ({} columns, {} photoreceptors)",
                w.retina.columns(), w.retina.photons)
        } else {
            "SILENCED (control) -- optic-flow proxy is the visual drive".to_string()
        }
    );
    println!(
        "probe: {} trials, warmup {:.0}s, interval {:.1}s, response {:.0}ms, amplitude {:.0} rad/s",
        p.trials, p.warmup_s, p.interval_s, p.response_ms, p.amplitude
    );

    let t_wall0 = std::time::Instant::now();
    for _ in 0..warmup_windows {
        w.advance();
    }
    println!(
        "probe: warmup done at t={:.1}s, altitude {:.1} mm, haltere {:.1}/{:.1} Hz, mode {}",
        w.step as f64 * DT_MS as f64 / 1000.0,
        w.body.pos[2],
        w.body.haltere_l,
        w.body.haltere_r,
        w.body.mode.as_str()
    );

    // Per axis, per sign, per channel: one scalar per trial, plus the averaged
    // time course. Indexed [ai * 2 + sign][channel].
    let nch = 4; // steer differential, power differential, yaw rate, lift ratio
    let mut scalars: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); nch]; PROBED_AXES.len() * 2];
    let mut course: Vec<Vec<Vec<f64>>> =
        vec![vec![vec![0.0; nch]; resp_windows]; PROBED_AXES.len() * 2];
    let mut hal_on_onset: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 2]; PROBED_AXES.len()];
    let mut yaw_peak: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 2]; PROBED_AXES.len()];

    // Balanced sign schedule, one list per axis, shuffled so the sign is not
    // correlated with the fly's own behaviour at that instant. Half of each
    // axis's trials go positive and half negative, so the two group means are
    // compared with equal weight regardless of how few trials there are.
    let per_axis = p.trials.div_ceil(PROBED_AXES.len() as u32) as usize;
    let schedule = balanced_sign_schedule(PROBED_AXES.len(), per_axis, o.seed);

    let mut done = 0u32;
    for trial in 0..p.trials {
        let ai = (trial as usize) % PROBED_AXES.len();
        let k = trial as usize / PROBED_AXES.len();
        let axis = PROBED_AXES[ai];
        let positive = schedule[ai][k];
        let sign = if positive { 1.0f32 } else { -1.0f32 };
        let si = if positive { 0 } else { 1 };

        if trial > 0 {
            for _ in 0..iti_windows {
                w.advance();
            }
        }

        w.inject_rotation(axis, sign * p.amplitude);
        // The afferent rate is read one window later, because the body computes
        // it from the rotation that was just imposed.
        w.advance();
        let (hl, hr) = w.haltere_rates();
        hal_on_onset[ai][si].push((hl as f64 + hr as f64) * 0.5);

        let mut ch = vec![0.0f64; nch];
        let mut peak = 0.0f64;
        for r in 0..resp_windows {
            w.advance();
            let m = w.motors();
            let d_steer = (m.flight_steer_r - m.flight_steer_l) as f64;
            let d_power = (m.flight_power_r - m.flight_power_l) as f64;
            let yaw_rate = w.yaw_rate_rad_s() as f64;
            let lift = (w.body.wing_lift() / w.body.weight()) as f64;
            let v = [d_steer, d_power, yaw_rate, lift];
            for c in 0..nch {
                course[ai * 2 + si][r][c] += v[c];
                ch[c] += v[c];
            }
            if yaw_rate.abs() > peak {
                peak = yaw_rate.abs();
            }
        }
        for c in 0..nch {
            scalars[ai * 2 + si][c].push(ch[c] / resp_windows as f64);
        }
        yaw_peak[ai][si].push(peak);
        done += 1;
        if done % 8 == 0 {
            println!(
                "  {}/{} trials, wall {:.0}s",
                done,
                p.trials,
                t_wall0.elapsed().as_secs_f64()
            );
        }
    }

    let channel_names = [
        "steer differential (R-L)",
        "power differential (R-L)",
        "yaw rate rad/s",
        "lift / weight",
    ];

    println!("\n=== ROTATION PERTURBATION PROBE ===");
    println!(
        "Imposed angular velocity: {} rad/s, both signs, sign drawn at random.",
        p.amplitude
    );
    println!("Response window: {:.0} ms after onset.\n", p.response_ms);

    let mut out = serde_json::Map::new();
    out.insert("amplitude_rad_s".into(), p.amplitude.into());
    out.insert("response_ms".into(), p.response_ms.into());
    out.insert("warmup_s".into(), p.warmup_s.into());
    out.insert("trials_total".into(), p.trials.into());
    out.insert("haltere_connected".into(), w.haltere_on.into());
    out.insert("retina_on".into(), w.retina.on.into());
    out.insert("retina_columns".into(), w.retina.columns().into());
    out.insert("retina_photoreceptors".into(), w.retina.photons.into());
    // Effective delivery, not the env flag. The gating in `step` zeroes the
    // proxy whenever the retina is on, so `flow_on` alone would archive a
    // visual drive that never reached the brain.
    out.insert(
        "optic_flow_proxy_delivered".into(),
        (w.flow_on && !w.retina.on).into(),
    );
    out.insert("seed".into(), o.seed.into());
    let mut axes_out = serde_json::Map::new();

    for (ai, &axis) in PROBED_AXES.iter().enumerate() {
        let name = AXIS_NAME[axis];
        println!("--- {} perturbation ---", name.to_uppercase());
        let nplus = scalars[ai * 2][0].len();
        let nminus = scalars[ai * 2 + 1][0].len();
        println!("  trials: {} positive, {} negative", nplus, nminus);

        let onset_p = mean64(&hal_on_onset[ai][0]);
        let onset_m = mean64(&hal_on_onset[ai][1]);
        println!(
            "  haltere afferent at onset: {:.1} Hz (+) vs {:.1} Hz (-), mean of both sides",
            onset_p, onset_m
        );
        let yp = mean64(&yaw_peak[ai][0]);
        let ym = mean64(&yaw_peak[ai][1]);
        println!("  peak |yaw rate| reached: {:.1} vs {:.1} rad/s", yp, ym);

        // Time course of the steering differential, averaged over the trials of
        // each sign. A reflex is a separation between these two curves that
        // grows over the tens of milliseconds after onset.
        let cplus = &course[ai * 2];
        let cminus = &course[ai * 2 + 1];
        let np = nplus.max(1) as f64;
        let nm = nminus.max(1) as f64;
        println!("  steer differential (R-L) by lag:");
        let mut k = 0usize;
        while k < resp_windows {
            let lag = (k as f64 + 1.0) * WINDOW_S as f64 * 1000.0;
            let a = cplus[k][0] / np;
            let b = cminus[k][0] / nm;
            println!("    {:>5.0} ms   +{:+.5}   -{:+.5}   diff {:+.5}", lag, a, b, a - b);
            k += 10;
        }

        let mut ch_out = serde_json::Map::new();
        for c in 0..nch {
            let a = &scalars[ai * 2][c];
            let b = &scalars[ai * 2 + 1][c];
            let (ma, mb) = (mean64(a), mean64(b));
            let t = welch_t(a, b);
            println!(
                "  {:<26} + {:>9.4} +/- {:.4}   - {:>9.4} +/- {:.4}   diff {:>9.4}   t {:>5.2}",
                channel_names[c],
                ma,
                sem64(a),
                mb,
                sem64(b),
                ma - mb,
                t
            );
            let mut m = serde_json::Map::new();
            m.insert("plus_mean".into(), ma.into());
            m.insert("plus_sem".into(), sem64(a).into());
            m.insert("minus_mean".into(), mb.into());
            m.insert("minus_sem".into(), sem64(b).into());
            m.insert("difference".into(), (ma - mb).into());
            m.insert("welch_t".into(), t.into());
            ch_out.insert(channel_names[c].into(), m.into());
        }
        println!();
        axes_out.insert(name.into(), ch_out.into());
    }
    out.insert("axes".into(), axes_out.into());

    let dir = o.out.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(&serde_json::Value::Object(out))?;
    std::fs::write(o.out.with_extension("json"), json)?;
    println!(
        "probe: {} trials in {:.0}s wall; wrote {}",
        p.trials,
        t_wall0.elapsed().as_secs_f64(),
        o.out.with_extension("json").display()
    );
    Ok(())
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    /// The sign schedule must give the two groups equal size. An unbalanced draw
    /// would weight one sign more than the other, and the whole measurement is a
    /// difference of the two means.
    #[test]
    fn sign_schedule_is_balanced_within_tolerance() {
        for seed in [1u64, 7, 99, 12345] {
            let s = balanced_sign_schedule(PROBED_AXES.len(), 30, seed);
            assert_eq!(s.len(), PROBED_AXES.len());
            for axis in &s {
                assert_eq!(axis.len(), 30);
                let pos = axis.iter().filter(|b| **b).count();
                assert_eq!(pos, 15, "seed {seed} gave {pos} positive of 30");
            }
        }
    }

    /// The shuffle has to actually move things, otherwise the sign would follow a
    /// fixed alternating pattern and could correlate with anything periodic in
    /// the fly's own behaviour.
    #[test]
    fn sign_schedule_is_shuffled_not_alternating() {
        let s = balanced_sign_schedule(1, 30, 7);
        let alternating = (0..30).map(|i| i % 2 == 0).collect::<Vec<_>>();
        assert_ne!(s[0], alternating);
    }

    /// And it has to be reproducible from the seed, or a probe run could not be
    /// repeated.
    #[test]
    fn sign_schedule_is_deterministic() {
        let a = balanced_sign_schedule(2, 20, 7);
        let b = balanced_sign_schedule(2, 20, 7);
        assert_eq!(a, b);
        let c = balanced_sign_schedule(2, 20, 8);
        assert_ne!(a, c);
    }

    /// No signal means no statistic. If the two groups are drawn from the same
    /// distribution, welch_t must not manufacture a difference.
    #[test]
    fn welch_t_is_zero_for_identical_samples() {
        let v: Vec<f64> = (0..40).map(|i| ((i * 37) % 11) as f64).collect();
        assert!(welch_t(&v, &v) < 1e-9);
    }

    /// A real shift has to show up. This is the property the probe relies on:
    /// a consistent sign-dependent response survives the averaging.
    #[test]
    fn welch_t_detects_a_consistent_shift() {
        let a: Vec<f64> = (0..40).map(|i| 1.0 + ((i * 37) % 11) as f64 * 0.01).collect();
        let b: Vec<f64> = (0..40).map(|i| 0.0 + ((i * 37) % 11) as f64 * 0.01).collect();
        assert!(welch_t(&a, &b) > 5.0);
    }

    /// sem must be finite for two samples and must fall as samples accumulate.
    #[test]
    fn sem_shrinks_with_more_samples() {
        let few: Vec<f64> = vec![0.0, 1.0];
        let many: Vec<f64> = (0..64).map(|i| (i % 2) as f64).collect();
        assert!(sem64(&few).is_finite());
        assert!(sem64(&many) < sem64(&few));
    }
}