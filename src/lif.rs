//! Leaky integrate-and-fire engine over the MaleCNS CSR graph.
//!
//! This reproduces the published point-neuron convention used by our NumPy
//! reference engine (and by the upstream FlyWire/Brian2 model) exactly:
//!
//!   dt            0.1 ms
//!   resting/reset -52 mV
//!   threshold     -45 mV   (fires when v >= threshold)
//!   tau membrane  20 ms
//!   tau synapse   5 ms
//!   refractory    2.2 ms
//!   delay         1.8 ms
//!   weight        0.275 mV per signed contact count
//!
//! Per tick, in this order:
//!   1. read the delay-ring slot for this step, then clear it
//!   2. g = g*decay_s + arrivals
//!   3. v = rest + (v - rest)*decay_m + g*coupling
//!   4. add direct external input to stimulated neurons
//!   5. decrement refractory (clamped at 0)
//!   6. fire where refractory == 0 and v >= threshold: reset v and g
//!   7. propagate spikes through CSR into the ring slot read `delay` later
//!
//! The float32 evaluation order matches the NumPy reference operation for
//! operation, and this CPU has no FMA, so no contraction can perturb rounding.
//!
//! Performance shape: the only dense work is one fused pass over v, g and the
//! refractory counter per tick, which is 1.7 MB and stays cache resident. The
//! ring slot is read densely (it is added to every neuron's conductance) but
//! cleared with a single memset, and spiking is propagated sparsely. Firing
//! detection writes directly into a bitmask, so a tick allocates nothing.

use crate::pack::Connectome;
use rayon::prelude::*;

pub const DT_MS: f32 = 0.1;
pub const REST_MV: f32 = -52.0;
pub const THRESHOLD_MV: f32 = -45.0;
pub const TAU_M_MS: f64 = 20.0;
pub const TAU_S_MS: f64 = 5.0;
pub const REFRACTORY_MS: f64 = 2.2;
pub const DELAY_MS: f64 = 1.8;
pub const POISSON_MV: f32 = 68.75;

const CHUNK: usize = 2048;
const WORDS: usize = CHUNK / 64;

#[inline]
fn mask_words(n: usize) -> usize {
    n.div_ceil(CHUNK) * WORDS
}

pub struct Lif {
    pub n: usize,
    pub delay_steps: usize,
    pub refractory_steps: i16,
    pub v: Vec<f32>,
    pub g: Vec<f32>,
    pub refractory: Vec<i16>,
    /// Delay ring: `slots` rows of `n` floats.
    pub ring: Vec<f32>,
    pub slots: usize,
    pub slot: usize,
    pub decay_m: f32,
    pub decay_s: f32,
    pub coupling: f32,
    pub step_index: u64,
    pub spike_count: Vec<u32>,
    pub active_neurons: u32,
    pub total_spikes: u64,
    pub spike_seq: u64,
    /// Recent spike deltas: (seq, model index) pairs, capped.
    pub recent: Vec<(u64, u32)>,
    pub recent_cap: usize,
    /// Model indices that fired during the most recent `step`, ascending.
    pub fired: Vec<u32>,
    /// Destinations written into each ring slot since it was last cleared, so
    /// the slot can be reset sparsely instead of with a full memset.
    slot_writes: Vec<Vec<u32>>,
    /// Firing bitmask, one bit per neuron, reused every tick.
    pub mask: Vec<u64>,
    pub profiling: bool,
    pub dense_fires: u32,
    pub t_dense: f64,
    pub t_clear: f64,
    pub t_scan: f64,
    pub t_prop: f64,
}

impl Lif {
    pub fn new(c: &Connectome) -> Lif {
        let n = c.n;
        let delay_steps = (DELAY_MS / DT_MS as f64).round() as usize;
        let slots = delay_steps + 1;
        let decay_m = (-(DT_MS as f64) / TAU_M_MS).exp() as f32;
        let decay_s = (-(DT_MS as f64) / TAU_S_MS).exp() as f32;
        let coupling =
            (TAU_S_MS / (TAU_M_MS - TAU_S_MS) * (decay_m as f64 - decay_s as f64)) as f32;
        Lif {
            n,
            delay_steps,
            refractory_steps: (REFRACTORY_MS / DT_MS as f64).round() as i16,
            v: vec![REST_MV; n],
            g: vec![0.0; n],
            refractory: vec![0; n],
            ring: vec![0.0; slots * n],
            slots,
            slot: 0,
            decay_m,
            decay_s,
            coupling,
            step_index: 0,
            spike_count: vec![0; n],
            active_neurons: 0,
            total_spikes: 0,
            spike_seq: 0,
            recent: Vec::with_capacity(1 << 16),
            recent_cap: 1 << 20,
            fired: Vec::with_capacity(512),
            slot_writes: (0..slots).map(|_| Vec::new()).collect(),
            mask: vec![0u64; mask_words(n)],
            profiling: false,
            dense_fires: 0,
            t_dense: 0.0,
            t_clear: 0.0,
            t_scan: 0.0,
            t_prop: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.v.fill(REST_MV);
        self.g.fill(0.0);
        self.refractory.fill(0);
        self.ring.fill(0.0);
        self.slot = 0;
        self.step_index = 0;
        self.spike_count.fill(0);
        self.active_neurons = 0;
        self.total_spikes = 0;
        self.spike_seq = 0;
        self.recent.clear();
        self.fired.clear();
        for w in self.slot_writes.iter_mut() {
            w.clear();
        }
    }

    /// One 0.1 ms tick. `stim` is a list of (model index, count) direct inputs.
    pub fn step(&mut self, c: &Connectome, stim: &[(u32, u32)]) {
        let n = self.n;
        let read_slot = self.slot;
        let store_slot = (self.slot + self.delay_steps - 1) % self.slots;

        let ds = self.decay_s;
        let dm = self.decay_m;
        let coup = self.coupling;
        let prof = self.profiling;
        let mut t_dense = 0.0f64;
        let mut t_clear = 0.0f64;
        let mut t_scan = 0.0f64;
        let mut t_prop = 0.0f64;

        let mut t = std::time::Instant::now();
        {
            let v = &mut self.v;
            let g = &mut self.g;
            let rf = &mut self.refractory;
            let mask = &mut self.mask;
            let arrivals = &self.ring[read_slot * n..(read_slot + 1) * n];
            let fire_total: u32 = v
                .par_chunks_mut(CHUNK)
                .zip(g.par_chunks_mut(CHUNK))
                .zip(rf.par_chunks_mut(CHUNK))
                .zip(arrivals.par_chunks(CHUNK))
                .zip(mask.par_chunks_mut(WORDS))
                .map(|((((vc, gc), rfc), ac), mc)| {
                    let mut local_fires: u32 = 0;
                    for (wi, word) in mc.iter_mut().enumerate() {
                        let base = wi * 64;
                        let end = (base + 64).min(vc.len());
                        let mut acc: u64 = 0;
                        for i in base..end {
                            let gg = gc[i] * ds + ac[i];
                            gc[i] = gg;
                            let vv = REST_MV + (vc[i] - REST_MV) * dm + gg * coup;
                            vc[i] = vv;
                            let r = rfc[i] - 1;
                            let r = if r < 0 { 0 } else { r };
                            rfc[i] = r;
                            if r == 0 && vv >= THRESHOLD_MV {
                                acc |= 1u64 << (i - base);
                                local_fires += 1;
                            }
                        }
                        *word = acc;
                    }
                    local_fires
                })
                .sum();
            self.dense_fires = fire_total;
        }
        if prof {
            t_dense = t.elapsed().as_secs_f64();
            t = std::time::Instant::now();
        }

        // Clear the slot we just read.
        self.ring[read_slot * n..(read_slot + 1) * n].fill(0.0);
        self.slot = (self.slot + 1) % self.slots;

        // Direct external input.
        if !stim.is_empty() {
            for &(idx, count) in stim {
                let k = idx as usize;
                if k < n {
                    self.v[k] += POISSON_MV * count as f32;
                }
            }
        }
        if prof {
            t_clear = t.elapsed().as_secs_f64();
            t = std::time::Instant::now();
        }

        // Extract firing indices from the bitmask, ascending by construction.
        self.fired.clear();
        for (wi, &w) in self.mask.iter().enumerate() {
            let mut bits = w;
            if bits == 0 {
                continue;
            }
            let base = wi * 64;
            while bits != 0 {
                let b = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if base + b < n {
                    self.fired.push((base + b) as u32);
                }
            }
        }
        if prof {
            t_scan = t.elapsed().as_secs_f64();
            t = std::time::Instant::now();
        }

        if !self.fired.is_empty() {
            // Ascending model index, so the float accumulation order into each
            // destination matches the reference bincount exactly.
            let fired = std::mem::take(&mut self.fired);
            {
                let v = &mut self.v;
                let g = &mut self.g;
                let rf = &mut self.refractory;
                let store = &mut self.ring[store_slot * n..(store_slot + 1) * n];
                for &i in &fired {
                    let k = i as usize;
                    v[k] = REST_MV;
                    g[k] = 0.0;
                    rf[k] = self.refractory_steps;
                    let a = c.row_ptr[k] as usize;
                    let b = c.row_ptr[k + 1] as usize;
                    let dests = &c.destinations[a..b];
                    let ws = &c.edge_write_mv[a..b];
                    // Destinations are validated in range at pack load time.
                    for (d, w) in dests.iter().zip(ws.iter()) {
                        unsafe {
                            *store.get_unchecked_mut(*d as usize) += *w;
                        }
                    }
                }
            }
            {
                let sc = &mut self.spike_count;
                let recent = &mut self.recent;
                let mut seq = self.spike_seq;
                let mut active = self.active_neurons;
                let cap = self.recent_cap;
                for &i in &fired {
                    let k = i as usize;
                    if sc[k] == 0 {
                        active += 1;
                    }
                    sc[k] += 1;
                    seq += 1;
                    if recent.len() < cap {
                        recent.push((seq, i));
                    }
                }
                self.spike_seq = seq;
                self.active_neurons = active;
                self.total_spikes += fired.len() as u64;
            }
            self.fired = fired;
        }
        if prof {
            t_prop = t.elapsed().as_secs_f64();
            self.t_dense += t_dense;
            self.t_clear += t_clear;
            self.t_scan += t_scan;
            self.t_prop += t_prop;
        }
        self.step_index += 1;
    }
}

/// Counter-based deterministic stimulus generator (SplitMix64), so runs are
/// reproducible and independent of the OS RNG.
pub struct StimGen {
    targets: Vec<u32>,
    rate_hz: f64,
    seed: u64,
    step: u64,
    events: Vec<(u32, u32)>,
}

impl StimGen {
    pub fn new(targets: Vec<u32>, rate_hz: f64, seed: u64) -> StimGen {
        StimGen { targets, rate_hz, seed, step: 0, events: Vec::with_capacity(1024) }
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    fn u01(seed: u64, step: u64, j: u64) -> f64 {
        let mut z = seed
            .wrapping_add(step.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add(j.wrapping_mul(0xBF58_476D_1CE4_E5B9));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Poisson drive: each target fires with probability rate * dt.
    pub fn next_events(&mut self) -> &[(u32, u32)] {
        let p = self.rate_hz * DT_MS as f64 / 1000.0;
        self.events.clear();
        for (j, &t) in self.targets.iter().enumerate() {
            if Self::u01(self.seed, self.step, j as u64) < p {
                self.events.push((t, 1));
            }
        }
        self.step += 1;
        &self.events
    }
}
