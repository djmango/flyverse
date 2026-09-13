//! Soma point cloud for the brain panel.
//!
//! Coordinates come from the official neuPrint neuron table
//! (`somaLocation:point{srid:9157}`, 8 nm voxels), converted to millimetres
//! and written in pack model-index order by `prep/prep_inputs.py`.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub const DEFAULT_SOMA_PATH: &str = "/opt/data/workspaces/skg/flybrain/flyverse/data/soma_positions.f32";

pub struct Somas {
    pub bytes: Vec<u8>,
    pub n: usize,
}

impl Somas {
    pub fn load(path: &Path, expected_n: usize) -> Result<Somas> {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        if bytes.len() % 12 != 0 {
            bail!("{} is not n*3 float32", path.display());
        }
        let n = bytes.len() / 12;
        if n != expected_n {
            bail!("{} holds {n} somas, expected {expected_n}", path.display());
        }
        Ok(Somas { bytes, n })
    }

    pub fn path() -> PathBuf {
        PathBuf::from(DEFAULT_SOMA_PATH)
    }
}
