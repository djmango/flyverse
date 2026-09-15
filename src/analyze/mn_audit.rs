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
    for _ in 0..windows {
        w.advance();
        let mut cnt = 0u32;
        for &i in &w.window_spikes {
            if inset[i as usize] {
                cnt += 1;
            }
        }
        win_hist[cnt as usize] += 1;
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
         predicted (v-REST) = k * sum(c_i * r_i), k = {k:.3e} mV per (contact*Hz); \
         threshold gap = {gap:.1} mV"
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
        let pred = k * scr;
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