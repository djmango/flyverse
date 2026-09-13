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
        "analyze: {} neurons, {} edges, {} stimulus targets at 150 Hz",
        w.conn.n, w.conn.m, w.vnc_targets
    );
    println!(
        "analyze: {} s of simulated time = {} control windows of {:.1} ms (sample every {})",
        o.seconds,
        total_windows,
        WINDOW_S * 1000.0,
        stride
    );

    let mut samples: Vec<Sample> = Vec::with_capacity((total_windows / stride + 2) as usize);
    for k in 0..total_windows {
        w.advance();
        if k % stride == 0 {
            samples.push(sample_of(&w));
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
    let wall_seconds = t_wall0.elapsed().as_secs_f64();
    let sim_seconds = w.step as f64 * DT_MS as f64 / 1000.0;
    println!(
        "\nanalyze: ran {} steps in {:.1}s wall ({:.0} steps/s, {:.3}x realtime)",
        w.step,
        wall_seconds,
        w.step as f64 / wall_seconds,
        sim_seconds / wall_seconds
    );

    let summary = summarize(&samples, &w, sim_seconds, wall_seconds, o);
    std::fs::create_dir_all(&o.out)?;
    write_csv(&o.out.join("trace.csv"), &samples)?;
    std::fs::write(o.out.join("summary.json"), serde_json::to_string_pretty(&summary)?)?;
    write_report(&o.out.join("report.html"), &samples, &summary)?;

    println!("\n{}", headline(&summary));
    println!("\nwrote {}/trace.csv, summary.json, report.html", o.out.display());
    Ok(())
}