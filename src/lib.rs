//! Library face of the FlyVerse engine.
//!
//! The binary in `src/main.rs` still declares its own modules; this target
//! exists so other crates (the WebAssembly build for the lesson, and its parity
//! test) can link the *same* source files for the LIF dynamics and the
//! connectome pack. Nothing here is a second implementation: `lif.rs` and
//! `pack.rs` are the files the native simulation runs.

pub mod lif;
pub mod npy;
pub mod pack;
