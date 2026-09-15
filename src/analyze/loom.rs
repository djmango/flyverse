//! Open-loop loom probe.
//!
//! The closed loop cannot answer whether the visual steering pathway works.
//! There the loom channel is ONE geometric scalar (time-to-contact with the
//! nearest wall face) and `sim.rs::sense` delivers it at the SAME rate to both
//! `visual_loom_left` and `visual_loom_right`, so the channel carries no
//! left/right differential by construction -- and because the fly is pinned to
//! the walls by its own standing turn, it is also saturated (85.3 % of airborne
//! samples above 0.9). A constant carries no information, so "the loom does not
//! steer the fly" is untestable in that loop: the stimulus had nothing to say.
//!
//! This probe imposes the stimulus instead. A fixed (left, right) pair is held
//! across the response window, and two things are varied:
//!
//! * **sign** -- which eye is stimulated. Left-only and right-only are the two
//!   signs of a lateralised looming cue; the pair is compared directly, and
//!   each is compared against the null.
//! * **rate** -- how hard, as a multiple of the closed loop's own mapping
//!   (1.0 = 60 Hz per neuron, its maximum). A flat dose-response is as
//!   informative as a rising one.
//!
//! A third condition, both eyes at the same rate, reproduces the closed loop's
//! own symmetric pattern: it is non-lateral by construction, so if it moves the
//! differential as much as a one-eyed stimulus, nothing measured here is a
//! lateral response.
//!
//! **Null control.** The same protocol with the loom drives at zero. Trials are
//! interleaved in a shuffled, seeded order so the null carries the same chaotic
//! variation and the same stage of the fly's own behaviour as the stimulated
//! trials; the statistic is a Welch t of the per-trial means. The repo's
//! established probe floor is |t| ~ 2.3, and an earlier haltere probe found no
//! channel cleared it -- the bar is not lowered here.
//!
//! **Positive control.** The two `visual_loom` pools are single neurons
//! (MeVP24, one per side), so their firing rate is a direct read-out of whether
//! the imposed stimulus arrived at the sense organ at all. That column is
//! reported for every condition. Without it a flat motor response could not be
//! told apart from a stimulus that never landed.

use crate::lif::DT_MS;
use crate::sim::{World, WINDOW_S};
use anyhow::Result;
use std::path::Path;

use super::stats::{mean64, next_u64, sem64, welch_t};
use super::trace::Options;

/// Parameters for the open-loop loom probe.
pub struct LoomProbeOptions {
    /// Simulated seconds before the first trial, so the fly is in its own
    /// behaviour rather than at the spawn state.
    pub warmup_s: f64,
    /// Trials per condition. Split evenly, and shuffled.
    pub trials_per_condition: u32,
    /// Simulated seconds between trials, with the closed-loop loom restored, so
    /// one stimulus has decayed before the next.
    pub interval_s: f64,
    /// Response window length, in milliseconds.
    pub response_ms: f64,
    /// Stimulus levels, in loop units: a level `v` drives the pool at
    /// `60 * v` Hz per neuron. 1.0 is the closed loop's maximum (a saturated
    /// loom).
    pub levels: Vec<f32>,
}

/// Which pools are stimulated, and how hard.
struct Condition {
    name: String,
    left_hz: f64,
    right_hz: f64,
    /// Per-trial scalar means, indexed [channel][trial].
    scalars: Vec<Vec<f64>>,
    /// Per-trial mean firing rate of the two `visual_loom` pools, [side][trial].
    loom_hz: Vec<Vec<f64>>,
    /// Fraction of the response window spent airborne, [trial].
    air: Vec<f64>,
}

const CHANNEL_NAMES: [&str; 8] = [
    "steer differential (R-L)",
    "steer left",
    "steer right",
    "yaw rate rad/s",
    "|yaw rate| rad/s",
    "lift / weight",
    "visual_loom_left Hz",
    "visual_loom_right Hz",
];
const NCH: usize = 8;

/// Member count of a named pool, or a marker if the annotation has no such
/// group -- an absent stimulus pool must be visible, not silently zero.
fn loom_pool_size(w: &World, name: &str) -> String {
    let gi = w.groups.idx(name);
    if gi == usize::MAX {
        "ABSENT".to_string()
    } else {
        format!("{} neurons", w.groups.group_size[gi])
    }
}

pub fn loom_probe(pack: &Path, o: &Options, p: &LoomProbeOptions) -> Result<()> {
    let mut w = World::new(pack, o.seed, None)?;
    let resp_windows = (((p.response_ms / 1000.0) / WINDOW_S as f64).round() as usize).max(1);
    let iti_windows = ((p.interval_s / WINDOW_S as f64) as u64).max(1);
    let warmup_windows = ((p.warmup_s / WINDOW_S as f64) as u64).max(1);
    let levels: Vec<f32> = p.levels.iter().copied().filter(|v| *v > 0.0).collect();

    println!(
        "loom probe: arena {:.0} x {:.0} x {:.0} mm, x {:.0}..{:.0}, y {:.0}..{:.0}, z {:.0}..{:.0}",
        w.room.x[1] - w.room.x[0],
        w.room.y[1] - w.room.y[0],
        w.room.z[1] - w.room.z[0],
        w.room.x[0],
        w.room.x[1],
        w.room.y[0],
        w.room.y[1],
        w.room.z[0],
        w.room.z[1]
    );
    println!(
        "loom probe: visual_loom pools: left {}, right {} (driven at 60 Hz * level); retina {}",
        loom_pool_size(&w, "visual_loom_left"),
        loom_pool_size(&w, "visual_loom_right"),
        if w.retina.on { "ON" } else { "silenced" }
    );

    // Conditions: the null first, then per level the two signs and the
    // symmetric (closed-loop-shaped) pattern.
    let mut conds: Vec<Condition> = vec![Condition {
        name: "null: loom 0 / 0".to_string(),
        left_hz: 0.0,
        right_hz: 0.0,
        scalars: vec![Vec::new(); NCH],
        loom_hz: vec![Vec::new(); 2],
        air: Vec::new(),
    }];
    for &v in &levels {
        let hz = 60.0 * v as f64;
        for (tag, l, r) in [
            ("left eye", v, 0.0f32),
            ("right eye", 0.0f32, v),
            ("both eyes (symmetric)", v, v),
        ] {
            conds.push(Condition {
                name: format!("{tag} {hz:.0} Hz (level {v:.2})"),
                left_hz: l as f64 * 60.0,
                right_hz: r as f64 * 60.0,
                scalars: vec![Vec::new(); NCH],
                loom_hz: vec![Vec::new(); 2],
                air: Vec::new(),
            });
        }
    }
    let n_cond = conds.len();
    let per = p.trials_per_condition.max(1) as usize;

    // Shuffled, balanced order: every condition gets `per` trials, and the
    // order is drawn from the seed so the stimulus is not correlated with the
    // stage of the fly's own behaviour.
    let mut order: Vec<usize> = (0..n_cond * per).map(|i| i % n_cond).collect();
    let mut rng = o.seed ^ 0x51ED_2701_9E37_79B9;
    for i in (1..order.len()).rev() {
        let j = (next_u64(&mut rng) % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }

    println!(
        "loom probe: {} conditions x {} trials = {} trials, warmup {:.0}s, interval {:.2}s, \
         response {:.0}ms ({} windows)",
        n_cond,
        per,
        order.len(),
        p.warmup_s,
        p.interval_s,
        p.response_ms,
        resp_windows
    );

    let t_wall0 = std::time::Instant::now();
    for _ in 0..warmup_windows {
        w.advance();
    }
    println!(
        "loom probe: warmup done at t={:.1}s, altitude {:.1} mm, speed {:.1} mm/s, mode {}, \
         closed-loop loom {:.3}",
        w.step as f64 * DT_MS as f64 / 1000.0,
        w.body.pos[2],
        w.body.speed(),
        w.body.mode.as_str(),
        w.loom_delivered()
    );

    for (t, &ci) in order.iter().enumerate() {
        let c = &mut conds[ci];
        if t > 0 {
            w.clear_imposed_loom();
            for _ in 0..iti_windows {
                w.advance();
            }
        }
        // Impose the stimulus. The response window starts with the first
        // window in which it is in effect, so no conduction delay is baked in.
        w.set_imposed_loom(c.left_hz as f32 / 60.0, c.right_hz as f32 / 60.0);
        let mut acc = [0.0f64; NCH];
        let mut lm = [0.0f64; 2];
        let mut air = 0.0f64;
        for _ in 0..resp_windows {
            w.advance();
            let m = w.motors();
            let d_steer = (m.flight_steer_r - m.flight_steer_l) as f64;
            let yaw_rate = w.yaw_rate_rad_s() as f64;
            let lift = (w.body.wing_lift() / w.body.weight()) as f64;
            let v = [
                d_steer,
                m.flight_steer_l as f64,
                m.flight_steer_r as f64,
                yaw_rate,
                yaw_rate.abs(),
                lift,
                0.0,
                0.0,
            ];
            for k in 0..6 {
                acc[k] += v[k];
            }
            lm[0] += w.group_hz("visual_loom_left") as f64;
            lm[1] += w.group_hz("visual_loom_right") as f64;
            if matches!(
                w.body.mode,
                crate::body::Mode::Takeoff | crate::body::Mode::Cruise | crate::body::Mode::Landing
            ) {
                air += 1.0;
            }
        }
        let d = resp_windows as f64;
        // Channels 0..6 are window means already. The two loom-pool rates are
        // pushed straight from their own accumulator: dividing them a second
        // time by `d` (as the shared `acc` path would) is what put the JSON's
        // control column out by a factor of `resp_windows`.
        for k in 0..6 {
            c.scalars[k].push(acc[k] / d);
        }
        c.scalars[6].push(lm[0] / d);
        c.scalars[7].push(lm[1] / d);
        c.loom_hz[0].push(lm[0] / d);
        c.loom_hz[1].push(lm[1] / d);
        c.air.push(air / d);

        if (t + 1) % 12 == 0 {
            println!(
                "  {}/{} trials, wall {:.0}s",
                t + 1,
                order.len(),
                t_wall0.elapsed().as_secs_f64()
            );
        }
    }
    w.clear_imposed_loom();

    // ---------------------------------------------------------------- report
    println!("\n=== OPEN-LOOP LOOM PROBE ===");
    println!(
        "Imposed looming stimulus, both signs and several rates, held for {:.0} ms. \
         Probe floor is |t| ~ 2.3.",
        p.response_ms
    );
    println!(
        "Every condition is compared with the null by Welch t on the per-trial means; \
         a condition is a response only if it clears the floor.\n"
    );

    let nul = &conds[0];
    println!(
        "{:<34} {:>7} {:>10} {:>9} {:>9} {:>8} {:>8}",
        "condition", "air%", "loomL Hz", "loomR Hz", "steerR-L", "yaw r/s", "|yaw|"
    );
    for c in &conds {
        println!(
            "{:<34} {:>6.1} {:>10.1} {:>10.1} {:>9.4} {:>9.4} {:>8.3}",
            c.name,
            100.0 * mean64(&c.air),
            mean64(&c.loom_hz[0]),
            mean64(&c.loom_hz[1]),
            mean64(&c.scalars[0]),
            mean64(&c.scalars[3]),
            mean64(&c.scalars[4]),
        );
    }

    println!("\nt statistic vs the null control (|t| > 2.3 is the repo's floor):");
    println!(
        "{:<34} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "condition",
        "steer d",
        "steer L",
        "steer R",
        "yaw",
        "|yaw|",
        "lift/w",
        "loomL",
        "loomR"
    );
    // Only the motor / behavioural channels count as a response. The two loom
    // pool rates are the positive control -- they say whether the stimulus
    // arrived -- and they are expected to clear the floor by construction, so
    // including them would make the verdict trivially positive.
    const MOTOR_CH: [usize; 6] = [0, 1, 2, 3, 4, 5];
    let mut best_t = 0.0f64;
    let mut best_where = String::new();
    let mut stimulated = 0usize;
    for c in &conds {
        print!("{:<34}", c.name);
        for k in [0usize, 1, 2, 3, 4, 5, 6, 7] {
            let t = welch_t(&c.scalars[k], &nul.scalars[k]);
            print!(" {:>8.2}", t);
            if c.name != nul.name && MOTOR_CH.contains(&k) {
                stimulated += 1;
                if t > best_t {
                    best_t = t;
                    best_where = format!("{} / {}", c.name, CHANNEL_NAMES[k]);
                }
            }
        }
        println!();
    }

    // The lateral contrast: the two signs directly against each other. This is
    // the measurement the closed loop cannot make, because there the two pools
    // receive the same rate.
    println!("\nlateral contrast (one eye vs the other), by stimulus level:");
    println!(
        "{:<10} {:>10} {:>10} {:>9} {:>9} {:>9} {:>9}",
        "level", "L steerR-L", "R steerR-L", "diff", "t(steer)", "L yaw", "R yaw"
    );
    let mut level_rows: Vec<serde_json::Value> = Vec::new();
    for (li, &v) in levels.iter().enumerate() {
        let cl = &conds[1 + li * 3];
        let cr = &conds[2 + li * 3];
        let dl = mean64(&cl.scalars[0]);
        let dr = mean64(&cr.scalars[0]);
        let t = welch_t(&cl.scalars[0], &cr.scalars[0]);
        println!(
            "{:<10} {:>10.4} {:>10.4} {:>9.4} {:>9.2} {:>9.4} {:>9.4}",
            format!("{v:.2}"),
            dl,
            dr,
            dl - dr,
            t,
            mean64(&cl.scalars[3]),
            mean64(&cr.scalars[3])
        );
        level_rows.push(serde_json::json!({
            "level": v,
            "hz": 60.0 * v,
            "left_eye_steer_differential_mean": dl,
            "right_eye_steer_differential_mean": dr,
            "difference": dl - dr,
            "welch_t": welch_t(&cl.scalars[0], &cr.scalars[0]),
            "left_eye_yaw_rate_mean": mean64(&cl.scalars[3]),
            "right_eye_yaw_rate_mean": mean64(&cr.scalars[3]),
            "left_eye_loom_pool_hz": mean64(&cl.loom_hz[0]),
            "right_eye_loom_pool_hz": mean64(&cr.loom_hz[1]),
        }));
    }

    println!(
        "\nlargest |t| of any stimulated condition against the null: {:.2} ({})",
        best_t, best_where
    );

    // The floor, measured in this design rather than assumed. The haltere probe
    // established that a null-vs-null comparison reaches |t| ~ 2.3 on one channel
    // with no perturbation at all, because the two groups are different trials at
    // different points in a chaotic run. The same check is cheap here: split the
    // null's own trials in half and compare the halves. `null_floor` is what
    // this probe can achieve with no stimulus, and it is the bar a condition has
    // to beat.
    let mut half: Vec<Vec<f64>> = vec![Vec::new(); NCH];
    let mut other: Vec<Vec<f64>> = vec![Vec::new(); NCH];
    let mut floor_t = 0.0f64;
    let mut floor_where = String::new();
    for (k, name) in CHANNEL_NAMES.iter().enumerate() {
        for (i, v) in nul.scalars[k].iter().enumerate() {
            if i % 2 == 0 {
                half[k].push(*v);
            } else {
                other[k].push(*v);
            }
        }
        let t = welch_t(&half[k], &other[k]);
        if t > floor_t {
            floor_t = t;
            floor_where = name.to_string();
        }
    }
    println!(
        "null-vs-null floor measured in this probe: |t| {:.2} ({}). {} stimulated conditions x 6 \
         motor channels = {} comparisons were made, so a maximum near this floor is what chance \
         alone produces.",
        floor_t, floor_where, conds.len() - 1, stimulated
    );
    if best_t < floor_t {
        println!(
            "VERDICT: no stimulated condition reaches even the measured no-stimulus floor \
             ({:.2} < {:.2}). Within this probe's power the imposed looming stimulus does not \
             reach the steering motor output.",
            best_t, floor_t
        );
    } else if best_t < 2.3 {
        println!(
            "VERDICT: no condition reaches the |t| ~ 2.3 floor (best {:.2}). Within this \
             probe's power the imposed looming stimulus does not reach the steering motor output.",
            best_t
        );
    } else {
        println!(
            "VERDICT: {} reaches |t| {:.2}, against a measured no-stimulus floor of {:.2}. Check \
             it against the number of comparisons above before reading it as a response.",
            best_where, best_t, floor_t
        );
    }

    // ------------------------------------------------------------- artifacts
    let mut conditions_json = serde_json::Map::new();
    for c in &conds {
        let mut ch = serde_json::Map::new();
        for (k, name) in CHANNEL_NAMES.iter().enumerate() {
            ch.insert(
                name.to_string(),
                serde_json::json!({
                    "mean": mean64(&c.scalars[k]),
                    "sem": sem64(&c.scalars[k]),
                    "n": c.scalars[k].len(),
                    "welch_t_vs_null": welch_t(&c.scalars[k], &nul.scalars[k]),
                }),
            );
        }
        ch.insert(
            "airborne_fraction".to_string(),
            serde_json::json!({ "mean": mean64(&c.air) }),
        );
        conditions_json.insert(c.name.clone(), serde_json::Value::Object(ch));
    }
    let out = serde_json::json!({
        "seed": o.seed,
        "response_ms": p.response_ms,
        "interval_s": p.interval_s,
        "warmup_s": p.warmup_s,
        "trials_per_condition": per,
        "levels": levels,
        "null_condition": nul.name,
        "arena_mm": {
            "x": [w.room.x[0], w.room.x[1]],
            "y": [w.room.y[0], w.room.y[1]],
            "z": [w.room.z[0], w.room.z[1]],
        },
        "retina_on": w.retina.on,
        "conditions": conditions_json,
        "lateral_contrast_by_level": level_rows,
        "probe_floor_abs_t": 2.3,
        "null_vs_null_floor": {
            "abs_t": floor_t,
            "channel": floor_where,
            "note": "the largest |t| the null reaches against itself, with no stimulus at all; \
                     the same sampling floor the haltere probe measured at ~2.3",
        },
        "comparisons_stimulated_motor": stimulated,
        "max_abs_t_vs_null": best_t,
        "max_abs_t_vs_null_where": best_where,
    });
    let dir = o.out.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        o.out.with_extension("json"),
        serde_json::to_string_pretty(&out)?,
    )?;
    println!(
        "loom probe: {} trials in {:.0}s wall; wrote {}",
        order.len(),
        t_wall0.elapsed().as_secs_f64(),
        o.out.with_extension("json").display()
    );
    Ok(())
}