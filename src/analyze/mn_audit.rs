//! Motor-neuron audit: why does the flight-power pool fire at ~150-330 Hz when
//! a real *Drosophila* DLM fires at 3-20 Hz?
//!
//! This is a measurement instrument, not a model. It reports, for a named
//! motor pool:
//!
//!   1. the anatomical input composition of each member neuron -- how many
//!      presynaptic partners it has, how many excitatory and inhibitory
//!      contacts those partners deliver, the excitatory fraction, and the
//!      largest single-edge contact count. A single incoming edge is one
//!      presynaptic spike times `0.275 mV` per contact (`pack::WEIGHT_MV`), so
//!      the largest edge says how much one upstream spike can move a neuron.
//!   2. the measured per-neuron firing rate of every member over a closed-loop
//!      run, plus the network mean rate and a whole-network rate histogram.
//!   3. the top presynaptic drivers of the pool by contact count, each with its
//!      own measured firing rate -- so it is visible whether the pool is being
//!      driven by a small number of hyperactive hub neurons or by a broad,
//!      balanced population.
//!
//! Everything printed is measured from the run; nothing is assumed.

use crate::lif::DT_MS;
use crate::pack::WEIGHT_MV;
use crate::sim::{World, WINDOW_S};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct AuditOptions {
    pub seconds: f64,
    pub seed: u64,
    pub group: String,
    pub top: usize,
}

fn signed_contacts(w: &World, e: usize) -> i32 {
    (w.conn.edge_write_mv[e] / WEIGHT_MV).round() as i32
}

pub fn mn_audit(pack: &Path, o: &AuditOptions) -> Result<()> {
    let mut w = World::new(pack, o.seed, None)?;
    let n = w.conn.n;
    let m = w.conn.m;

    let gi = w.groups.idx(&o.group);
    if gi == usize::MAX {
        anyhow::bail!("group {} is absent from the annotation", o.group);
    }
    let members: Vec<u32> = w.groups.get(&o.group).to_vec();
    if members.is_empty() {
        anyhow::bail!("group {} has no members in the pack", o.group);
    }

    println!("=== motor-neuron audit: {} ===", o.group);
    println!(
        "pack: {n} neurons, {m} edges; group {} has {} members; drive {:?} Hz vnc_sensory",
        o.group,
        members.len(),
        std::env::var("FLYVERSE_STIM_HZ").unwrap_or_else(|_| "<default>".into())
    );
    println!(
        "LIF: dt={} ms, rest/reset={} mV, threshold={} mV, gap={} mV, tau_m={} ms, tau_s={} ms, \
         refractory={} ms (ceiling {} Hz), delay={} ms, weight={} mV/contact, coupling={:.6}, \
         external pulse={} mV",
        crate::lif::DT_MS,
        crate::lif::REST_MV,
        crate::lif::THRESHOLD_MV,
        crate::lif::THRESHOLD_MV - crate::lif::REST_MV,
        crate::lif::TAU_M_MS,
        crate::lif::TAU_S_MS,
        crate::lif::REFRACTORY_MS,
        1000.0 / crate::lif::REFRACTORY_MS,
        crate::lif::DELAY_MS,
        WEIGHT_MV,
        w.lif.coupling,
        crate::lif::POISSON_MV,
    );

    // ---------------------------------------------------------------- anatomy
    let mut inset = vec![false; n];
    for &mm in &members {
        inset[mm as usize] = true;
    }
    let pos_of: HashMap<u32, usize> =
        members.iter().enumerate().map(|(k, &mm)| (mm, k)).collect();

    let mut partners: Vec<HashSet<u32>> = vec![HashSet::new(); members.len()];
    // Per member: (presynaptic model index, signed contacts), kept so the
    // mean-field drive from the partners' MEASURED rates can be computed.
    let mut member_in: Vec<Vec<(u32, i32)>> = vec![Vec::new(); members.len()];
    let mut in_e = vec![0i64; members.len()];
    let mut in_i = vec![0i64; members.len()];
    let mut in_edges = vec![0usize; members.len()];
    let mut max_edge = vec![0i32; members.len()];
    // src -> (contacts onto the whole pool, is-excitatory)
    let mut by_src: HashMap<u32, (i64, bool)> = HashMap::new();
    let mut scanned = 0usize;
    for e in 0..m {
        let d = w.conn.destinations[e];
        if !inset[d as usize] {
            continue;
        }
        scanned += 1;
        let k = pos_of[&d];
        let c = signed_contacts(&w, e);
        partners[k].insert(w.conn.srcs[e]);
        member_in[k].push((w.conn.srcs[e], c));
        in_edges[k] += 1;
        if c > 0 {
            in_e[k] += c as i64;
        } else {
            in_i[k] += -c as i64;
        }
        if c.abs() > max_edge[k] {
            max_edge[k] = c.abs();
        }
        let ent = by_src.entry(w.conn.srcs[e]).or_insert((0, c > 0));
        ent.0 += c.abs() as i64;
        if c > 0 {
            ent.1 = true;
        }
    }

    let pool_e: i64 = in_e.iter().sum();
    let pool_i: i64 = in_i.iter().sum();
    let tot = (pool_e + pool_i).max(1);
    println!(
        "\n-- input onto the pool (anatomy, from the pack) --\n\
         {scanned} incoming edges; {pool_e} excitatory contacts, {pool_i} inhibitory contacts; \
         E fraction {:.3}; {} distinct presynaptic neurons",
        pool_e as f64 / tot as f64,
        by_src.len()
    );
    println!(
        "{:>8} {:>10} {:>10} {:>10} {:>10} {:>9} {:>9} {:>8}",
        "model", "in_edges", "in_partners", "E_contacts", "I_contacts", "E_frac", "max_edge", "out_deg"
    );
    for (k, &mm) in members.iter().enumerate() {
        let e = in_e[k];
        let i = in_i[k];
        let d = (e + i).max(1);
        let out = w.conn.row_ptr[mm as usize + 1] as usize - w.conn.row_ptr[mm as usize] as usize;
        println!(
            "{:>8} {:>10} {:>10} {:>10} {:>10} {:>9.3} {:>9} {:>8}",
            mm,
            in_edges[k],
            partners[k].len(),
            e,
            i,
            e as f64 / d as f64,
            max_edge[k],
            out
        );
    }

    // --------------------------------------------------------------- dynamics
    let windows = ((o.seconds / WINDOW_S as f64) as u64).max(1);
    println!(
        "\n-- run: {:.1} s = {} control windows of {:.1} ms, seed {} --",
        o.seconds,
        windows,
        WINDOW_S * 1000.0,
        o.seed
    );
    // Per-window accounting: the read-out the model actually uses is a spike
    // COUNT over the 2 ms control window, not a long-run rate. Its resolution
    // is one spike per window, so the histogram below is the real measurement
    // resolution, and `win_hist[0]` says how often the pool reads exactly zero.
    let mut win_hist = vec![0u64; members.len() + 1];
    // The ACTUATOR COMMAND the body actually receives: `GroupRates::norm` of the
    // window, passed through the same `sim::MOTOR_TAU_S` first-order lag `World`
    // applies before `body.rs` reads it. The raw 2 ms window is one spike per
    // member; this is what the wing sees.
    let mut a_cmd: Vec<f32> = Vec::with_capacity(windows as usize);
    let scale_rd = crate::groups::phys_full_scale_hz(&o.group)
        .unwrap_or(1000.0 / (WINDOW_S * 1000.0));
    let a_lag = 1.0 - (-WINDOW_S / crate::sim::MOTOR_TAU_S).exp();
    let mut a_sm = 0.0f32;
    for _ in 0..windows {
        w.advance();
        let mut cnt = 0u32;
        for &i in &w.window_spikes {
            if inset[i as usize] {
                cnt += 1;
            }
        }
        win_hist[cnt as usize] += 1;
        let raw = (cnt as f32 / (WINDOW_S * members.len() as f32) / scale_rd).clamp(0.0, 1.0);
        a_sm += (raw - a_sm) * a_lag;
        a_cmd.push(a_sm);
    }
    let sim_s = w.step as f64 * DT_MS as f64 / 1000.0;
    let net_hz = w.lif.total_spikes as f64 / n as f64 / sim_s;
    println!(
        "ran {:.2} s of model time; {} spikes total; network mean {:.2} Hz/neuron; \
         {} neurons ever active ({:.1}% of the net)",
        sim_s,
        w.lif.total_spikes,
        net_hz,
        w.lif.active_neurons,
        100.0 * w.lif.active_neurons as f64 / n as f64
    );

    println!(
        "\n-- per-neuron firing rate of the pool (count / model seconds) --"
    );
    println!(
        "{:>8} {:>12} {:>12} {:>12}",
        "model", "spikes", "Hz", "frac_of_ceiling"
    );
    let mut sum_hz = 0.0f64;
    let ceil_hz = 1000.0 / crate::lif::REFRACTORY_MS;
    for &mm in &members {
        let c = w.lif.spike_count[mm as usize] as f64;
        let hz = c / sim_s;
        sum_hz += hz;
        println!(
            "{:>8} {:>12} {:>12.1} {:>12.3}",
            mm,
            w.lif.spike_count[mm as usize],
            hz,
            hz / ceil_hz
        );
    }
    let pool_hz = sum_hz / members.len() as f64;
    println!(
        "pool mean {:.1} Hz/neuron = {:.3} x the {:.0} Hz physiological maximum (POWER_MN_MAX_HZ) \
         and {:.3} x the {:.0} Hz refractory ceiling",
        pool_hz,
        pool_hz / 20.0,
        crate::groups::POWER_MN_MAX_HZ,
        pool_hz / ceil_hz,
        ceil_hz
    );

    // ------------------------------------------------------ gradedness
    // The question this pool exists to answer: is its output GRADED, or is it a
    // switch pinned at one rail? Three distributions answer it, all measured:
    //   (a) the per-NEURON long-run rate, against the 3-20 Hz a DLM motor neuron
    //       occupies in flight;
    //   (b) the per-WINDOW pool rate, i.e. what the actuator read-out sees
    //       before it is smoothed: the fraction of control windows reading at
    //       the physiological full scale (the read-out's rail, `norm` == 1.0),
    //       and the fraction reading exactly zero;
    //   (c) whether (a) and (b) move when the stimulus moves, which the sweep
    //       over FLYVERSE_STIM_HZ / the sense-ablation ladder measures and this
    //       block reports for the run it is given.
    let mut rates: Vec<f64> = members
        .iter()
        .map(|&mm| w.lif.spike_count[mm as usize] as f64 / sim_s)
        .collect();
    rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| -> f64 {
        let k = ((rates.len() - 1) as f64 * p).round() as usize;
        rates[k.min(rates.len() - 1)]
    };
    let band = |lo: f64, hi: f64| -> usize {
        rates.iter().filter(|&&r| r >= lo && r < hi).count()
    };
    let n_mem = members.len() as f64;
    let pct = |c: usize| 100.0 * c as f64 / n_mem;
    println!(
        "\n-- GRADEDNESS: per-neuron rate distribution ({} neurons) --\n\
         min {:.1}  q1 {:.1}  median {:.1}  q3 {:.1}  max {:.1}  Hz/neuron; \
         3-20 Hz physiological band = [{:.0}-{:.0} Hz working range]",
        members.len(),
        q(0.0),
        q(0.25),
        q(0.50),
        q(0.75),
        q(1.0),
        crate::groups::POWER_MN_MAX_HZ * 0.15,
        crate::groups::POWER_MN_MAX_HZ
    );
    println!(
        "  ==    0 Hz   {:>3}  {:>6.1}%   (silent)\n\
         \x20 (0,  3) Hz   {:>3}  {:>6.1}%   (below the 3-12 Hz flight working range)\n\
         \x20 [3, 12) Hz   {:>3}  {:>6.1}%   (IN the measured DLM flight working range)\n\
         \x20 [12, 20) Hz  {:>3}  {:>6.1}%   (in range: manoeuvring rates)\n\
         \x20 [20,100) Hz  {:>3}  {:>6.1}%   (above any measured DLM rate)\n\
         \x20 >=100 Hz     {:>3}  {:>6.1}%   (>5x the physiological maximum)",
        band(0.0, 1e-9),
        pct(band(0.0, 1e-9)),
        band(1e-9, 3.0),
        pct(band(1e-9, 3.0)),
        band(3.0, 12.0),
        pct(band(3.0, 12.0)),
        band(12.0, 20.0),
        pct(band(12.0, 20.0)),
        band(20.0, 100.0),
        pct(band(20.0, 100.0)),
        band(100.0, f64::INFINITY),
        pct(band(100.0, f64::INFINITY)),
    );

    // Per-window pool rate, i.e. exactly the quantity `GroupRates::norm` divides
    // by its full scale before `MOTOR_TAU_S` smoothing.
    let win_ms = WINDOW_S as f64 * 1000.0;
    let scale = scale_rd as f64;
    let one_spike_hz = 1000.0 / win_ms / n_mem;
    let mut ac = a_cmd.clone();
    ac.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let aq = |p: f64| -> f64 {
        let k = ((ac.len() - 1) as f64 * p).round() as usize;
        ac[k.min(ac.len() - 1)] as f64
    };
    let amean = ac.iter().map(|&v| v as f64).sum::<f64>() / ac.len() as f64;
    let asd =
        (ac.iter().map(|&v| (v as f64 - amean).powi(2)).sum::<f64>() / ac.len() as f64).sqrt();
    let a_rail = ac.iter().filter(|&&v| v >= 1.0).count() as f64;
    let a_zero = ac.iter().filter(|&&v| v <= 0.0).count() as f64;
    let zero = win_hist[0] as f64;
    let rail = win_hist[members.len()] as f64;
    let one_mem = win_hist.iter().skip(1).map(|&c| c as u64).sum::<u64>() as f64;
    println!(
        "\n-- GRADEDNESS: the actuator command `a` the wing is given ({} windows of {:.1} ms, \
         read-out full scale {:.0} Hz, then the {:.0} ms `MOTOR_TAU_S` lag) --\n\
         a mean {:.4}  sd {:.4}  min {:.4}  p05 {:.4}  median {:.4}  p95 {:.4}  max {:.4}\n\
         windows with a EXACTLY 0.0:  {:>6.1}% ({:.0})   <- read-out pinned at zero\n\
         windows with a AT 1.0:       {:>6.1}% ({:.0})   <- read-out pinned at its rail\n\
         \x20 raw window facts: one member firing = {:.1} Hz/neuron = {:.2} of full scale, so at \
         this window length the RAW read-out is near-binary;\n\
         \x20 windows reading EXACTLY ZERO: {:>6.1}% ({:.0}); windows with >=1 of {} firing: \
         {:>6.1}% ({:.0}); windows with ALL {} firing: {:>6.1}% ({:.0})",
        windows,
        win_ms,
        scale,
        crate::sim::MOTOR_TAU_S * 1000.0,
        amean,
        asd,
        aq(0.0),
        aq(0.05),
        aq(0.50),
        aq(0.95),
        aq(1.0),
        100.0 * a_zero / windows as f64,
        a_zero,
        100.0 * a_rail / windows as f64,
        a_rail,
        one_spike_hz,
        one_spike_hz / scale,
        100.0 * zero / windows as f64,
        zero,
        members.len(),
        100.0 * one_mem / windows as f64,
        one_mem,
        members.len(),
        100.0 * rail / windows as f64,
        rail
    );
    // Verdict, stated as the measurement it is. The test is the pool's own
    // OUTPUT, not the command: a rate sitting at the refractory ceiling, or at
    // zero, is a switch; a distribution spread through the physiological band
    // that moves with the drive is graded.
    let frac_at_ceil = win_hist[members.len()] as f64 / windows as f64;
    println!(
        "  verdict: {} neurons; pool mean {:.1} Hz/neuron; per-neuron median {:.1} Hz; \
         {:.1}% of raw windows with all {} members firing (the spike-count ceiling); \
         actuator command a: mean {:.3} sd {:.3} min {:.3} max {:.3}. {}",
        members.len(),
        pool_hz,
        q(0.50),
        100.0 * frac_at_ceil,
        members.len(),
        amean,
        asd,
        aq(0.0),
        aq(1.0),
        if a_rail / windows as f64 > 0.95 {
            "The read-out is PINNED at its rail: this pool is a switch."
        } else if pool_hz > 100.0 {
            "The pool's RATE is above any measured DLM rate: still a switch, one notch down."
        } else if a_zero / windows as f64 > 0.95 {
            "The pool is silent: the read-out is PINNED at zero."
        } else if pool_hz < 1.0 {
            "The pool is nearly silent; the read-out is below its measured range."
        } else {
            "The read-out spans its range: the pool is GRADED, not pinned."
        }
    );

    // Whole-network rate histogram: is the pool an outlier, or is the whole
    // network hyperactive?
    let mut bins = [0usize; 10];
    for i in 0..n {
        let hz = w.lif.spike_count[i] as f64 / sim_s;
        let b = if hz == 0.0 {
            0
        } else if hz < 1.0 {
            1
        } else if hz < 5.0 {
            2
        } else if hz < 10.0 {
            3
        } else if hz < 20.0 {
            4
        } else if hz < 50.0 {
            5
        } else if hz < 100.0 {
            6
        } else if hz < 200.0 {
            7
        } else if hz < 455.0 {
            8
        } else {
            9
        };
        bins[b] += 1;
    }
    let labels = [
        "== 0", "(0,1)", "[1,5)", "[5,10)", "[10,20)", "[20,50)", "[50,100)", "[100,200)",
        "[200,455)", ">=455",
    ];
    print!("\n-- whole-network rate histogram --\n");
    for (i, l) in labels.iter().enumerate() {
        print!(
            "  {l:>9} Hz {:>8}  {:.2}%\n",
            bins[i],
            100.0 * bins[i] as f64 / n as f64
        );
    }

    // ----------------------------------------------------------- top drivers
    let mut drv: Vec<(u32, i64, bool)> =
        by_src.iter().map(|(&s, &(c, e))| (s, c, e)).collect();
    drv.sort_by(|a, b| b.1.cmp(&a.1));
    println!(
        "\n-- top {} presynaptic drivers of the pool, by |contacts|, with their own measured rate --",
        o.top
    );
    println!(
        "{:>8} {:>10} {:>8} {:>12} {:>10} {:>10}",
        "src", "contacts", "sign", "spikes", "Hz", "out_deg"
    );
    for &(s, c, e) in drv.iter().take(o.top) {
        let out = w.conn.row_ptr[s as usize + 1] as usize - w.conn.row_ptr[s as usize] as usize;
        println!(
            "{:>8} {:>10} {:>8} {:>12} {:>10.1} {:>10}",
            s,
            c,
            if e { "E" } else { "I" },
            w.lif.spike_count[s as usize],
            w.lif.spike_count[s as usize] as f64 / sim_s,
            out
        );
    }

    // How much of the pool's input comes from the single strongest driver, and
    // how many drivers are themselves running above 100 Hz? If a handful of
    // neurons do most of the driving, the pool's rate is inherited, not
    // generated by the pool.
    let total_contacts: i64 = drv.iter().map(|d| d.1).sum();
    let top10: i64 = drv.iter().take(10).map(|d| d.1).sum();
    let hot = drv
        .iter()
        .filter(|&&(s, _, _)| w.lif.spike_count[s as usize] as f64 / sim_s > 100.0)
        .count();
    let hot_contacts: i64 = drv
        .iter()
        .filter(|&&(s, _, _)| w.lif.spike_count[s as usize] as f64 / sim_s > 100.0)
        .map(|d| d.1)
        .sum();
    println!(
        "input concentration: top-10 drivers hold {:.1}% of the pool's {} contacts; \
         {} of {} drivers run >100 Hz and hold {:.1}% of the contacts",
        100.0 * top10 as f64 / total_contacts.max(1) as f64,
        total_contacts,
        hot,
        drv.len(),
        100.0 * hot_contacts as f64 / total_contacts.max(1) as f64
    );

    // ------------------------------------------------- read-out resolution
    println!(
        "\n-- what the 2 ms control window can resolve (the actual read-out) --"
    );
    let win_ms = WINDOW_S as f64 * 1000.0;
    let ceiling = 1000.0 / win_ms;
    let win_mean = win_hist
        .iter()
        .enumerate()
        .map(|(k, &c)| k as f64 * c as f64)
        .sum::<f64>()
        / windows as f64;
    println!(
        "window {:.1} ms -> arithmetic ceiling {:.0} Hz/neuron; one member firing in a window \
         reads {:.1} Hz; {} windows measured",
        win_ms,
        ceiling,
        ceiling / members.len() as f64,
        windows
    );
    println!(
        "members firing per window: mean {:.3} of {}; windows with ZERO of the pool firing: \
         {:.1}% ({})",
        win_mean,
        members.len(),
        100.0 * win_hist[0] as f64 / windows as f64,
        win_hist[0]
    );
    print!("count histogram:");
    for (k, &c) in win_hist.iter().enumerate() {
        if c > 0 {
            print!(" {k}:{c}");
        }
    }
    print!("\n");
    // The same arithmetic for the signal the biology actually asks for.
    for r in [5.0f64, 12.0, 20.0] {
        let p_one = 1.0 - (-r * win_ms / 1000.0).exp(); // Poisson P(>=1 spike in the window)
        println!(
            "a member firing at {:>4.0} Hz has P(at least one spike in a {:.1} ms window) = {:.3}; \
             a physiological {:.0} Hz command is one spike per {:.0} ms",
            r,
            win_ms,
            p_one,
            r,
            1000.0 / r
        );
    }

    // ------------------------------------------------------------ mean field
    // Predicted steady-state depolarisation from the partners' MEASURED rates.
    // For this parameterisation `(v - REST)* = g * coupling / (1 - decay_m)` and
    // `g* = arrivals/(1 - decay_s)` with arrivals = sum_i 0.275*c_i * r_i * dt,
    // so the whole chain collapses to a single constant times
    // sum(c_i * r_i) -- the contact-weighted input rate.
    let ds = w.lif.decay_s as f64;
    let dm = w.lif.decay_m as f64;
    let coup = w.lif.coupling as f64;
    let k = 1e-4 * WEIGHT_MV as f64 / (1.0 - ds) * coup / (1.0 - dm);
    let gap = (crate::lif::THRESHOLD_MV - crate::lif::REST_MV) as f64;
    println!(
        "\n-- mean field: does the measured input alone explain the measured rate? --\n\
         predicted (v-REST) = k * gain * sum(c_i * r_i), k = {k:.3e} mV per (contact*Hz); \
         threshold gap = {gap:.1} mV\n\
         per-cell input gain on this pool: {:.6} ({} cells; lif::Lif::gain, see \
         sim::MN_POWER_INPUT_GAIN)",
        w.mn_gain, w.mn_cells
    );
    println!(
        "{:>8} {:>14} {:>14} {:>12} {:>12} {:>12}",
        "model", "sum(c*r)", "pred_mV", "x_gap", "obs_Hz", "partner_Hz"
    );
    let mut pred_sum = 0.0f64;
    let mut obs_sum = 0.0f64;
    let mut net_contacts_sum = 0.0f64;
    for (kk, &mm) in members.iter().enumerate() {
        let scr: f64 = member_in[kk]
            .iter()
            .map(|&(s, c)| {
                c as f64 * (w.lif.spike_count[s as usize] as f64 / sim_s)
            })
            .sum();
        let net_c: f64 = member_in[kk].iter().map(|&(_, c)| c as f64).sum();
        let pred = k * w.lif.gain[mm as usize] as f64 * scr;
        let obs = w.lif.spike_count[mm as usize] as f64 / sim_s;
        pred_sum += pred;
        obs_sum += obs;
        net_contacts_sum += net_c;
        println!(
            "{:>8} {:>14.0} {:>14.1} {:>12.2} {:>12.1} {:>12.2}",
            mm,
            scr,
            pred,
            pred / gap,
            obs,
            scr / net_c.abs().max(1.0)
        );
    }
    let n_mem = members.len() as f64;
    println!(
        "pool mean: predicted depolarisation {:.1} mV = {:.1}x the {:.1} mV threshold gap; \
         observed {:.1} Hz",
        pred_sum / n_mem,
        pred_sum / n_mem / gap,
        gap,
        obs_sum / n_mem
    );
    // The mean partner rate at which threshold is first reached, if every
    // partner fired at the same rate.
    let c_mean = net_contacts_sum / n_mem;
    println!(
        "each member's net signed contact count averages {:.0}; threshold is reached when the \
         contact-weighted mean partner rate is {:.2} Hz -- the network mean is {:.2} Hz, i.e. \
         {:.1}x that",
        c_mean,
        (gap / k) / c_mean.abs().max(1.0),
        net_hz,
        net_hz / ((gap / k) / c_mean.abs().max(1.0))
    );

    Ok(())
}