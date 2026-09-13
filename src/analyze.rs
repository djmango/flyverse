//! Behaviour analysis harness for the embodied fly.
//!
//! Runs the closed loop headless, records a trace, and reports what the fly
//! actually did. Every number is measured from that run; nothing is asserted.
//!
//! Why this exists: the flight layer is an engineered surrogate (see body.rs),
//! so the interesting question is not "did it fly" but "which behaviours are
//! caused by the connectome and which are produced by the surrogate loops".
//! The loom-versus-steering statistics below are the direct test of that: the
//! room reaches the brain only through the sensory channels in `World::sensors`
//! (odour at the antennae, optic flow, taste, and the looming-wall channel). If
//! a looming wall does not move the steering motor neurons, the fly cannot
//! avoid the wall, no matter how good the aerodynamics are.

use crate::body::Mode;
use crate::lif::DT_MS;
use crate::sim::{World, WINDOW_S};
use anyhow::Result;
use std::io::Write;
use std::path::{Path, PathBuf};

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

impl Default for Options {
    fn default() -> Options {
        Options { seconds: 60.0, seed: 7, sample_every: 10, out: PathBuf::from(".") }
    }
}

fn mode_code(m: Mode) -> u8 {
    match m {
        Mode::Ground => 0,
        Mode::Takeoff => 1,
        Mode::Cruise => 2,
        Mode::Landing => 3,
        Mode::Feeding => 4,
    }
}

fn sample_of(w: &World) -> Sample {
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
        wall_hits: b.wall_hits,
        takeoffs: b.takeoffs,
        landings: b.landings,
        eats: b.eats,
        win_spikes: w.window_spikes.len() as u32,
        tot_spikes: w.lif.total_spikes,
    }
}

// ---------------------------------------------------------------------------
// Stats helpers

fn mean(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().map(|&x| x as f64).sum::<f64>() as f32 / v.len() as f32
}

fn pct(v: &[f32], q: f64) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let i = ((s.len() - 1) as f64 * q.clamp(0.0, 1.0)).round() as usize;
    s[i]
}

fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let ma = a[..n].iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    let mb = b[..n].iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    let (mut num, mut da, mut db) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let x = a[i] as f64 - ma;
        let y = b[i] as f64 - mb;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    let d = (da * db).sqrt();
    if d <= 1e-12 {
        0.0
    } else {
        (num / d) as f32
    }
}

/// Sub-slice helper: values of `s` where `keep(sample)` holds.
fn sel<F: Fn(&Sample) -> bool>(s: &[Sample], f: F) -> Vec<Sample> {
    s.iter().copied().filter(|x| f(x)).collect()
}

fn col<F: Fn(&Sample) -> f32>(s: &[Sample], f: F) -> Vec<f32> {
    s.iter().map(f).collect()
}

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

// ---------------------------------------------------------------------------
// Summary

fn summarize(
    s: &[Sample],
    w: &World,
    sim_seconds: f64,
    wall_seconds: f64,
    o: &Options,
) -> serde_json::Value {
    let n = s.len().max(1);
    let air = sel(s, |x| x.mode == 1 || x.mode == 2 || x.mode == 3);
    let cruise = sel(s, |x| x.mode == 2);
    let minutes = (sim_seconds / 60.0).max(1e-9);

    // Mode occupancy.
    let mut mode_time = [0f64; 5];
    for x in s {
        mode_time[x.mode as usize] += 1.0;
    }
    let mode_pct = |i: usize| mode_time[i] / n as f64 * 100.0;

    // Path length and displacement (mm).
    let mut path = 0f64;
    let mut path_h = 0f64;
    for i in 1..s.len() {
        let dx = (s[i].x - s[i - 1].x) as f64;
        let dy = (s[i].y - s[i - 1].y) as f64;
        let dz = (s[i].z - s[i - 1].z) as f64;
        path += (dx * dx + dy * dy + dz * dz).sqrt();
        path_h += (dx * dx + dy * dy).sqrt();
    }
    let first = s.first().copied().unwrap_or_default();
    let last = s.last().copied().unwrap_or_default();
    let net_h = (((last.x - first.x) as f64).powi(2) + ((last.y - first.y) as f64).powi(2)).sqrt();

    // Wall behaviour.
    let near_wall = s.iter().filter(|x| x.wall_dist < 20.0).count();
    let hits = last.wall_hits - first.wall_hits;
    let near_wall_air = air.iter().filter(|x| x.wall_dist < 20.0).count();

    // Turning.
    let air_yaw_rate = col(&air, |x| x.yaw_rate.abs());
    let total_turn: f64 = if air.len() > 1 {
        let mut t = 0f64;
        for i in 1..air.len() {
            let mut d = air[i].yaw - air[i - 1].yaw;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            t += d.abs() as f64;
        }
        t
    } else {
        0.0
    };

    // The key test: does a looming wall move the steering motor neurons?
    let steer_pair: Vec<(f32, f32)> = air.iter().map(|x| (x.loom, x.steer_r - x.steer_l)).collect();
    let loom_v = steer_pair.iter().map(|p| p.0).collect::<Vec<f32>>();
    let steer_v = steer_pair.iter().map(|p| p.1).collect::<Vec<f32>>();
    let loom_steer_r = pearson(&loom_v, &steer_v);
    let yaw_v = col(&air, |x| x.yaw_rate);
    let loom_yaw_r = pearson(&loom_v, &yaw_v);

    let imminent = sel(&air, |x| x.loom > 0.5);
    let calm = sel(&air, |x| x.loom < 0.2);
    let looming_yaw = mean(&col(&imminent, |x| x.yaw_rate.abs()));
    let calm_yaw = mean(&col(&calm, |x| x.yaw_rate.abs()));
    let looming_steer = mean(&col(&imminent, |x| (x.steer_r - x.steer_l).abs()));
    let calm_steer = mean(&col(&calm, |x| (x.steer_r - x.steer_l).abs()));

    // Occupancy grids over the floor plan.
    const NX: usize = 12;
    const NY: usize = 9;
    let grid = |set: &Vec<Sample>| -> Vec<Vec<f64>> {
        let mut g = vec![vec![0f64; NX]; NY];
        for x in set {
            let cx = (((x.x + 300.0) / 600.0) * NX as f32).floor().clamp(0.0, NX as f32 - 1.0) as usize;
            let cy = (((x.y + 220.0) / 440.0) * NY as f32).floor().clamp(0.0, NY as f32 - 1.0) as usize;
            g[cy][cx] += 1.0;
        }
        let tot: f64 = g.iter().flat_map(|r| r.iter()).sum::<f64>().max(1.0);
        for r in g.iter_mut() {
            for v in r.iter_mut() {
                *v /= tot;
            }
        }
        g
    };

    // Histograms.
    let mut alt_hist = vec![0f64; 22];
    for x in &air {
        let b = (x.z / 10.0).floor().clamp(0.0, 21.0) as usize;
        alt_hist[b] += 1.0;
    }
    let at = air.len().max(1) as f64;
    for v in alt_hist.iter_mut() {
        *v /= at;
    }
    let mut speed_hist = vec![0f64; 16];
    for x in &air {
        let b = (x.speed / 25.0).floor().clamp(0.0, 15.0) as usize;
        speed_hist[b] += 1.0;
    }
    for v in speed_hist.iter_mut() {
        *v /= at;
    }

    let alt_v = col(&air, |x| x.z);
    let spd_v = col(&air, |x| x.speed);

    serde_json::json!({
        "config": {
            "seed": o.seed,
            "seconds": o.seconds,
            "sample_every_windows": o.sample_every,
            "window_ms": WINDOW_S * 1000.0,
            "dt_ms": DT_MS,
            "samples": s.len(),
            "pack": "official-pack",
        },
        "run": {
            "sim_seconds": sim_seconds,
            "wall_seconds": wall_seconds,
            "steps": w.step,
            "steps_per_second": w.step as f64 / wall_seconds,
            "realtime_factor": sim_seconds / wall_seconds,
        },
        "neural": {
            "total_spikes": w.lif.total_spikes,
            "spikes_per_sim_second": w.lif.total_spikes as f64 / sim_seconds.max(1e-9),
            "active_neurons": w.lif.active_neurons,
            "mean_rate_hz": w.lif.total_spikes as f64 / w.conn.n as f64 / sim_seconds.max(1e-9),
            "neurons": w.conn.n,
            "edges": w.conn.m,
            "stimulus_targets": w.vnc_targets,
        },
        "mode_percent": {
            "GROUND": mode_pct(0),
            "TAKEOFF": mode_pct(1),
            "CRUISE": mode_pct(2),
            "LANDING": mode_pct(3),
            "FEEDING": mode_pct(4),
        },
        "events": {
            "takeoffs": last.takeoffs - first.takeoffs,
            "landings": last.landings - first.landings,
            "wall_hits": hits,
            "eats": last.eats - first.eats,
            "wall_hits_per_min": hits as f64 / minutes,
            "takeoffs_per_min": (last.takeoffs - first.takeoffs) as f64 / minutes,
            "mean_seconds_between_wall_hits":
                if hits > 0 { sim_seconds / hits as f64 } else { f64::INFINITY },
        },
        "movement": {
            "path_total_mm": path,
            "path_horizontal_mm": path_h,
            "net_horizontal_displacement_mm": net_h,
            "tortuosity": if net_h > 1e-6 { path_h / net_h } else { f64::INFINITY },
            "mean_speed_mm_s": mean(&spd_v),
            "p50_speed_mm_s": pct(&spd_v, 0.5),
            "p95_speed_mm_s": pct(&spd_v, 0.95),
            "max_speed_mm_s": spd_v.iter().cloned().fold(0.0f32, f32::max),
        },
        "altitude_mm": {
            "mean": mean(&alt_v),
            "p05": pct(&alt_v, 0.05),
            "p50": pct(&alt_v, 0.5),
            "p95": pct(&alt_v, 0.95),
            "max": alt_v.iter().cloned().fold(0.0f32, f32::max),
            "ceiling_contacts_pct_of_airborne": 100.0 * air.iter().filter(|x| x.z > 200.0).count() as f64 / at,
            "hist_10mm": alt_hist,
        },
        "walls": {
            "pct_samples_within_20mm": 100.0 * near_wall as f64 / n as f64,
            "pct_airborne_within_20mm": 100.0 * near_wall_air as f64 / at,
            "mean_wall_distance_airborne_mm": mean(&col(&air, |x| x.wall_dist)),
        },
        "turning": {
            "mean_abs_yaw_rate_rad_s": mean(&air_yaw_rate),
            "p95_abs_yaw_rate_rad_s": pct(&air_yaw_rate, 0.95),
            "pct_airborne_turning_gt_0p5": 100.0 * air.iter().filter(|x| x.yaw_rate.abs() > 0.5).count() as f64 / at,
            "total_turn_rad_airborne": total_turn,
            "full_circles_airborne": total_turn / std::f64::consts::TAU,
            "net_yaw_change_rad": (last.yaw - first.yaw) as f64,
            "steer_differential_mean_abs": mean(&col(&air, |x| (x.steer_r - x.steer_l).abs())),
        },
        "loom_response": {
            "note": "Direct test of whether the room reaches the brain: the looming-wall \
                     channel is the only wall proximity signal the connectome receives.",
            "pearson_loom_vs_steer_differential": loom_steer_r,
            "pearson_loom_vs_yaw_rate": loom_yaw_r,
            "mean_abs_yaw_rate_when_loom_gt_0p5": looming_yaw,
            "mean_abs_yaw_rate_when_loom_lt_0p2": calm_yaw,
            "mean_abs_steer_when_loom_gt_0p5": looming_steer,
            "mean_abs_steer_when_loom_lt_0p2": calm_steer,
            "samples_imminent_wall": imminent.len(),
            "samples_calm": calm.len(),
        },
        "motors_mean": {
            "flight_power_l": mean(&col(s, |x| x.pow_l)),
            "flight_power_r": mean(&col(s, |x| x.pow_r)),
            "flight_steer_l": mean(&col(s, |x| x.steer_l)),
            "flight_steer_r": mean(&col(s, |x| x.steer_r)),
            "walk_l": mean(&col(s, |x| x.walk_l)),
            "walk_r": mean(&col(s, |x| x.walk_r)),
            "land_l": mean(&col(s, |x| x.land_l)),
            "land_r": mean(&col(s, |x| x.land_r)),
            "mn9": mean(&col(s, |x| x.mn9)),
        },
        "descending_mean": {
            "dn02_flight_power": mean(&col(s, |x| x.dn02)),
            "dn07_power_decrease": mean(&col(s, |x| x.dn07)),
            "sapp_flight_state": mean(&col(s, |x| x.sapp)),
            "landing_dn": mean(&col(s, |x| x.land_dn)),
            "dn_filtered": mean(&col(s, |x| x.dn_filt)),
        },
        "sensory_mean": {
            "odor_l": mean(&col(s, |x| x.odor_l)),
            "odor_r": mean(&col(s, |x| x.odor_r)),
            "flow_l": mean(&col(s, |x| x.flow_l)),
            "flow_r": mean(&col(s, |x| x.flow_r)),
            "loom": mean(&col(s, |x| x.loom)),
            "taste_hz": mean(&col(s, |x| x.taste)),
        },
        "occupancy_airborne": grid(&air),
        "occupancy_ground": grid(&sel(s, |x| x.mode == 0)),
        "speed_hist_25mm_s": speed_hist,
        "cruise_samples": cruise.len(),
    })
}

fn headline(sum: &serde_json::Value) -> String {
    let num = |a: &str, b: &str| -> f64 { sum[a][b].as_f64().unwrap_or(0.0) };
    format!(
        "BEHAVIOUR\n  mode %      ground {:.1}  takeoff {:.1}  cruise {:.1}  landing {:.1}  feeding {:.1}\n  \
         events       takeoffs {}  landings {}  wall hits {} ({:.1}/min, one every {:.1}s)  eats {}\n  \
         movement     path {:.0} mm, net {:.0} mm, tortuosity {:.2}, mean speed {:.0} mm/s (p95 {:.0})\n  \
         altitude     mean {:.1} mm, p05 {:.1}, p95 {:.1}, max {:.1}\n  \
         walls        {:.1}% of airborne samples within 20 mm of a wall\n  \
         turning      {:.2} full circles airborne, mean |yaw rate| {:.2} rad/s, {:.1}% of time turning > 0.5\n  \
         LOOM TEST    corr(loom, steer diff) = {:.3}, corr(loom, yaw rate) = {:.3}\n  \
         wall ahead   |yaw rate| {:.2} rad/s when looming vs {:.2} when calm; |steer| {:.3} vs {:.3}\n  \
         neural       {:.0} spikes/s of sim, {:.2} Hz mean rate",
        num("mode_percent","GROUND"),
        num("mode_percent","TAKEOFF"),
        num("mode_percent","CRUISE"),
        num("mode_percent","LANDING"),
        num("mode_percent","FEEDING"),
        num("events","takeoffs"),
        num("events","landings"),
        num("events","wall_hits"),
        num("events","wall_hits_per_min"),
        num("events","mean_seconds_between_wall_hits"),
        num("events","eats"),
        num("movement","path_horizontal_mm"),
        num("movement","net_horizontal_displacement_mm"),
        num("movement","tortuosity"),
        num("movement","mean_speed_mm_s"),
        num("movement","p95_speed_mm_s"),
        num("altitude_mm","mean"),
        num("altitude_mm","p05"),
        num("altitude_mm","p95"),
        num("altitude_mm","max"),
        num("walls","pct_airborne_within_20mm"),
        num("turning","full_circles_airborne"),
        num("turning","mean_abs_yaw_rate_rad_s"),
        num("turning","pct_airborne_turning_gt_0p5"),
        num("loom_response","pearson_loom_vs_steer_differential"),
        num("loom_response","pearson_loom_vs_yaw_rate"),
        num("loom_response","mean_abs_yaw_rate_when_loom_gt_0p5"),
        num("loom_response","mean_abs_yaw_rate_when_loom_lt_0p2"),
        num("loom_response","mean_abs_steer_when_loom_gt_0p5"),
        num("loom_response","mean_abs_steer_when_loom_lt_0p2"),
        num("neural","spikes_per_sim_second"),
        num("neural","mean_rate_hz"),
    )
}

fn write_csv(path: &PathBuf, s: &[Sample]) -> Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        f,
        "t,x,y,z,speed,yaw,yaw_rate,roll,pitch,wing_amp,mode,pow_l,pow_r,steer_l,steer_r,\
walk_l,walk_r,land_l,land_r,mn9,dn02,dn07,sapp,land_dn,dn_filt,odor_l,odor_r,flow_l,flow_r,\
loom,taste,wall_dist,wall_hits,takeoffs,landings,eats,win_spikes,tot_spikes"
    )?;
    for x in s {
        writeln!(
            f,
            "{:.3},{:.2},{:.2},{:.2},{:.2},{:.4},{:.4},{:.4},{:.4},{:.4},{},{:.4},{:.4},{:.4},{:.4},\
{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},\
{:.4},{:.1},{:.2},{},{},{},{},{},{}",
            x.t, x.x, x.y, x.z, x.speed, x.yaw, x.yaw_rate, x.roll, x.pitch, x.wing_amp, x.mode,
            x.pow_l, x.pow_r, x.steer_l, x.steer_r, x.walk_l, x.walk_r, x.land_l, x.land_r, x.mn9,
            x.dn02, x.dn07, x.sapp, x.land_dn, x.dn_filt, x.odor_l, x.odor_r, x.flow_l, x.flow_r,
            x.loom, x.taste, x.wall_dist, x.wall_hits, x.takeoffs, x.landings, x.eats,
            x.win_spikes, x.tot_spikes
        )?;
    }
    f.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Report

const TEMPLATE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<title>FlyVerse behaviour report</title>
<style>
:root{--bg:#0d1117;--panel:#161b22;--line:#30363d;--fg:#e6edf3;--dim:#8b949e;--acc:#58a6ff}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:13px/1.5 ui-sans-serif,system-ui,-apple-system,Segoe UI,Roboto,sans-serif;padding:20px 24px}
h1{font-size:19px;margin:0 0 2px}h2{font-size:13px;text-transform:uppercase;letter-spacing:.08em;color:var(--dim);margin:26px 0 10px;font-weight:600}
.sub{color:var(--dim);margin:0 0 18px}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:12px}
.card{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:12px 14px}
.card h3{margin:0 0 8px;font-size:12px;color:var(--dim);font-weight:600;letter-spacing:.04em;text-transform:uppercase}
.kv{display:flex;justify-content:space-between;gap:10px;padding:2px 0;border-bottom:1px dotted #21262d}
.kv:last-child{border-bottom:0}.kv b{font-variant-numeric:tabular-nums;font-weight:600}
.warn{color:#f0883e}.good{color:#3fb950}.bad{color:#f85149}
canvas{width:100%;display:block;border-radius:6px;background:#0b0f14}
.note{background:#161b22;border-left:3px solid var(--acc);padding:10px 14px;border-radius:6px;color:var(--dim);margin:10px 0}
table{border-collapse:collapse;width:100%;font-size:12px}
th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);vertical-align:top}
th{color:var(--dim);font-weight:600}
code{background:#0b0f14;padding:1px 5px;border-radius:4px;font-size:11px;color:#79c0ff}
</style></head><body>
<h1>FlyVerse behaviour report</h1>
<p class="sub" id="sub"></p>

<h2>Headline</h2>
<div class="grid" id="head"></div>

<h2>Where it went</h2>
<div class="grid">
  <div class="card"><h3>Floor occupancy, airborne (darker = more time)</h3><canvas id="occ" height="300"></canvas></div>
  <div class="card"><h3>Altitude distribution, airborne</h3><canvas id="alt" height="300"></canvas></div>
</div>

<h2>What it did over time</h2>
<div class="grid">
  <div class="card" style="grid-column:1/-1"><h3>Mode timeline</h3><canvas id="mode" height="70"></canvas></div>
  <div class="card" style="grid-column:1/-1"><h3>Altitude and speed</h3><canvas id="ts" height="220"></canvas></div>
  <div class="card" style="grid-column:1/-1"><h3>Wall proximity and the loom channel (does a wall ahead move the steering?)</h3><canvas id="loom" height="200"></canvas></div>
</div>

<h2>The loom test</h2>
<div class="note" id="loomnote"></div>

<h2>Provenance: what is real, what is surrogate</h2>
<table id="prov">
<tr><th>Layer</th><th>Source</th><th>Detail</th></tr>
<tr><td>Connectome graph</td><td class="good">real</td><td>Janelia FlyEM <b>MaleCNS v1.0</b>, 166,700 neurons / 24,469,412 edges, CC BY 4.0. Loaded from the official pack, no pruning or sampling.</td></tr>
<tr><td>Neuron dynamics</td><td class="good">real</td><td>LIF at <code>DT_MS = 0.1</code> ms, dense vote + ring propagation, <code>src/lif.rs</code>.</td></tr>
<tr><td>Spike rates</td><td class="good">real (rate-coded)</td><td>Motor and descending read-outs are measured from actual spikes in the network, not assigned.</td></tr>
<tr><td>Stimulus: vnc_sensory</td><td class="warn">reference replay</td><td>6,370 body IDs driven at 150 Hz from the dataset's own reference set. Fixed Poisson, <b>not</b> generated by the simulated body.</td></tr>
<tr><td>Odour field</td><td class="warn">surrogate</td><td>Finite-core exponential plume with an upwind virtual source, <code>room.rs::odor</code>. A stimulus field, not a fluid solve.</td></tr>
<tr><td>Optic flow</td><td class="warn">surrogate</td><td>Translation + rotation proxy from speed and yaw rate, <code>sim.rs::sense</code>. Not a rendered optic array.</td></tr>
<tr><td>Looming wall</td><td class="warn">surrogate</td><td>Time-to-contact with the nearest wall face, mapped to 0..1 and driven at up to 60 Hz. This is the only wall-proximity signal the brain gets.</td></tr>
<tr><td>Wingbeat</td><td class="warn">surrogate</td><td>Real wingbeat is ~200 Hz; the visual phase runs at 19 Hz so the stroke is visible. Aerodynamics are a tuned quadratic lift/drag model, <code>body.rs</code>.</td></tr>
<tr><td>Attitude and altitude</td><td class="warn">surrogate</td><td>Stabilisation, altitude targeting, banking and heading are engineered control loops. The dataset has no muscles, no VNC and no donor body state.</td></tr>
<tr><td>Gait and collision</td><td class="warn">surrogate</td><td>Walk speed cap, wall bounce restitution 0.15, takeoff/landing timers. No leg kinematics from data.</td></tr>
</table>

<script>
const S = __DATA__;
const SUM = __SUMMARY__;
const MODES = ['GROUND','TAKEOFF','CRUISE','LANDING','FEEDING'];
const MCOL = ['#484f58','#3fb950','#58a6ff','#f0883e','#a371f7'];

function kv(k,v,cls){return `<div class="kv"><span>${k}</span><b class="${cls||''}">${v}</b></div>`}
const n=(x,d=1)=>(x===null||x===undefined||!isFinite(x))?'-':(+x).toFixed(d);

document.getElementById('sub').textContent =
  `seed ${SUM.config.seed} - ${n(SUM.run.sim_seconds,1)} s simulated - ${SUM.config.samples} samples - ` +
  `${SUM.neural.neurons} neurons / ${SUM.neural.edges} edges - ${n(SUM.run.steps_per_second,0)} steps/s - ` +
  `${n(SUM.run.realtime_factor,3)}x realtime`;

const g=SUM.mode_percent, e=SUM.events, m=SUM.movement, a=SUM.altitude_mm, tl=SUM.turning, lr=SUM.loom_response;
document.getElementById('head').innerHTML =
  `<div class="card"><h3>Flight budget</h3>`+
  kv('ground', n(g.GROUND)+' %')+kv('takeoff',n(g.TAKEOFF)+' %')+kv('cruise',n(g.CRUISE)+' %')+
  kv('landing',n(g.LANDING)+' %')+kv('feeding',n(g.FEEDING)+' %')+`</div>`+
  `<div class="card"><h3>Events</h3>`+
  kv('takeoffs',e.takeoffs)+kv('landings',e.landings)+
  kv('wall hits', e.wall_hits, e.wall_hits_per_min>20?'bad':'warn')+
  kv('wall hits / min', n(e.wall_hits_per_min))+
  kv('one every', (e.mean_seconds_between_wall_hits==null || !isFinite(e.mean_seconds_between_wall_hits))?'never':n(e.mean_seconds_between_wall_hits)+' s')+
  kv('meals',e.eats)+`</div>`+
  `<div class="card"><h3>Movement</h3>`+
  kv('path', n(m.path_horizontal_mm,0)+' mm')+kv('net displacement', n(m.net_horizontal_displacement_mm,0)+' mm')+
  kv('tortuosity', n(m.tortuosity,2))+kv('mean speed', n(m.mean_speed_mm_s,0)+' mm/s')+
  kv('p95 speed', n(m.p95_speed_mm_s,0)+' mm/s')+kv('max speed', n(m.max_speed_mm_s,0)+' mm/s')+`</div>`+
  `<div class="card"><h3>Altitude and walls</h3>`+
  kv('alt mean', n(a.mean,1)+' mm')+kv('alt p05 / p95', n(a.p05,1)+' / '+n(a.p95,1))+
  kv('alt max', n(a.max,1)+' mm')+kv('ceiling contacts', n(a.ceiling_contacts_pct_of_airborne,1)+' % of airborne')+
  kv('within 20 mm of a wall', n(SUM.walls.pct_airborne_within_20mm,1)+' %', SUM.walls.pct_airborne_within_20mm>50?'bad':'')+`</div>`+
  `<div class="card"><h3>Steering</h3>`+
  kv('mean |yaw rate|', n(tl.mean_abs_yaw_rate_rad_s,2)+' rad/s')+
  kv('turning > 0.5', n(tl.pct_airborne_turning_gt_0p5,1)+' %')+
  kv('full circles', n(tl.full_circles_airborne,2))+
  kv('net yaw change', n(tl.net_yaw_change_rad,2)+' rad')+
  kv('mean |steer diff|', n(tl.steer_differential_mean_abs,3))+`</div>`+
  `<div class="card"><h3>Neural</h3>`+
  kv('spikes total', SUM.neural.total_spikes.toLocaleString())+
  kv('spikes / sim s', n(SUM.neural.spikes_per_sim_second,0))+
  kv('mean rate', n(SUM.neural.mean_rate_hz,2)+' Hz')+
  kv('active neurons', SUM.neural.active_neurons.toLocaleString())+
  kv('stimulus targets', SUM.neural.stimulus_targets)+`</div>`;

document.getElementById('loomnote').innerHTML =
  `<b>corr(loom, steer differential) = ${n(lr.pearson_loom_vs_steer_differential,3)}</b>, ` +
  `corr(loom, yaw rate) = ${n(lr.pearson_loom_vs_yaw_rate,3)}. ` +
  `With a wall filling the view (loom &gt; 0.5) mean |yaw rate| is ${n(lr.mean_abs_yaw_rate_when_loom_gt_0p5,2)} rad/s ` +
  `versus ${n(lr.mean_abs_yaw_rate_when_loom_lt_0p2,2)} when the way is clear; mean |steer| ` +
  `${n(lr.mean_abs_steer_when_loom_gt_0p5,3)} versus ${n(lr.mean_abs_steer_when_loom_lt_0p2,3)}. ` +
  `${lr.samples_imminent_wall.toLocaleString()} of the airborne samples had a wall imminent. ` +
  (Math.abs(lr.pearson_loom_vs_steer_differential) < 0.1
    ? `<span class="bad">A correlation near zero means the looming signal is not reaching the wings: the steering motor neurons are not being driven by it, so nothing steers the fly away from a wall.</span>`
    : `<span class="good">A non-zero correlation means the looming signal does move the steering motor neurons.</span>`);

function fit(id, h){
  const c=document.getElementById(id), r=window.devicePixelRatio||1;
  c.width=c.clientWidth*r; c.height=(h||c.clientHeight)*r;
  const x=c.getContext('2d'); x.setTransform(r,0,0,r,0,0);
  return [x, c.clientWidth, h||c.clientHeight];
}

// Occupancy heatmap
(function(){
  const [x,W,H]=fit('occ',300);
  const g=SUM.occupancy_airborne, ny=g.length, nx=g[0].length;
  const cw=W/nx, ch=H/ny, mx=Math.max(...g.flat(),1e-9);
  for(let r=0;r<ny;r++)for(let c=0;c<nx;c++){
    const v=g[r][c]/mx, al=Math.pow(v,0.45);
    x.fillStyle=`rgba(88,166,255,${(al*0.92).toFixed(3)})`;
    x.fillRect(c*cw, H-(r+1)*ch, cw-1, ch-1);
  }
  x.strokeStyle='#30363d'; x.strokeRect(0.5,0.5,W-1,H-1);
  x.fillStyle='#8b949e'; x.font='10px ui-monospace,monospace';
  x.fillText('x -300..300 mm', 6, H-6); x.fillText('y -220..220 mm  (top-down)', 6, 12);
})();

// Altitude histogram
(function(){
  const [x,W,H]=fit('alt',300);
  const h=SUM.altitude_mm.hist_10mm, mx=Math.max(...h,1e-9), bw=W/h.length;
  for(let i=0;i<h.length;i++){
    const bh=(h[i]/mx)*(H-30);
    x.fillStyle= i*10<40 ? '#3fb950' : '#58a6ff';
    x.fillRect(i*bw+1, H-20-bh, bw-2, bh);
  }
  x.fillStyle='#8b949e'; x.font='10px ui-monospace,monospace';
  x.fillText('0', 2, H-6); x.fillText('220 mm', W-46, H-6);
  x.fillText('table top 40 mm', 2, 12);
  x.strokeStyle='#30363d'; x.beginPath(); x.moveTo(0,H-20); x.lineTo(W,H-20); x.stroke();
})();

// Mode timeline
(function(){
  const [x,W,H]=fit('mode',70);
  const n=S.length;
  for(let i=0;i<n;i++){
    x.fillStyle=MCOL[S[i].mode];
    x.fillRect(i/n*W, 18, Math.max(1, W/n), 26);
  }
  x.font='10px ui-monospace,monospace';
  let lx=0;
  MODES.forEach((m,i)=>{ x.fillStyle=MCOL[i]; x.fillRect(lx,4,9,9);
    x.fillStyle='#8b949e'; x.fillText(m, lx+12, 12); lx+=12+x.measureText(m).width+14; });
  x.fillStyle='#8b949e'; x.fillText('t = 0 s', 2, H-3); x.fillText('t = '+n(S[S.length-1].t,1)+' s', W-58, H-3);
})();

// Altitude + speed
(function(){
  const [x,W,H]=fit('ts',220);
  const pad=26, n=S.length;
  const zmax=Math.max(...S.map(s=>s.z))*1.08||1;
  const vmax=Math.max(...S.map(s=>s.speed))*1.08||1;
  const px=i=>pad+ (i/(n-1||1))*(W-pad-8);
  const py=(v,mx)=>H-18-(v/mx)*(H-34);
  x.strokeStyle='#21262d'; x.beginPath(); x.moveTo(pad,H-18); x.lineTo(W-8,H-18); x.stroke();
  x.strokeStyle='#58a6ff'; x.lineWidth=1.4; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(s.z,zmax); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.strokeStyle='#f0883e'; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(s.speed,vmax); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.font='10px ui-monospace,monospace';
  x.fillStyle='#58a6ff'; x.fillText('altitude (max '+n(zmax,0)+' mm)', pad+4, 12);
  x.fillStyle='#f0883e'; x.fillText('speed (max '+n(vmax,0)+' mm/s)', pad+150, 12);
  x.fillStyle='#8b949e'; x.fillText('t = 0', pad-14, H-4); x.fillText(n(S[S.length-1].t,0)+' s', W-30, H-4);
})();

// Loom + steering
(function(){
  const [x,W,H]=fit('loom',200);
  const pad=26, n=S.length;
  const px=i=>pad+(i/(n-1||1))*(W-pad-8);
  const py=v=>H-18-v*(H-34);
  x.strokeStyle='#21262d'; x.beginPath(); x.moveTo(pad,H-18); x.lineTo(W-8,H-18); x.stroke();
  x.strokeStyle='#f85149'; x.globalAlpha=0.85; x.lineWidth=1.1; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(Math.min(1,s.loom)); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.globalAlpha=1;
  const sd=S.map(s=>Math.abs(s.steer_r-s.steer_l));
  const sdmax=Math.max(...sd,1e-6);
  x.strokeStyle='#3fb950'; x.lineWidth=1.4; x.beginPath();
  S.forEach((s,i)=>{ const X=px(i),Y=py(Math.abs(s.steer_r-s.steer_l)/sdmax); i?x.lineTo(X,Y):x.moveTo(X,Y); }); x.stroke();
  x.font='10px ui-monospace,monospace';
  x.fillStyle='#f85149'; x.fillText('loom 0..1 (wall ahead)', pad+4, 12);
  x.fillStyle='#3fb950'; x.fillText('|steer differential| (scaled)', pad+160, 12);
  x.fillStyle='#8b949e'; x.fillText('t = 0', pad-14, H-4);
})();
</script></body></html>
"#;

fn write_report(path: &Path, samples: &[Sample], summary: &serde_json::Value) -> Result<()> {
    // Keep the embedded series bounded; the CSV carries the full rate.
    let stride = (samples.len() / 1500).max(1);
    let slim: Vec<&Sample> = samples.iter().step_by(stride).collect();
    let data = serde_json::to_string(&slim)?;
    let html = TEMPLATE
        .replace("__DATA__", &data)
        .replace("__SUMMARY__", &serde_json::to_string(summary)?);
    std::fs::write(path, html)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Rotation perturbation probe
//
// The on/off comparison in the main analysis cannot settle whether the haltere
// afferents do anything, because the system is chaotic: two runs differing in
// any input diverge, and that divergence alone produces differences of the size
// observed. This probe answers the question a different way.
//
// The body is given an angular velocity it did not generate, and the same
// perturbation is applied with both signs. The network state at the moment of
// perturbation is not controlled, so the sign is drawn at random and the
// response is averaged separately for each sign. Chaotic variation is
// independent of the sign and averages out; a reflex is a function of the sign
// and survives. That sign-conditioned difference is the measurement.
//
// A second design point: the probe only ever touches the body's angular
// velocity. It never injects spikes or alters the stimulus, so any motor
// response has to travel through the connectome and back out.

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

impl Default for ProbeOptions {
    fn default() -> Self {
        ProbeOptions {
            warmup_s: 10.0,
            trials: 60,
            interval_s: 0.5,
            response_ms: 200.0,
            amplitude: 25.0,
        }
    }
}

/// Yaw then roll. Yaw drives the two halteres in common, roll drives them
/// differentially, which is the split the real pair gives.
const PROBED_AXES: [usize; 2] = [2, 0];
const AXIS_NAME: [&str; 3] = ["roll", "pitch", "yaw"];

/// xorshift64. Deterministic from the seed, so a probe run is reproducible.
fn next_u64(r: &mut u64) -> u64 {
    *r ^= *r << 13;
    *r ^= *r >> 7;
    *r ^= *r << 17;
    *r
}

fn mean64(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn sem64(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::INFINITY;
    }
    let m = mean64(v);
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64;
    (var / v.len() as f64).sqrt()
}

/// Welch's t statistic for a difference of means. Reported with the caveat that
/// the trials are not independent samples of a stationary process, so the number
/// is a signal-to-noise guide rather than a p-value.
fn welch_t(a: &[f64], b: &[f64]) -> f64 {
    let (sa, sb) = (sem64(a), sem64(b));
    let denom = (sa * sa + sb * sb).sqrt();
    if !denom.is_finite() || denom <= 0.0 {
        return 0.0;
    }
    ((mean64(a) - mean64(b)) / denom).abs()
}

/// Balanced sign schedule: one list per probed axis, each half positive and half
/// negative, shuffled with the seeded generator so the sign is uncorrelated with
/// the fly's own behaviour at that instant. Equal group sizes matter because the
/// two means are compared directly, and with few trials an unbalanced draw would
/// put most of the weight on one side.
fn balanced_sign_schedule(n_axes: usize, per_axis: usize, seed: u64) -> Vec<Vec<bool>> {
    let mut rng = seed ^ 0x9E37_79B9_7F4A_7C15;
    (0..n_axes)
        .map(|_| {
            let mut v: Vec<bool> = (0..per_axis).map(|i| i % 2 == 0).collect();
            for i in (1..v.len()).rev() {
                let j = (next_u64(&mut rng) % (i as u64 + 1)) as usize;
                v.swap(i, j);
            }
            v
        })
        .collect()
}

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
        if w.flow_on {
            "delivered"
        } else {
            "SILENCED (control)"
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
    out.insert("optic_flow_delivered".into(), w.flow_on.into());
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
