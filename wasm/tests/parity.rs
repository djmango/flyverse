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

#[test]
fn closed_loop_is_deterministic_and_changes_the_neurons() {
    // The leg model sits in the same crate as the integrator, so it gets the same
    // treatment: run it twice, demand the same answer, and demand that closing
    // the loop actually reaches the neurons.
    //
    // The indices below are arbitrary. This test is about the loop arithmetic and
    // its determinism, not about which cells are proprioceptive: the real group
    // split is checked in the browser against the shipped asset.
    let dir = slice_dir();
    let conn = Connectome::load(&dir).expect("load slice pack");
    let npy = Npy::open(&dir.join("signed_counts.npy")).expect("open signed_counts");
    let signed = npy
        .i16(&dir.join("signed_counts.npy"))
        .expect("read signed_counts");
    let targets = targets();

    let run = |closed: bool| -> (u64, u64, u32, u32) {
        let mut e = Engine::new(conn.n, conn.m);
        e.load_csr(conn.row_ptr.clone(), conn.destinations.clone(), signed);
        e.set_stim(targets.clone(), RATE_HZ as f32, SEED);
        // Driven cells stand in for the motor pool, so the legs have something
        // to follow; the rest of the driven set stands in for the sense organs.
        let motors = &targets[..11];
        let prop_l = targets[11..31].to_vec();
        let prop_r = targets[31..51].to_vec();
        let touch_l = targets[51..71].to_vec();
        let touch_r = targets[71..91].to_vec();
        e.set_legs(
            prop_l,
            prop_r,
            touch_l,
            touch_r,
            motors.to_vec(),
            motors.to_vec(),
        );
        e.legs.on = closed;
        for _ in 0..STEPS {
            e.run_one();
        }
        (e.hash, e.total_spikes, e.legs.steps_l + e.legs.steps_r, e.legs.sent_prop as u32)
    };

    let (hash_a, spikes_a, steps_a, sent_a) = run(true);
    let (hash_b, spikes_b, steps_b, sent_b) = run(true);
    let (hash_open, _, _, sent_open) = run(false);

    assert_eq!(hash_a, hash_b, "the closed loop must be reproducible");
    assert_eq!(spikes_a, spikes_b);
    assert_eq!(steps_a, steps_b);
    assert_eq!(sent_a, sent_b);
    assert!(sent_a > 0, "a closed loop that sends nothing is not closed");
    assert_eq!(sent_open, 0, "an open loop must send nothing back");
    assert!(steps_a > 0, "the legs never stepped, so the read-out is dead");
    assert_ne!(hash_a, hash_open, "closing the loop changed nothing in the slice");
    println!(
        "loop closed: {} spikes, {} steps, {} afferent events, hash {:#018x}",
        spikes_a, steps_a, sent_a, hash_a
    );
}
