//! Parity test: the WebAssembly engine against the native engine.
//!
//! Both run the real 4,022-neuron / 407,720-edge leg motor pool slice, with the
//! same Poisson drive at the same rate and seed, for 5,000 steps (500 ms of fly
//! time). The test compares every step's fired set, the per-neuron spike counts,
//! the total, and a FNV-1a hash of the whole spike log.
//!
//! Regenerate the slice with:
//!   /opt/data/workspaces/skg/flybrain/venv/bin/python scripts/export_slice.py

use std::path::{Path, PathBuf};

use fly_lif_wasm::Engine;
use flyverse::lif::{Lif, StimGen};
use flyverse::npy::Npy;
use flyverse::pack::Connectome;

const STEPS: usize = 5_000;
const RATE_HZ: f64 = 150.0;
const SEED: u64 = 7;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn hash_pair(h: &mut u64, x: u32) {
    *h ^= x as u64;
    *h = h.wrapping_mul(FNV_PRIME);
}

fn slice_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../build/slice-pack")
}

fn targets() -> Vec<u32> {
    let raw = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../build/fly-lif-stim.u32"))
        .expect("run scripts/export_slice.py first");
    raw.chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn wasm_engine_matches_native_engine() {
    let dir = slice_dir();
    let conn = Connectome::load(&dir).expect("load slice pack");
    let npy = Npy::open(&dir.join("signed_counts.npy")).expect("open signed_counts");
    let signed = npy
        .i16(&dir.join("signed_counts.npy"))
        .expect("read signed_counts");
    let targets = targets();
    assert_eq!(conn.n, 4_022, "slice shape changed");
    assert_eq!(conn.m, 407_720, "slice shape changed");
    assert!(!targets.is_empty(), "no stimulus targets");

    // Native engine.
    let mut native = Lif::new(&conn);
    let mut stim = StimGen::new(targets.clone(), RATE_HZ, SEED);
    let mut native_counts = vec![0u32; conn.n];
    let mut native_hash = FNV_OFFSET;
    let mut native_steps: Vec<Vec<u32>> = Vec::with_capacity(STEPS);
    for step in 0..STEPS {
        let ev = stim.next_events().to_vec();
        native.step(&conn, &ev);
        for &i in native.fired.iter() {
            native_counts[i as usize] += 1;
            hash_pair(&mut native_hash, step as u32);
            hash_pair(&mut native_hash, i);
        }
        native_steps.push(native.fired.clone());
    }

    // WebAssembly engine, same slice and same stimulus.
    let mut wasm = Engine::new(conn.n, conn.m);
    wasm.load_csr(conn.row_ptr, conn.destinations, signed);
    wasm.set_stim(targets.clone(), RATE_HZ as f32, SEED);
    let mut first_divergence: Option<(usize, Vec<u32>, Vec<u32>)> = None;
    for (step, native_fired) in native_steps.iter().enumerate() {
        wasm.run_one();
        if wasm.fired != *native_fired && first_divergence.is_none() {
            first_divergence = Some((step, native_fired.clone(), wasm.fired.clone()));
        }
    }

    let wasm_hash = wasm.hash;
    let total_native: u64 = native_counts.iter().map(|&c| c as u64).sum();
    let active_native = native_counts.iter().filter(|&&c| c > 0).count();
    let active_wasm = wasm.counts.iter().filter(|&&c| c > 0).count();

    println!("steps              {STEPS}");
    println!("native  spikes     {total_native}  active {active_native}");
    println!("wasm    spikes     {}  active {active_wasm}", wasm.total_spikes);
    println!("native  hash       {native_hash:016x}");
    println!("wasm    hash       {wasm_hash:016x}");

    if let Some((step, a, b)) = &first_divergence {
        println!("first divergence at step {step}: native {a:?} wasm {b:?}");
    }

    assert_eq!(wasm.total_spikes, total_native, "spike totals differ");
    assert_eq!(wasm.counts, native_counts, "per-neuron spike counts differ");
    assert_eq!(wasm_hash, native_hash, "spike log hash differs");

    // Record what was verified, so the shipped page can check itself against it
    // instead of trusting a number typed into the source by hand.
    let record = format!(
        "{{\n\
         \x20 \"steps\": {STEPS},\n\
         \x20 \"rate_hz\": {RATE_HZ},\n\
         \x20 \"seed\": {SEED},\n\
         \x20 \"stimulus\": \"vnc_sensory\",\n\
         \x20 \"spikes\": {total_native},\n\
         \x20 \"active_neurons\": {active_native},\n\
         \x20 \"hash\": \"{native_hash:016x}\",\n\
         \x20 \"engines\": \"native Rust (rayon) vs wasm32-unknown-unknown, same slice and stimulus\",\n\
         \x20 \"compared\": \"every step's fired set, per-neuron counts, and a FNV-1a hash of the spike log\"\n\
         }}\n"
    );
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../build/parity.json");
    std::fs::write(&out, record).expect("write build/parity.json");
    println!("wrote {}", out.display());
}

#[test]
fn wasm_engine_hashes_are_stable() {
    // Two identical runs in the same process must agree, so the browser can
    // compare its own hash against the number this test prints.
    let dir = slice_dir();
    let conn = Connectome::load(&dir).expect("load slice pack");
    let npy = Npy::open(&dir.join("signed_counts.npy")).expect("open signed_counts");
    let signed = npy.i16(&dir.join("signed_counts.npy")).expect("read");
    let targets = targets();

    let mut hashes = Vec::new();
    for _ in 0..2 {
        let mut e = Engine::new(conn.n, conn.m);
        e.load_csr(conn.row_ptr, conn.destinations, signed);
        e.set_stim(targets.clone(), RATE_HZ as f32, SEED);
        for _ in 0..STEPS {
            e.run_one();
        }
        hashes.push(e.hash);
    }
    println!("stable hash {:#018x}", hashes[0]);
    assert_eq!(hashes[0], hashes[1]);
}
