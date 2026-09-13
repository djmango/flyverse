//! Minimal, strict NPY v1.0/v2.0 reader over an mmap.
//!
//! The upstream pack arrays are plain C-order little-endian arrays, so we only
//! accept exactly the descriptors the compiler emits and fail loudly otherwise.

use anyhow::{bail, Context, Result};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

pub struct Npy {
    _mmap: Mmap,
    pub descr: String,
    pub shape: Vec<usize>,
    offset: usize,
}

fn token_after(header: &str, key: &str) -> Option<String> {
    let start = header.find(key)? + key.len();
    let rest = &header[start..];
    let q1 = rest.find('\'')?;
    let q2 = rest[q1 + 1..].find('\'')?;
    Some(rest[q1 + 1..q1 + 1 + q2].to_string())
}

impl Npy {
    pub fn open(path: &Path) -> Result<Npy> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let mmap = unsafe { Mmap::map(&file)? };
        let b: &[u8] = &mmap;
        if b.len() < 16 || &b[0..6] != b"\x93NUMPY" {
            bail!("{}: not a NPY file", path.display());
        }
        let major = b[6];
        let (hlen, start) = match major {
            1 => (u16::from_le_bytes([b[8], b[9]]) as usize, 10usize),
            2 | 3 => (
                u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize,
                12usize,
            ),
            other => bail!("{}: unsupported NPY major version {}", path.display(), other),
        };
        if start + hlen > b.len() {
            bail!("{}: truncated NPY header", path.display());
        }
        let header = std::str::from_utf8(&b[start..start + hlen])
            .with_context(|| format!("{}: NPY header is not UTF-8", path.display()))?;
        let descr = token_after(header, "'descr':")
            .with_context(|| format!("{}: no descr in NPY header", path.display()))?;
        let shape_raw = {
            let s = header
                .find("'shape':")
                .with_context(|| format!("{}: no shape in NPY header", path.display()))?
                + "'shape':".len();
            let rest = &header[s..];
            let open = rest.find('(').context("shape has no '('")?;
            let close = rest.find(')').context("shape has no ')'")?;
            rest[open + 1..close].to_string()
        };
        let shape: Vec<usize> = shape_raw
            .split(',')
            .filter_map(|t| t.trim().parse::<usize>().ok())
            .collect();
        let offset = start + hlen;
        if offset % 8 != 0 {
            bail!("{}: NPY data offset {offset} is not 8-byte aligned", path.display());
        }
        Ok(Npy { _mmap: mmap, descr, shape, offset })
    }

    fn data(&self) -> &[u8] {
        &self._mmap[self.offset..]
    }

    fn expect(&self, descr: &str, path: &Path) -> Result<()> {
        if self.descr != descr {
            bail!(
                "{}: expected NPY descr '{descr}' but found '{}'",
                path.display(),
                self.descr
            );
        }
        Ok(())
    }

    pub fn f32(&self, path: &Path) -> Result<&[f32]> {
        self.expect("<f4", path)?;
        let n: usize = self.shape.iter().product();
        Ok(bytemuck_f32(&self.data()[..n * 4]))
    }

    pub fn i16(&self, path: &Path) -> Result<&[i16]> {
        self.expect("<i2", path)?;
        let n: usize = self.shape.iter().product();
        let raw = &self.data()[..n * 2];
        if raw.as_ptr() as usize % 2 != 0 {
            bail!("{}: i16 data is not 2-byte aligned", path.display());
        }
        Ok(unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const i16, n) })
    }

    pub fn u32(&self, path: &Path) -> Result<&[u32]> {
        self.expect("<u4", path)?;
        let n: usize = self.shape.iter().product();
        let raw = &self.data()[..n * 4];
        if raw.as_ptr() as usize % 4 != 0 {
            bail!("{}: u32 data is not 4-byte aligned", path.display());
        }
        Ok(unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const u32, n) })
    }

    pub fn u64(&self, path: &Path) -> Result<&[u64]> {
        self.expect("<u8", path)?;
        let n: usize = self.shape.iter().product();
        let raw = &self.data()[..n * 8];
        if raw.as_ptr() as usize % 8 != 0 {
            bail!("{}: u64 data is not 8-byte aligned", path.display());
        }
        Ok(unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const u64, n) })
    }
}

fn bytemuck_f32(raw: &[u8]) -> &[f32] {
    // The NPY v1.0 header pads to 64 bytes, and the mmap base is page aligned,
    // so the payload really is 4-byte aligned; this check documents the claim.
    assert!(raw.as_ptr() as usize % 4 == 0, "f32 data is not 4-byte aligned");
    let n = raw.len() / 4;
    unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const f32, n) }
}