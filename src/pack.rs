//! The MaleCNS v1.0 runtime pack: a signed, source-major CSR connectome.
//!
//! Arrays (compiled by our own Python compiler from the official Janelia
//! `gs://flyem-male-cns/v1.0` feather tables):
//!   neuron_ids.npy     u64[n]        body IDs in model-index order
//!   row_ptr.npy        u32[n + 1]    source-major CSR offsets
//!   destinations.npy   u32[m]        postsynaptic model indices
//!   signed_counts.npy  i16[m]        signed anatomical contact counts
//!
//! The sign is the presynaptic cell's consensus transmitter: acetylcholine
//! positive, GABA/glutamate negative.

use crate::npy::Npy;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const WEIGHT_MV: f32 = 0.275;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub dataset: Option<String>,
    pub dataset_version: Option<String>,
    pub license: Option<String>,
    #[serde(default)]
    pub counts: serde_json::Value,
    #[serde(default)]
    pub array_sha256: serde_json::Value,
}

pub struct Connectome {
    pub ids: &'static [u64],
    pub row_ptr: &'static [u32],
    pub destinations: &'static [u32],
    pub srcs: Vec<u32>,
    pub edge_write_mv: Vec<f32>,
    pub n: usize,
    pub m: usize,
    _keep: Vec<Npy>,
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

impl Connectome {
    pub fn load(pack: &Path) -> Result<Connectome> {
        for f in [
            "neuron_ids.npy",
            "row_ptr.npy",
            "destinations.npy",
            "signed_counts.npy",
        ] {
            if !pack.join(f).exists() {
                bail!("pack {} is missing {f}", pack.display());
            }
        }
        let ids_np = Npy::open(&pack.join("neuron_ids.npy"))?;
        let rp_np = Npy::open(&pack.join("row_ptr.npy"))?;
        let dst_np = Npy::open(&pack.join("destinations.npy"))?;
        let sc_np = Npy::open(&pack.join("signed_counts.npy"))?;

        let ids = leak(ids_np.u64(&pack.join("neuron_ids.npy"))?.to_vec());
        let row_ptr = leak(rp_np.u32(&pack.join("row_ptr.npy"))?.to_vec());
        let destinations = leak(dst_np.u32(&pack.join("destinations.npy"))?.to_vec());
        let signed = sc_np.i16(&pack.join("signed_counts.npy"))?;

        let n = ids.len();
        let m = destinations.len();
        if row_ptr.len() != n + 1 {
            bail!("row_ptr has {} entries, expected {}", row_ptr.len(), n + 1);
        }
        if signed.len() != m {
            bail!("signed_counts has {} entries, expected {m}", signed.len());
        }
        if row_ptr[n] as usize != m {
            bail!("row_ptr terminal {} does not equal edge count {m}", row_ptr[n]);
        }
        // strict CSR validation: monotone offsets, in-range destinations
        let mut prev = 0u32;
        for (i, &v) in row_ptr.iter().enumerate() {
            if v < prev {
                bail!("row_ptr is not monotone at index {i}");
            }
            if v as usize > m {
                bail!("row_ptr[{i}] = {v} exceeds edge count {m}");
            }
            prev = v;
        }
        for (e, &d) in destinations.iter().enumerate() {
            if d as usize >= n {
                bail!("destinations[{e}] = {d} is out of range for {n} neurons");
            }
        }

        // edge_write_mv must match the Python reference bit for bit:
        // signed_counts.astype(float32) * float32(0.275)
        let edge_write_mv: Vec<f32> = signed
            .iter()
            .map(|&c| (c as f32) * WEIGHT_MV)
            .collect();

        let mut srcs = vec![0u32; m];
        for s in 0..n {
            let (a, b) = (row_ptr[s] as usize, row_ptr[s + 1] as usize);
            for e in a..b {
                srcs[e] = s as u32;
            }
        }

        Ok(Connectome {
            ids,
            row_ptr,
            destinations,
            srcs,
            edge_write_mv,
            n,
            m,
            _keep: vec![ids_np, rp_np, dst_np, sc_np],
        })
    }

    pub fn manifest(pack: &Path) -> Result<Manifest> {
        let text = std::fs::read_to_string(pack.join("manifest.json"))
            .with_context(|| format!("read {}/manifest.json", pack.display()))?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Map body IDs to model indices. Input must be sorted ascending.
    pub fn indices_of_sorted(&self, body_ids: &[u64]) -> Vec<u32> {
        let mut out = Vec::with_capacity(body_ids.len());
        for &id in body_ids {
            if let Ok(p) = self.ids.binary_search(&id) {
                out.push(p as u32);
            }
        }
        out
    }

    pub fn census(&self) -> Census {
        let mut exc = 0usize;
        let mut inh = 0usize;
        let mut contacts: i64 = 0;
        for i in 0..self.m {
            let c = (self.edge_write_mv[i] / WEIGHT_MV).round() as i64;
            if c > 0 {
                exc += 1;
            } else if c < 0 {
                inh += 1;
            }
            contacts += c.abs();
        }
        Census { neurons: self.n, edges: self.m, excitatory_edges: exc, inhibitory_edges: inh, contact_sum: contacts }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Census {
    pub neurons: usize,
    pub edges: usize,
    pub excitatory_edges: usize,
    pub inhibitory_edges: usize,
    pub contact_sum: i64,
}

/// Default connectome pack, relative to the working directory (a repo-root
/// `official-pack` symlink points at it). Override with `FLYVERSE_PACK`.
pub fn pack_dir() -> PathBuf {
    std::env::var("FLYVERSE_PACK")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("official-pack"))
}
