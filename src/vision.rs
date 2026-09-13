//! Retinotopic visual front end.
//!
//! The optic lobe is a retinotopic map. `assignedOlHex1/2` in the MaleCNS
//! annotation tiles it into 892 columns per eye, every column holds one neuron of
//! each of L1-L5, Mi1/4/9, Tm1/2/20, T1 and C3, and each column's photoreceptors
//! look in a fixed direction in the body frame. Those directions, and the
//! photoreceptors belonging to each column, come from
//! `scripts/derive_retinotopy.py`; this module puts a real scene in front of them.
//!
//! One ray per column is cast into the room, the luminance of whatever surface it
//! hits drives that column's photoreceptors, and the connectome is left to work
//! out that the image is moving. Motion is deliberately not computed here: T4/T5
//! and the lobula plate have to derive it from the temporal correlations this
//! produces, which is the entire point of driving the retina instead of handing
//! the tangential cells a motion estimate.
//!
//! Nothing in this module encodes behaviour. The room's texture is an arbitrary
//! but *passive* property of the world, in the same sense as the odour plume's
//! shape. The photoreceptors fire at a rate set only by the light arriving along
//! their own optical axis.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;

use crate::body::qrot;
use crate::groups::Drive;
use crate::room::{add, scale, Room, V3};

/// Body-frame position of each eye's centre, relative to the thorax origin.
/// A fly is about 2.3 mm long and the head sits roughly 1.2 mm forward
/// (`Body::head`), so the eyes are just behind that and about 0.3 mm either side.
const EYE_OFF: [V3; 2] = [[1.00, 0.32, 0.12], [1.00, -0.32, 0.12]];

/// Photoreceptor firing range. Fly photoreceptors rest in the tens of Hz and
/// reach a few hundred; the range here only has to be wide enough that a moving
/// image produces a large modulation relative to the LIF threshold.
pub const BASE_HZ: f64 = 30.0;
pub const GAIN_HZ: f64 = 150.0;

pub struct Retina {
    pub on: bool,
    /// Unit gaze direction per column, in the body frame (x forward, y left, z up).
    gaze: Vec<V3>,
    /// 0 for the left eye, 1 for the right.
    side: Vec<u8>,
    /// One Poisson drive per column, targeting that column's photoreceptors.
    drives: Vec<Drive>,
    /// Luminance of the last sample per column, for telemetry.
    pub lum: Vec<f32>,
    pub photons: usize,
    /// Membership mask over all model indices, so photoreceptor spikes can be
    /// counted separately from everything else the connectome is doing.
    mask: Vec<bool>,
    spikes: u64,
    steps: u64,
}

impl Retina {
    pub fn asset_path() -> PathBuf {
        std::env::var("FLYVERSE_RETINOTOPY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("assets/male_cns_v1_retinotopy.json"))
    }

    /// Build the retina from the retinotopy asset, resolving each column's
    /// photoreceptors into pack indices.
    pub fn load(conn: &crate::pack::Connectome, path: &PathBuf, seed: u64) -> Result<Retina> {
        let raw = std::fs::read_to_string(path).with_context(|| {
            format!(
                "the retinotopy table {}; run scripts/derive_retinotopy.py",
                path.display()
            )
        })?;
        let v: Value = serde_json::from_str(&raw).context("parsing the retinotopy table")?;
        let cols = v["columns"].as_object().context("no `columns` in the table")?;

        let mut gaze = Vec::new();
        let mut side = Vec::new();
        let mut drives = Vec::new();
        let mut photons = 0usize;

        for (si, key) in ["L", "R"].iter().enumerate() {
            let entry = cols.get(*key).context("missing a side in the table")?;
            let g = entry["gaze"].as_object().context("no `gaze` map")?;
            let pr = v["photoreceptors"][*key].as_object();
            for (tile, dir) in g {
                let d = dir.as_array().context("gaze is not an array")?;
                if d.len() != 3 {
                    anyhow::bail!("gaze for column {key}:{tile} is not a 3-vector");
                }
                let gv = [d[0].as_f64().unwrap_or(0.0) as f32,
                          d[1].as_f64().unwrap_or(0.0) as f32,
                          d[2].as_f64().unwrap_or(0.0) as f32];
                // A column with no photoreceptor in this release is kept: it
                // still has laminar neurons, but nothing to drive, so it is
                // skipped below rather than silently shifting every column index.
                let mut ids: Vec<u64> = pr
                    .and_then(|m| m.get(tile))
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|z| z.as_u64()).collect())
                    .unwrap_or_default();
                if ids.is_empty() {
                    continue;
                }
                ids.sort_unstable();
                ids.dedup();
                let idx = conn.indices_of_sorted(&ids);
                if idx.is_empty() {
                    continue;
                }
                photons += idx.len();
                let tag = (drives.len() as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                gaze.push(gv);
                side.push(si as u8);
                drives.push(Drive::new(idx, 0.0, seed ^ tag));
            }
        }

        if drives.is_empty() {
            anyhow::bail!("the retinotopy table produced no drivable columns");
        }
        let n = drives.len();
        let mut mask = vec![false; conn.n];
        for d in drives.iter() {
            for &t in d.targets.iter() {
                if let Some(m) = mask.get_mut(t as usize) {
                    *m = true;
                }
            }
        }
        Ok(Retina {
            on: std::env::var("FLYVERSE_NO_RETINA").is_err(),
            gaze,
            side,
            drives,
            lum: vec![0.0; n],
            photons,
            mask,
            spikes: 0,
            steps: 0,
        })
    }

    /// Count photoreceptor spikes in a window's fired list.
    pub fn observe(&mut self, fired: &[u32]) {
        for &i in fired {
            if self.mask.get(i as usize).copied().unwrap_or(false) {
                self.spikes += 1;
            }
        }
        self.steps += 1;
    }

    /// Mean photoreceptor firing rate in Hz over the run so far.
    pub fn photo_hz(&self, dt_ms: f64) -> f64 {
        if self.steps == 0 || self.photons == 0 {
            return 0.0;
        }
        self.spikes as f64 / self.steps as f64 / (dt_ms / 1000.0) / self.photons as f64
    }

    pub fn columns(&self) -> usize {
        self.drives.len()
    }

    /// Sample the room for every column and set its photoreceptor drive rate.
    pub fn update(&mut self, pos: V3, q: [f32; 4], room: &Room) {
        if !self.on {
            return;
        }
        for (i, d) in self.drives.iter_mut().enumerate() {
            let eye = add(pos, qrot(q, EYE_OFF[self.side[i] as usize]));
            let dir = qrot(q, self.gaze[i]);
            let lum = match scene_hit(eye, dir, room) {
                Some((p, n)) => luminance(p, n),
                // A ray that leaves the room means the sampler is broken, not
                // that the fly sees the void; report darkness, not a crash.
                None => 0.0,
            };
            self.lum[i] = lum;
            d.rate_hz = BASE_HZ + GAIN_HZ * lum as f64;
        }
    }

    /// Events for one 0.1 ms step, appended in column order.
    pub fn push_events(&mut self, out: &mut Vec<(u32, u32)>) {
        if !self.on {
            return;
        }
        for d in self.drives.iter_mut() {
            out.extend_from_slice(d.events());
        }
    }

    /// Mean luminance over the columns, for the startup log and the readout.
    pub fn mean_lum(&self) -> f32 {
        if self.lum.is_empty() {
            return 0.0;
        }
        self.lum.iter().sum::<f32>() / self.lum.len() as f32
    }
}

// --------------------------------------------------------------- the scene

/// Slab-method ray/box intersection, returning the entry and exit distances.
fn ray_box(o: V3, d: V3, lo: V3, hi: V3) -> Option<(f32, f32)> {
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;
    for a in 0..3 {
        if d[a].abs() < 1e-9 {
            if o[a] < lo[a] || o[a] > hi[a] {
                return None;
            }
        } else {
            let inv = 1.0 / d[a];
            let mut ta = (lo[a] - o[a]) * inv;
            let mut tb = (hi[a] - o[a]) * inv;
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            if ta > t0 {
                t0 = ta;
            }
            if tb < t1 {
                t1 = tb;
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((t0, t1))
}

/// Outward normal of the face of the box `[lo, hi]` that `p` lies on.
fn box_normal(p: V3, lo: V3, hi: V3) -> V3 {
    let mut best = 0usize;
    let mut bd = f32::INFINITY;
    let mut neg = false;
    for a in 0..3 {
        let dl = (p[a] - lo[a]).abs();
        if dl < bd {
            bd = dl;
            best = a;
            neg = true;
        }
        let dh = (p[a] - hi[a]).abs();
        if dh < bd {
            bd = dh;
            best = a;
            neg = false;
        }
    }
    let mut n = [0.0f32; 3];
    n[best] = if neg { -1.0 } else { 1.0 };
    n
}

/// Stable per-face seed, so the six walls are not six copies of one patch.
fn face_seed(n: V3) -> u32 {
    let axis = if n[0] != 0.0 { 0u32 } else if n[1] != 0.0 { 1 } else { 2 };
    let sign = if n[axis as usize] > 0.0 { 1u32 } else { 0 };
    0x51ED_2701 ^ (axis << 3) ^ (sign << 7)
}

/// Nearest surface along a ray: the room's inner walls, or the table.
/// Returns the hit point and the surface normal there.
fn scene_hit(o: V3, d: V3, room: &Room) -> Option<(V3, V3)> {
    let lo = [room.x[0], room.y[0], room.z[0]];
    let hi = [room.x[1], room.y[1], room.z[1]];
    let mut best: Option<(f32, V3, V3)> = None;
    // The eye is normally inside the room, so the wall it sees is the exit
    // face; if it ever starts outside a box, the first surface is the entry
    // face. Taking whichever of the two is in front handles both.
    if let Some((t0, t1)) = ray_box(o, d, lo, hi) {
        let t = if t0 > 1e-4 { t0 } else { t1 };
        if t > 1e-4 {
            let p = add(o, scale(d, t));
            best = Some((t, p, box_normal(p, lo, hi)));
        }
    }
    let tlo = [room.table.x[0], room.table.y[0], room.table.top - room.table.thickness];
    let thi = [room.table.x[1], room.table.y[1], room.table.top];
    if let Some((t0, t1)) = ray_box(o, d, tlo, thi) {
        let t = if t0 > 1e-4 { t0 } else { t1 };
        if t > 1e-4 && best.map_or(true, |b| t < b.0) {
            let p = add(o, scale(d, t));
            best = Some((t, p, box_normal(p, tlo, thi)));
        }
    }
    best.map(|(_, p, n)| (p, n))
}

// ------------------------------------------------------------- the texture

fn h3(a: i32, b: i32, c: i32, seed: u32) -> f32 {
    let mut h = (a as u32)
        .wrapping_mul(0x8DA6_B343)
        ^ (b as u32).wrapping_mul(0xD816_3841)
        ^ (c as u32).wrapping_mul(0xCB1A_B31F)
        ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Trilinear value noise on a unit lattice.
fn vnoise(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (ux, uy, uz) = (s(x - fx), s(y - fy), s(z - fz));
    let mut acc = 0.0;
    for dz in 0..2 {
        for dy in 0..2 {
            for dx in 0..2 {
                let w = (if dx == 1 { ux } else { 1.0 - ux })
                    * (if dy == 1 { uy } else { 1.0 - uy })
                    * (if dz == 1 { uz } else { 1.0 - uz });
                acc += w * h3(ix + dx, iy + dy, iz + dz, seed);
            }
        }
    }
    acc.clamp(0.0, 1.0)
}

/// Luminance of the room's surfaces at a world point.
///
/// Two octaves: 3 mm structure, which is about what an ommatidium resolves at the
/// distances involved here (Drosophila inter-ommatidial angle is near 4.5
/// degrees), plus a finer one so the image is not a single spatial frequency.
/// The field is fixed in the world, so it slides across the retina as the fly
/// moves, and the only thing that changes over time at a given photoreceptor is
/// where the fly is looking.
pub fn luminance(p: V3, n: V3) -> f32 {
    let seed = face_seed(n);
    let a = vnoise(p[0] / 3.0, p[1] / 3.0, p[2] / 3.0, seed);
    let b = vnoise(p[0] / 1.1, p[1] / 1.1, p[2] / 1.1, seed ^ 0x9E37_79B9);
    (0.65 * a + 0.35 * b).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::Table;

    fn room() -> Room {
        Room {
            x: [40.0, 280.0],
            y: [40.0, 280.0],
            z: [0.0, 200.0],
            table: Table {
                x: [-20.0, 340.0],
                y: [-20.0, 340.0],
                top: 40.0,
                thickness: 6.0,
            },
            wind: [0.0, 0.0, 0.0],
            odor_amp: 0.0,
            odor_lambda: 1.0,
        }
    }

    #[test]
    fn a_ray_from_the_middle_hits_the_wall_it_points_at() {
        let r = room();
        let o = [160.0, 160.0, 100.0];
        let (hit, n) = scene_hit(o, [1.0, 0.0, 0.0], &r).expect("forward ray must hit");
        assert!((hit[0] - r.x[1]).abs() < 1e-2, "got {hit:?}");
        // Outward from the room box, which is the only thing the texture needs:
        // a stable identifier for which of the six faces this is.
        assert_eq!(n, [1.0, 0.0, 0.0], "the far wall's outward normal");
        let (hit, _) = scene_hit(o, [0.0, 0.0, -1.0], &r).expect("downward ray must hit");
        assert!((hit[2] - r.table.top).abs() < 1e-2, "got {hit:?}");
    }

    #[test]
    fn a_ray_never_leaves_the_room_it_starts_inside() {
        let r = room();
        let o = [160.0, 160.0, 100.0];
        let mut n = 0;
        for i in 0..24 {
            for j in 0..24 {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let b = j as f32 / 24.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_2;
                let d = [a.cos() * b.cos(), a.sin() * b.cos(), b.sin()];
                assert!(scene_hit(o, d, &r).is_some(), "missed at {i},{j}");
                n += 1;
            }
        }
        assert_eq!(n, 576);
    }

    #[test]
    fn luminance_is_bounded_and_varies_over_the_room() {
        let n = [-1.0, 0.0, 0.0];
        let a = luminance([100.0, 160.0, 60.0], n);
        let b = luminance([260.0, 44.0, 180.0], n);
        assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
        assert!((a - b).abs() > 1e-3, "the texture is flat: {a} vs {b}");
    }

    #[test]
    fn the_texture_is_a_function_of_position_only() {
        // Same world point, same face: the luminance must not depend on how the
        // fly reached it, or the retina would be seeing its own motion.
        let p = [173.25, 91.5, 40.0];
        let n = [-1.0, 0.0, 0.0];
        assert_eq!(luminance(p, n), luminance(p, n));
        // Different faces at the same coordinates must differ, or a corner
        // would be invisible to the eye.
        assert_ne!(face_seed([-1.0, 0.0, 0.0]), face_seed([1.0, 0.0, 0.0]));
        assert_ne!(face_seed([-1.0, 0.0, 0.0]), face_seed([0.0, 0.0, 1.0]));
    }

    #[test]
    fn the_eye_has_something_to_look_at_from_where_the_fly_starts() {
        // Ties the sampler to the shipped room and the shipped start position:
        // the fly spawns at (-220, -150, 0), on the floor outside the table's
        // footprint, and every direction must still find a surface. A ray that
        // escapes would drive its column's photoreceptors at darkness, and a
        // pattern of darkness looks like a working eye that sees nothing.
        let r = crate::room::ROOM;
        let start = [-220.0f32, -150.0, 0.0];
        let mut seen = 0;
        for si in 0..2 {
            let o = add(start, EYE_OFF[si]);
            for i in 0..32 {
                for j in 0..16 {
                    let a = i as f32 / 32.0 * std::f32::consts::TAU;
                    let b = j as f32 / 16.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_2;
                    let d = [a.cos() * b.cos(), a.sin() * b.cos(), b.sin()];
                    assert!(scene_hit(o, d, &r).is_some(), "escaped at eye {si}, dir {i},{j}");
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, 1024);
    }

    #[test]
    fn a_box_normal_points_away_from_the_box() {
        let lo = [0.0, 0.0, 0.0];
        let hi = [10.0, 10.0, 10.0];
        assert_eq!(box_normal([0.0, 5.0, 5.0], lo, hi), [-1.0, 0.0, 0.0]);
        assert_eq!(box_normal([10.0, 5.0, 5.0], lo, hi), [1.0, 0.0, 0.0]);
        assert_eq!(box_normal([5.0, 5.0, 10.0], lo, hi), [0.0, 0.0, 1.0]);
    }
}
