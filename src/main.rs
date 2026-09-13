//! flyverse: native Rust MaleCNS connectome runtime.

mod analyze;
mod body;
mod brain;
mod groups;
mod httpd;
mod lif;
mod npy;
mod pack;
mod room;
mod sim;
mod wing;

use anyhow::{bail, Context, Result};
use lif::{Lif, StimGen, DT_MS};
use pack::Connectome;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Default connectome pack, relative to the working directory. Override with
/// `FLYVERSE_PACK` or `--pack DIR`.
const DEFAULT_PACK: &str = "official-pack";
/// Default vnc_sensory stimulus set. Override with `FLYVERSE_VNC_TARGETS` or
/// `--targets FILE`.
const TARGETS_VNC: &str = "data/targets_vnc_sensory.u64";

fn default_pack() -> PathBuf {
    std::env::var("FLYVERSE_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_PACK))
}

fn usage() -> ! {
    eprintln!(
        "flyverse <command>\n\
         \n\
         commands:\n\
         \x20 bench   --pack DIR [--steps N] [--rate HZ] [--seed S] [--targets FILE]\n\
         \x20 verify  --pack DIR --stim FILE --out FILE [--steps N]\n\
         \x20 census  --pack DIR\n\
         \x20 analyze --seconds N [--seed S] [--every N] [--out DIR]\n\
         \x20 haltere-probe [--seed S] [--trials N] [--amplitude R] [--out FILE]\n\
         \x20 serve   [--pack DIR] [--port N] [--seconds N] [--rate HZ] [--seed S] [--targets FILE]\n"
    );
    std::process::exit(2)
}

struct Args {
    map: std::collections::HashMap<String, String>,
}

impl Args {
    fn parse() -> Args {
        let mut map = std::collections::HashMap::new();
        let mut it = std::env::args().skip(2);
        while let Some(a) = it.next() {
            if let Some(k) = a.strip_prefix("--") {
                let v = it.next().unwrap_or_default();
                map.insert(k.to_string(), v);
            }
        }
        Args { map }
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.map.get(k).map(|s| s.as_str())
    }
    fn u64(&self, k: &str, d: u64) -> u64 {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    fn f64(&self, k: &str, d: f64) -> f64 {
        self.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
}

fn read_u64_file(p: &Path) -> Result<Vec<u64>> {
    let bytes = std::fs::read(p).with_context(|| format!("read {}", p.display()))?;
    if bytes.len() % 8 != 0 {
        bail!("{} is not a multiple of 8 bytes", p.display());
    }
    Ok(bytes
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect())
}

fn targets(args: &Args) -> Result<Vec<u64>> {
    let path = PathBuf::from(args.get("targets").unwrap_or(TARGETS_VNC));
    let mut ids = read_u64_file(&path)?;
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_else(|| usage());
    let args = Args::parse();
    let pack_path = match args.get("pack") {
        Some(p) => PathBuf::from(p),
        None => default_pack(),
    };

    match cmd.as_str() {
        "census" => {
            let c = Connectome::load(&pack_path)?;
            let man = Connectome::manifest(&pack_path).ok();
            println!("pack        {}", pack_path.display());
            if let Some(m) = &man {
                println!(
                    "dataset     {} {}  ({})",
                    m.dataset.clone().unwrap_or_default(),
                    m.dataset_version.clone().unwrap_or_default(),
                    m.license.clone().unwrap_or_default()
                );
            }
            println!("{}", serde_json::to_string_pretty(&c.census())?);
            Ok(())
        }
        "bench" => {
            let steps = args.u64("steps", 10_000) as usize;
            let rate = args.f64("rate", 150.0);
            let seed = args.u64("seed", 7);
            let t0 = Instant::now();
            let c = Connectome::load(&pack_path)?;
            let load_s = t0.elapsed().as_secs_f64();
            let ids = targets(&args)?;
            let idx = c.indices_of_sorted(&ids);
            let mut sim = Lif::new(&c);
            sim.profiling = true;
            let mut stim = StimGen::new(idx.clone(), rate, seed);
            println!(
                "pack loaded in {:.2}s: {} neurons, {} edges; {} stimulus targets",
                load_s,
                c.n,
                c.m,
                idx.len()
            );
            // warm up one step
            sim.step(&c, stim.next_events());
            let t1 = Instant::now();
            let mut window_spikes: u64 = 0;
            let mut last_report = 0usize;
            for s in 0..steps {
                let before = sim.total_spikes;
                sim.step(&c, stim.next_events());
                window_spikes = sim.total_spikes - before;
                let _ = window_spikes;
                if s + 1 == steps / 10 * (last_report + 1) {
                    last_report += 1;
                }
            }
            let dt = t1.elapsed().as_secs_f64();
            let sim_seconds = steps as f64 * DT_MS as f64 / 1000.0;
            let out = serde_json::json!({
                "engine": "flyverse native Rust (f32, rayon)",
                "pack": pack_path.display().to_string(),
                "neurons": c.n,
                "edges": c.m,
                "steps": steps,
                "dt_ms": DT_MS,
                "sim_seconds": sim_seconds,
                "wall_seconds": dt,
                "steps_per_second": steps as f64 / dt,
                "realtime_factor": sim_seconds / dt,
                "total_spikes": sim.total_spikes,
                "active_neurons": sim.active_neurons,
                "mean_rate_hz": sim.total_spikes as f64 / c.n as f64 / sim_seconds,
                "stimulus": format!("Poisson {rate} Hz on {} targets", idx.len()),
                "seed": seed,
                "threads": rayon::current_num_threads(),
                "phase_seconds": {
                    "dense_v_g_refractory": sim.t_dense,
                    "ring_clear_and_stimulus": sim.t_clear,
                    "fire_scan": sim.t_scan,
                    "spike_propagation": sim.t_prop,
                },
            });
            let txt = serde_json::to_string_pretty(&out)?;
            println!("{txt}");
            let outdir = PathBuf::from("/opt/data/workspaces/skg/flybrain/flyverse/data");
            std::fs::create_dir_all(&outdir)?;
            std::fs::write(outdir.join("bench_rust.json"), &txt)?;
            Ok(())
        }
        "verify" => {
            let steps = args.u64("steps", 10_000) as usize;
            let stim_path = PathBuf::from(args.get("stim").context("--stim FILE is required")?);
            let out = PathBuf::from(args.get("out").context("--out FILE is required")?);
            let c = Connectome::load(&pack_path)?;
            let stim = load_stim(&stim_path)?;
            let mut sim = Lif::new(&c);
            let n = steps.min(stim.len());
            let mut log = std::io::BufWriter::new(std::fs::File::create(&out)?);
            let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
            let mut mix = |b: u8, h: &mut u64| {
                *h = (*h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            };
            for step in 0..n {
                let events: Vec<(u32, u32)> = stim[step].iter().map(|&i| (i, 1)).collect();
                sim.step(&c, &events);
                let len = sim.fired.len() as u32;
                log.write_all(&len.to_le_bytes())?;
                for b in len.to_le_bytes() {
                    mix(b, &mut hash);
                }
                for &i in &sim.fired {
                    log.write_all(&i.to_le_bytes())?;
                    for b in i.to_le_bytes() {
                        mix(b, &mut hash);
                    }
                }
            }
            log.flush()?;
            let report = serde_json::json!({
                "engine": "flyverse native Rust (f32, rayon)",
                "pack": pack_path.display().to_string(),
                "steps": n,
                "total_spikes": sim.total_spikes,
                "active_neurons": sim.active_neurons,
                "spike_log": out.display().to_string(),
                "spike_log_fnv1a": format!("{hash:016x}"),
            });
            let txt = serde_json::to_string_pretty(&report)?;
            println!("{txt}");
            let mut rp = out.clone();
            rp.set_extension("json");
            std::fs::write(rp, &txt)?;
            Ok(())
        }
        "selftest" => {
            let steps = args.u64("steps", 40) as usize;
            let c = Connectome::load(&pack_path)?;
            let mut sim = Lif::new(&c);
            let stim_ids: Vec<u32> = (0..16u32).collect();
            for step in 0..steps {
                let stim: Vec<(u32, u32)> = stim_ids.iter().map(|&i| (i, 1)).collect();
                sim.step(&c, &stim);
                let ones: u32 = sim.mask.iter().map(|w| w.count_ones()).sum();
                let mx = sim.v.iter().cloned().fold(f32::MIN, f32::max);
                let mn = sim.v.iter().cloned().fold(f32::MAX, f32::min);
                println!(
                    "step {step:3}: dense_fires={:4} fired={:4} mask_ones={:4} max_v={mx:9.3} spikes_total={}",
                    sim.dense_fires,
                    sim.fired.len(),
                    ones,
                    sim.total_spikes
                );
            }
            Ok(())
        }
        "serve" => {
            let port = args.u64("port", 8099) as u16;
            let seconds = args.f64("seconds", 0.0);
            let rate = args.f64("rate", 150.0);
            let seed = args.u64("seed", 7);
            let rt = args.get("rt").and_then(|v| v.parse().ok());
            sim::serve(pack_path, port, seconds, rate, seed, rt)
        }
        "analyze" => {
            // Headless behaviour run: records a trace and reports what the fly
            // actually did, plus whether the room reaches the brain at all.
            let o = analyze::Options {
                seconds: args.f64("seconds", 60.0),
                seed: args.u64("seed", 7),
                sample_every: args.u64("every", 10),
                out: PathBuf::from(args.get("out").unwrap_or("runs/analyze")),
            };
            analyze::run(&pack_path, &o)
        }
        "haltere-probe" => {
            // Imposed-rotation probe: does the network drive the wings to oppose
            // a rotation it did not generate? Sign-averaged, so chaos cancels.
            let o = analyze::Options {
                seconds: 0.0,
                seed: args.u64("seed", 7),
                sample_every: 1,
                out: PathBuf::from(args.get("out").unwrap_or("runs/haltere-probe")),
            };
            let p = analyze::ProbeOptions {
                warmup_s: args.f64("warmup", 10.0),
                trials: args.u64("trials", 60) as u32,
                interval_s: args.f64("interval", 0.5),
                response_ms: args.f64("response", 200.0),
                amplitude: args
                    .get("amplitude")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(25.0),
            };
            analyze::rotation_probe(&pack_path, &o, &p)
        }
        "probe" => {
            // Headless closed loop: is the fly actually moving, and is anything
            // in the network driving it?
            let seconds = args.f64("seconds", 6.0);
            let seed = args.u64("seed", 7);
            let mut w = sim::World::new(&pack_path, seed, None)?;
            println!("{} neurons, {} edges", w.conn.n, w.conn.m);
            println!(
                "vnc_sensory stimulus set: {} neurons at 150 Hz",
                w.vnc_targets
            );
            for name in [
                "olfaction_left", "olfaction_right", "taste_sugar",
                "visual_motion_left", "visual_motion_right", "visual_loom_left",
                "visual_loom_right", "motor_flight_power_left", "motor_flight_steering_left",
                "motor_walking_left", "motor_landing_left", "feeding_mn9",
                "flight_dng02_left", "flight_dng07_left", "flight_state_sapp_left",
                "landing_dn_left", "walking_dn_left",
            ] {
                let gi = w.groups.idx(name);
                if gi == usize::MAX {
                    println!("  {name:<26} ABSENT");
                } else {
                    println!("  {name:<26} {} neurons", w.groups.group_size[gi]);
                }
            }
            let steps = (seconds / sim::WINDOW_S as f64) as u64;
            println!(
                "\n{:>7} {:>9} {:>9} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
                "t(s)", "mode", "speed", "z", "wing", "dn", "powL", "walkL", "mn9", "dn02", "hall"
            );
            for s in 0..steps {
                if s % 250 == 0 {
                    let b = &w.body;
                    let m = w.motors();
                    let r = &w.rates;
                    let f = |o: &sim::Out| o.norm(r);
                    println!(
                        "{:>7.2} {:>9} {:>9.1} {:>7.1} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3}",
                        w.step as f64 * lif::DT_MS as f64 / 1000.0,
                        b.mode.as_str(),
                        b.speed(),
                        b.pos[2],
                        b.wing_amp,
                        w.dn_filter(),
                        m.flight_power_l,
                        m.walk_l,
                        m.mn9,
                        f(&w.o_dn02),
                        f(&w.o_sapp),
                    );
                }
                w.advance();
            }
            let b = &w.body;
            println!(
                "\nfinal: mode={} pos=({:.1},{:.1},{:.1}) speed={:.1} wing={:.3} \
                 takeoffs={} eats={} landings={} wall_hits={} taste_gain={:.2}",
                b.mode.as_str(),
                b.pos[0],
                b.pos[1],
                b.pos[2],
                b.speed(),
                b.wing_amp,
                b.takeoffs,
                b.eats,
                b.landings,
                b.wall_hits,
                b.taste_gain
            );
            println!("spikes total: {}", w.lif.total_spikes);
            println!("\nper-group mean rate over the last window (Hz/neuron):");
            let mut rows: Vec<(String, u32, f32)> = (0..w.groups.names.len())
                .map(|gi| {
                    (
                        w.groups.names[gi].clone(),
                        w.groups.group_size[gi],
                        w.rates.rate_hz[gi],
                    )
                })
                .collect();
            rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            for (name, size, hz) in rows {
                println!("  {name:<30} n={size:<6} {hz:8.2} Hz");
            }
            Ok(())
        }
        other => {
            eprintln!("unknown command: {other}");
            usage()
        }
    }
}

pub fn load_stim(p: &Path) -> Result<Vec<Vec<u32>>> {
    let bytes = std::fs::read(p).with_context(|| format!("read {}", p.display()))?;
    let mut cur = 0usize;
    let mut out = Vec::new();
    while cur + 4 <= bytes.len() {
        let count = u32::from_le_bytes(bytes[cur..cur + 4].try_into().unwrap()) as usize;
        cur += 4;
        if cur + count * 4 > bytes.len() {
            bail!("{}: truncated stimulus file", p.display());
        }
        let mut row = Vec::with_capacity(count);
        for k in 0..count {
            let o = cur + k * 4;
            row.push(u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()));
        }
        cur += count * 4;
        out.push(row);
    }
    Ok(out)
}
