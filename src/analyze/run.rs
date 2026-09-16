//! The main headless run: close the loop, record a trace, write the artifacts.

use crate::lif::DT_MS;
use crate::sim::{World, WINDOW_S};
use anyhow::Result;
use std::io::Write;
use std::path::Path;

use super::report::{headline, write_csv, write_report};
use super::summary::summarize;
use super::trace::{sample_of, Options, Sample};

// ---------------------------------------------------------------------------
// Run

pub fn run(pack: &Path, o: &Options) -> Result<()> {
    let t_wall0 = std::time::Instant::now();
    let mut w = World::new(pack, o.seed, None)?;
    let total_windows = ((o.seconds / WINDOW_S as f64) as u64).max(1);
    let stride = o.sample_every.max(1);

    println!(
        "analyze: {} neurons, {} edges, {} stimulus targets at {} Hz",
        w.conn.n, w.conn.m, w.vnc_targets, w.vnc_hz
    );
    println!(
        "analyze: motor-neuron calibration: per-cell input gain {:.4} on {} flight power MN cells \
         ({}), every other neuron 1.0 -- see sim::MN_POWER_INPUT_GAIN",
        w.mn_gain,
        w.mn_cells,
        w.mn_members
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    println!(
        "analyze: {} s of simulated time = {} control windows of {:.1} ms (sample every {})",
        o.seconds,
        total_windows,
        WINDOW_S * 1000.0,
        stride
    );

    let mut samples: Vec<Sample> = Vec::with_capacity((total_windows / stride + 2) as usize);
    // The hand's own trace, only when there is a hand in the room. Kept out of
    // `Sample` so `trace.csv` keeps its schema and a run with no hand is
    // byte-identical to the pre-hand build.
    let mut hand_samples: Vec<super::hand::HandSample> =
        Vec::with_capacity((total_windows / stride + 2) as usize);
    for k in 0..total_windows {
        w.advance();
        if k % stride == 0 {
            samples.push(sample_of(&w));
            if let Some(hs) = super::hand::sample_of_hand(&w) {
                hand_samples.push(hs);
            }
        }
        if total_windows >= 20_000 && k % (total_windows / 10).max(1) == 0 {
            print!(
                "  {:.0}%  t={:.1}s wall={:.0}s\r",
                k as f64 / total_windows as f64 * 100.0,
                k as f64 * WINDOW_S as f64,
                t_wall0.elapsed().as_secs_f64()
            );
            let _ = std::io::stdout().flush();
        }
    }
    samples.push(sample_of(&w));
    if let Some(hs) = super::hand::sample_of_hand(&w) {
        hand_samples.push(hs);
    }
    let wall_seconds = t_wall0.elapsed().as_secs_f64();
    let sim_seconds = w.step as f64 * DT_MS as f64 / 1000.0;
    println!(
        "\nanalyze: ran {} steps in {:.1}s wall ({:.0} steps/s, {:.3}x realtime)",
        w.step,
        wall_seconds,
        w.step as f64 / wall_seconds,
        sim_seconds / wall_seconds
    );

    // The hand's block: the retinal/loom delivery check and the escape
    // measurement, both from the run that just happened. `None` when no hand was
    // in the room, which is the case that leaves the summary byte-identical.
    let hand_block = w
        .hand_traj
        .filter(|_| !hand_samples.is_empty())
        .map(|t| {
            let mut v = super::hand::analyze(&hand_samples, &t);
            // The trajectory as it actually ran -- the aim point is re-taken at
            // launch, so the run output is the only place the flown geometry is
            // exact.
            v["trajectory"] = super::hand::aim_block(&w);
            v
        });

    let summary = summarize(&samples, &w, sim_seconds, wall_seconds, o, hand_block);
    std::fs::create_dir_all(&o.out)?;
    write_csv(&o.out.join("trace.csv"), &samples)?;
    std::fs::write(o.out.join("summary.json"), serde_json::to_string_pretty(&summary)?)?;
    write_report(&o.out.join("report.html"), &samples, &summary)?;
    if let Some(h) = summary.get("hand") {
        super::hand::write_csv(&o.out.join("hand.csv"), &hand_samples)?;
        std::fs::write(o.out.join("hand.json"), serde_json::to_string_pretty(h)?)?;
        println!("\n{}", super::hand::headline(h));
        println!(
            "\nwrote {}/hand.csv, hand.json",
            o.out.display()
        );
    }

    println!("\n{}", headline(&summary));
    println!("\nwrote {}/trace.csv, summary.json, report.html", o.out.display());

    // Measurement affordance, off by default: FLYVERSE_ATTRACTOR=1 re-reads the
    // trace just written and reports where the fly ended up relative to the
    // fixed food position and whether it was steering by the odour gradient.
    // With the flag unset this is not compiled out but is never called, so a
    // normal run's output is unchanged.
    if std::env::var("FLYVERSE_ATTRACTOR").is_ok() {
        super::attractor::report(&samples, &w);
    }
    Ok(())
}