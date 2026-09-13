//! The FlyVerse leaky-integrate-and-fire engine as a WebAssembly module.
//!
//! This is the same integrator, the same constants and the same operation order
//! as `src/lif.rs` in the native engine, written single-threaded so it can run
//! in a browser. The native engine's parallel dense pass and this one are not
//! assumed to agree: `tests/parity.rs` runs both over the same slice with the
//! same stimulus and compares every step, every spike index and a hash of the
//! whole spike log. If that test passes, the browser is running the engine that
//! produced the native numbers, not a lookalike.
//!
//! Per tick, in this order (identical to the native engine):
//!   1. g = g*decay_s + arrivals(read slot)
//!   2. v = rest + (v - rest)*decay_m + g*coupling
//!   3. decrement refractory, clamped at 0
//!   4. fire where refractory == 0 and v >= threshold
//!   5. clear the read slot, advance the ring
//!   6. add direct external input to stimulated neurons
//!   7. reset fired neurons and propagate their spikes into the slot read
//!      `delay` steps later
//!
//! The ABI is plain C over the module's linear memory, so the page loads it with
//! `WebAssembly.instantiate` and no glue code.

#![allow(static_mut_refs)]

use std::alloc::{alloc, dealloc, Layout};

pub const DT_MS: f32 = 0.1;
pub const REST_MV: f32 = -52.0;
pub const THRESHOLD_MV: f32 = -45.0;
pub const TAU_M_MS: f64 = 20.0;
pub const TAU_S_MS: f64 = 5.0;
pub const REFRACTORY_MS: f64 = 2.2;
pub const DELAY_MS: f64 = 1.8;
pub const WEIGHT_MV: f32 = 0.275;
pub const POISSON_MV: f32 = 68.75;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub struct Engine {
    pub n: usize,
    pub m: usize,
    row_ptr: Vec<u32>,
    dst: Vec<u32>,
    cnt: Vec<i16>,
    v: Vec<f32>,
    g: Vec<f32>,
    rf: Vec<i16>,
    ring: Vec<f32>,
    slots: usize,
    slot: usize,
    delay_steps: usize,
    refractory_steps: i16,
    decay_m: f32,
    decay_s: f32,
    coupling: f32,
    // Poisson drive
    targets: Vec<u32>,
    rate_hz: f64,
    seed: u64,
    stim_step: u64,
    pending: Vec<(u32, u32)>,
    // read-out
    pub fired: Vec<u32>,
    pub counts: Vec<u32>,
    pub total_spikes: u64,
    pub steps: u64,
    pub active: u32,
    pub gain: f32,
    pub inhibition: bool,
    pub hash: u64,
    /// One byte per neuron, set when it fired during the current frame. Lets the
    /// page draw a frame's spikes with a single read instead of a per-step call.
    frame_mask: Vec<u8>,
}

impl Engine {
    pub fn new(n: usize, m: usize) -> Engine {
        let decay_m = (-(DT_MS as f64) / TAU_M_MS).exp() as f32;
        let decay_s = (-(DT_MS as f64) / TAU_S_MS).exp() as f32;
        let coupling = (TAU_S_MS / (TAU_M_MS - TAU_S_MS) * (decay_m as f64 - decay_s as f64)) as f32;
        let delay_steps = (DELAY_MS / DT_MS as f64).round() as usize;
        let slots = delay_steps + 1;
        Engine {
            n,
            m,
            row_ptr: vec![0; n + 1],
            dst: vec![0; m],
            cnt: vec![0; m],
            v: vec![REST_MV; n],
            g: vec![0.0; n],
            rf: vec![0; n],
            ring: vec![0.0; slots * n],
            slots,
            slot: 0,
            delay_steps,
            refractory_steps: (REFRACTORY_MS / DT_MS as f64).round() as i16,
            decay_m,
            decay_s,
            coupling,
            targets: Vec::new(),
            rate_hz: 150.0,
            seed: 7,
            stim_step: 0,
            pending: Vec::new(),
            fired: Vec::with_capacity(1024),
            counts: vec![0; n],
            total_spikes: 0,
            steps: 0,
            active: 0,
            gain: 1.0,
            inhibition: true,
            hash: FNV_OFFSET,
            frame_mask: vec![0; n],
        }
    }

    pub fn reset(&mut self) {
        self.v.fill(REST_MV);
        self.g.fill(0.0);
        self.rf.fill(0);
        self.ring.fill(0.0);
        self.slot = 0;
        self.stim_step = 0;
        self.pending.clear();
        self.fired.clear();
        self.counts.fill(0);
        self.total_spikes = 0;
        self.steps = 0;
        self.active = 0;
        self.hash = FNV_OFFSET;
        self.frame_mask.fill(0);
    }

    fn hash_u32(&mut self, x: u32) {
        self.hash ^= x as u64;
        self.hash = self.hash.wrapping_mul(FNV_PRIME);
    }

    /// SplitMix64, the native engine's counter-based stimulus stream.
    fn u01(seed: u64, step: u64, j: u64) -> f64 {
        let mut z = seed
            .wrapping_add(step.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add(j.wrapping_mul(0xBF58_476D_1CE4_E5B9));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }

    /// One Poisson step: every target fires with probability rate * dt.
    fn poisson_events(&mut self) {
        let p = self.rate_hz * DT_MS as f64 / 1000.0;
        let seed = self.seed;
        let step = self.stim_step;
        let mut events: Vec<(u32, u32)> = Vec::new();
        for (j, &t) in self.targets.iter().enumerate() {
            if Self::u01(seed, step, j as u64) < p {
                events.push((t, 1));
            }
        }
        self.stim_step += 1;
        self.pending.extend(events);
    }

    pub fn step(&mut self) {
        let n = self.n;
        let read_slot = self.slot;
        let store_slot = (self.slot + self.delay_steps - 1) % self.slots;
        let (ds, dm, coup) = (self.decay_s, self.decay_m, self.coupling);

        // 1-4: one fused pass over the dense state.
        self.fired.clear();
        {
            let ring = &self.ring[read_slot * n..(read_slot + 1) * n];
            let v = &mut self.v;
            let g = &mut self.g;
            let rf = &mut self.rf;
            let fired = &mut self.fired;
            let frame = &mut self.frame_mask;
            for i in 0..n {
                let gg = g[i] * ds + ring[i];
                g[i] = gg;
                let vv = REST_MV + (v[i] - REST_MV) * dm + gg * coup;
                v[i] = vv;
                let r = rf[i] - 1;
                let r = if r < 0 { 0 } else { r };
                rf[i] = r;
                if r == 0 && vv >= THRESHOLD_MV {
                    fired.push(i as u32);
                    frame[i] = 1;
                }
            }
        }

        // 5: clear the slot just read, advance the ring.
        self.ring[read_slot * n..(read_slot + 1) * n].fill(0.0);
        self.slot = (self.slot + 1) % self.slots;

        // 6: direct external input.
        self.pending.retain(|&(idx, count)| idx < n as u32 && count > 0);
        for k in 0..self.pending.len() {
            let (idx, count) = self.pending[k];
            self.v[idx as usize] += POISSON_MV * count as f32;
        }
        self.pending.clear();

        // 7: reset and propagate.
        let fired_len = self.fired.len();
        if fired_len > 0 {
            for fi in 0..fired_len {
                let i = self.fired[fi] as usize;
                self.v[i] = REST_MV;
                self.g[i] = 0.0;
                self.rf[i] = self.refractory_steps;
                let a = self.row_ptr[i] as usize;
                let b = self.row_ptr[i + 1] as usize;
                let base = store_slot * n;
                let gain = self.gain;
                let inhibition = self.inhibition;
                // Same accumulation order as the native engine: sources
                // ascending, destinations in CSR order.
                for e in a..b {
                    let c = self.cnt[e] as f32;
                    if !inhibition && c < 0.0 {
                        continue;
                    }
                    let w = c * WEIGHT_MV * gain;
                    let d = self.dst[e] as usize;
                    unsafe {
                        let p = self.ring.as_mut_ptr().add(base + d);
                        *p += w;
                    }
                }
            }
            for fi in 0..fired_len {
                let i = self.fired[fi] as usize;
                if self.counts[i] == 0 {
                    self.active += 1;
                }
                self.counts[i] += 1;
            }
            self.total_spikes += fired_len as u64;
            // Hash: (step index, neuron index) for every spike, in order.
            let step = self.steps as u32;
            for fi in 0..fired_len {
                let i = self.fired[fi];
                self.hash_u32(step);
                self.hash_u32(i);
            }
        }

        self.steps += 1;
    }

    pub fn run(&mut self, steps: u32) -> u32 {
        let mut spikes = 0u32;
        for _ in 0..steps {
            self.run_one();
            spikes += self.fired.len() as u32;
        }
        spikes
    }

    /// Copy a CSR connectome in. `cnt` are the signed anatomical contact counts.
    pub fn load_csr(&mut self, row_ptr: &[u32], dst: &[u32], cnt: &[i16]) -> u32 {
        assert_eq!(row_ptr.len(), self.n + 1, "row_ptr length");
        assert_eq!(dst.len(), self.m, "destination length");
        assert_eq!(cnt.len(), self.m, "count length");
        self.row_ptr.copy_from_slice(row_ptr);
        self.dst.copy_from_slice(dst);
        self.cnt.copy_from_slice(cnt);
        if self.row_ptr[0] != 0 {
            return 1;
        }
        let mut prev = 0u32;
        for &v in self.row_ptr.iter() {
            if v < prev || v as usize > self.m {
                return 2;
            }
            prev = v;
        }
        if self.row_ptr[self.n] as usize != self.m {
            return 3;
        }
        for &d in self.dst.iter() {
            if d as usize >= self.n {
                return 4;
            }
        }
        0
    }

    pub fn set_stim(&mut self, targets: Vec<u32>, rate_hz: f32, seed: u64) {
        self.targets = targets;
        self.rate_hz = rate_hz as f64;
        self.seed = seed;
        self.stim_step = 0;
    }

    /// Start a new display frame: forget which neurons fired in the last one.
    pub fn frame_begin(&mut self) {
        self.frame_mask.fill(0);
    }

    pub fn frame_ptr(&self) -> u32 {
        self.frame_mask.as_ptr() as u32
    }

    /// One step, including the Poisson drive for that step when targets are set.
    pub fn run_one(&mut self) {
        if !self.targets.is_empty() {
            self.poisson_events();
        }
        self.step();
    }
}

static mut ENGINE: Option<Engine> = None;

fn eng() -> &'static mut Engine {
    unsafe { ENGINE.as_mut().expect("fl_init must be called first") }
}

#[no_mangle]
pub extern "C" fn fl_version() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn fl_alloc(bytes: u32) -> u32 {
    unsafe {
        let layout = Layout::from_size_align_unchecked(bytes.max(1) as usize, 8);
        alloc(layout) as u32
    }
}

#[no_mangle]
pub extern "C" fn fl_free(ptr: u32, bytes: u32) {
    unsafe {
        let layout = Layout::from_size_align_unchecked(bytes.max(1) as usize, 8);
        dealloc(ptr as *mut u8, layout);
    }
}

/// Create the engine. Returns 1 on success, 0 if the shape is impossible.
#[no_mangle]
pub extern "C" fn fl_init(n: u32, m: u32) -> u32 {
    if n == 0 || n > 4_000_000 || m > 60_000_000 {
        return 0;
    }
    unsafe {
        ENGINE = Some(Engine::new(n as usize, m as usize));
    }
    1
}

/// Copy the CSR arrays out of wasm memory (little-endian u32 / u32 / i16).
/// Returns 0 on success, or the validation code from `Engine::load_csr`.
#[no_mangle]
pub extern "C" fn fl_load(row_ptr: u32, dst: u32, cnt: u32) -> u32 {
    let e = eng();
    unsafe {
        let rp = std::slice::from_raw_parts(row_ptr as *const u32, e.n + 1);
        let de = std::slice::from_raw_parts(dst as *const u32, e.m);
        let cn = std::slice::from_raw_parts(cnt as *const i16, e.m);
        e.load_csr(rp, de, cn)
    }
}

/// Stimulus targets (local indices) and the Poisson rate in Hz.
#[no_mangle]
pub extern "C" fn fl_set_stim(targets: u32, len: u32, rate_hz: f32, seed_lo: u32, seed_hi: u32) {
    let e = eng();
    let t: Vec<u32> = if len > 0 {
        let s = unsafe { std::slice::from_raw_parts(targets as *const u32, len as usize) };
        s.to_vec()
    } else {
        Vec::new()
    };
    let seed = (seed_lo as u64) | ((seed_hi as u64) << 32);
    e.set_stim(t, rate_hz, seed);
}

/// Queue one direct input (the interactive "poke this neuron" channel).
#[no_mangle]
pub extern "C" fn fl_inject(idx: u32, count: u32) {
    eng().pending.push((idx, count));
}

#[no_mangle]
pub extern "C" fn fl_run(steps: u32) -> u32 {
    eng().run(steps)
}

#[no_mangle]
pub extern "C" fn fl_step() -> u32 {
    let e = eng();
    e.run_one();
    e.fired.len() as u32
}

#[no_mangle]
pub extern "C" fn fl_reset() {
    eng().reset();
}

#[no_mangle]
pub extern "C" fn fl_set_gain(gain: f32) {
    eng().gain = gain;
}

#[no_mangle]
pub extern "C" fn fl_set_inhibition(on: u32) {
    eng().inhibition = on != 0;
}

#[no_mangle]
pub extern "C" fn fl_fired_ptr() -> u32 {
    eng().fired.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn fl_fired_len() -> u32 {
    eng().fired.len() as u32
}

/// Clear the per-frame fired mask (call once per rendered frame).
#[no_mangle]
pub extern "C" fn fl_frame_begin() {
    eng().frame_begin();
}

/// Pointer to the u8[neuron_count] mask of neurons that fired this frame.
#[no_mangle]
pub extern "C" fn fl_frame_ptr() -> u32 {
    eng().frame_ptr()
}

#[no_mangle]
pub extern "C" fn fl_counts_ptr() -> u32 {
    eng().counts.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn fl_v_ptr() -> u32 {
    eng().v.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn fl_total_spikes() -> u32 {
    eng().total_spikes as u32
}

#[no_mangle]
pub extern "C" fn fl_active() -> u32 {
    eng().active
}

#[no_mangle]
pub extern "C" fn fl_steps() -> u32 {
    eng().steps as u32
}

#[no_mangle]
pub extern "C" fn fl_hash_lo() -> u32 {
    (eng().hash & 0xffff_ffff) as u32
}

#[no_mangle]
pub extern "C" fn fl_hash_hi() -> u32 {
    (eng().hash >> 32) as u32
}

#[no_mangle]
pub extern "C" fn fl_mem_bytes() -> u32 {
    core::mem::size_of::<Engine>() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_runs_and_hashes() {
        let mut e = Engine::new(64, 128);
        for i in 0..64 {
            e.row_ptr[i] = (i * 2) as u32;
            e.dst[2 * i] = ((i + 1) % 64) as u32;
            e.dst[2 * i + 1] = ((i + 7) % 64) as u32;
            e.cnt[2 * i] = 8;
            e.cnt[2 * i + 1] = -4;
        }
        e.row_ptr[64] = 128;
        e.targets = (0..8).collect();
        e.run(2000);
        assert!(e.total_spikes > 0, "network should fire with a 150 Hz drive");
        assert_eq!(e.steps, 2000);
    }
}
