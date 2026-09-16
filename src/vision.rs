//! Retinotopic visual front end, with the fly's own spectral sensitivities.
//!
//! The optic lobe is a retinotopic map. `assignedOlHex1/2` in the MaleCNS
//! annotation tiles it into 892 columns per eye, every column holds one neuron of
//! each of L1-L5, Mi1/4/9, Tm1/2/20, T1 and C3, and each column's photoreceptors
//! look in a fixed direction in the body frame. Those directions, and the
//! photoreceptors belonging to each column, come from
//! `scripts/derive_retinotopy.py`; this module puts a real scene in front of them.
//!
//! One ray per column is cast into the room, the light reflected by whatever
//! surface it hits is integrated against each photoreceptor's spectral
//! sensitivity, and the connectome is left to work out that the image is moving.
//! Motion is deliberately not computed here: T4/T5 and the lobula plate have to
//! derive it from the temporal correlations this produces, which is the entire
//! point of driving the retina instead of handing the tangential cells a motion
//! estimate.
//!
//! Nothing in this module encodes behaviour. The room's texture is an arbitrary
//! but *passive* property of the world, in the same sense as the odour plume's
//! shape. The photoreceptors fire at a rate set only by the light arriving along
//! their own optical axis, in the band their own opsin absorbs.
//!
//! ## Why there is colour here at all
//!
//! Drosophila vision is chromatic. Each ommatidium holds six outer R1-R6
//! photoreceptors plus an inner R7/R8 pair, and the inner pair is what carries
//! colour: their opsins differ from Rh1 and from each other, so the inner cells
//! of one ommatidium sample different parts of the spectrum and the medulla can
//! compare them. The MaleCNS annotation resolves those classes (`type` column:
//! R1-R6, R7p, R7y, R8p, R8y, plus subtype-unresolved cells) and every one of
//! the 5895 photoreceptors this retina drives is in the pack. Before this
//! module's spectral change the retina fed all of them the SAME scalar -- one
//! luminance per column -- so the colour-sensitive cells existed in the graph
//! and were driven as if they were copies of each other, and no chromatic
//! information could reach the connectome at all. That is a defect in the sense
//! organ, not a property of the animal, and it is what `Spectral` below fixes.
//!
//! The spectral sensitivities are the measured opsins (see
//! `scripts/derive_spectral_classes.py` for the citations and the
//! subtype-unresolved rule) sampled onto a wavelength grid with the standard
//! A1 rhodopsin template. The scene is rendered as a reflectance spectrum, so
//! whether a surface looks coloured depends only on the light it reflects and
//! the pigment that absorbs it.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;

use crate::body::qrot;
use crate::groups::Drive;
use crate::room::{add, len, scale, sub, Room, V3};

/// Body-frame position of each eye's centre, relative to the thorax origin.
/// A fly is about 2.3 mm long and the head sits roughly 1.2 mm forward
/// (`Body::head`), so the eyes are just behind that and about 0.3 mm either side.
const EYE_OFF: [V3; 2] = [[1.00, 0.32, 0.12], [1.00, -0.32, 0.12]];

/// Photoreceptor firing range. Fly photoreceptors rest in the tens of Hz and
/// reach a few hundred; the range here only has to be wide enough that a moving
/// image produces a large modulation relative to the LIF threshold.
pub const BASE_HZ: f64 = 30.0;
pub const GAIN_HZ: f64 = 150.0;

// ---------------------------------------------------------------------------
// The spectrum

/// Number of wavelength samples.
pub const NBANDS: usize = 41;
/// First sample, nm.
const LAMBDA0: f32 = 300.0;
/// Sample spacing, nm. 300..=700 nm covers the fly's whole visual range (Rh3 at
/// 345 nm is the shortest pigment and Rh6 at 508 nm the longest, with the
/// metarhodopsins inside the grid).
const DLAMBDA: f32 = 10.0;

/// The illuminant: the CIE equal-energy reference stimulus E, radiance 1 at
/// every wavelength. The room has no lamp in it, so the only defensible choice
/// is an explicitly neutral one -- it leaves the spectral shaping entirely to
/// the animal's pigments and the surfaces' reflectance, which is exactly what a
/// colour-vision test wants. Every result below is stated against it.
const ILLUMINANT: f32 = 1.0;

/// A photoreceptor spectral class: the opsin the cell expresses.
///
/// The first five are the annotation's resolved classes. `R7u`/`R8u` are the
/// cells whose ommatidial subtype the annotation leaves open: they are NOT
/// given a guessed subtype, they are given the mean of the two pigments they
/// could be carrying (see `scripts/derive_spectral_classes.py`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spectral {
    /// Rh1, R1-R6, broadband blue-green.
    R16,
    /// Rh3, R7 in pale ommatidia, short UV.
    R7p,
    /// Rh4, R7 in yellow ommatidia, long UV.
    R7y,
    /// Rh5, R8 in pale ommatidia, blue.
    R8p,
    /// Rh6, R8 in yellow ommatidia, green.
    R8y,
    /// subtype-unresolved R7 (mean of Rh3 and Rh4).
    R7u,
    /// subtype-unresolved R8 (mean of Rh5 and Rh6).
    R8u,
}

pub const NCLASS: usize = 7;

/// Peak absorbance of each class's opsin, nm. Measured values; see
/// `scripts/derive_spectral_classes.py` for the primary sources
/// (Salcedo et al. 1999 J Neurosci 19:10716-10726 for Rh1/Rh3/Rh4/Rh5/Rh6,
/// cross-checked against Shakir et al. 2020 Sci Rep 10:17488).
pub const LAMBDA_MAX_NM: [f32; 5] = [478.0, 345.0, 375.0, 437.0, 508.0];

impl Spectral {
    pub fn index(self) -> usize {
        match self {
            Spectral::R16 => 0,
            Spectral::R7p => 1,
            Spectral::R7y => 2,
            Spectral::R8p => 3,
            Spectral::R8y => 4,
            Spectral::R7u => 5,
            Spectral::R8u => 6,
        }
    }

    /// The classes whose opsin this one's sensitivity is built from. The
    /// unresolved classes average two pigments; every other class is one opsin.
    fn template_lambda(self) -> &'static [f32] {
        match self {
            Spectral::R16 => &LAMBDA_MAX_NM[0..1],
            Spectral::R7p => &LAMBDA_MAX_NM[1..2],
            Spectral::R7y => &LAMBDA_MAX_NM[2..3],
            Spectral::R8p => &LAMBDA_MAX_NM[3..4],
            Spectral::R8y => &LAMBDA_MAX_NM[4..5],
            Spectral::R7u => &LAMBDA_MAX_NM[1..3],
            Spectral::R8u => &LAMBDA_MAX_NM[3..5],
        }
    }

    pub fn from_key(s: &str) -> Option<Spectral> {
        Some(match s {
            "R16" => Spectral::R16,
            "R7p" => Spectral::R7p,
            "R7y" => Spectral::R7y,
            "R8p" => Spectral::R8p,
            "R8y" => Spectral::R8y,
            "R7u" => Spectral::R7u,
            "R8u" => Spectral::R8u,
            _ => return None,
        })
    }

    pub fn key(self) -> &'static str {
        match self {
            Spectral::R16 => "R16",
            Spectral::R7p => "R7p",
            Spectral::R7y => "R7y",
            Spectral::R8p => "R8p",
            Spectral::R8y => "R8y",
            Spectral::R7u => "R7u",
            Spectral::R8u => "R8u",
        }
    }

    /// Is this class one of the inner (colour) photoreceptors?
    pub fn is_colour(self) -> bool {
        !matches!(self, Spectral::R16)
    }
}

/// The standard A1 rhodopsin alpha-band template of Govardovskii et al. (2000),
/// "In search of the visual pigment template", Vis Neurosci 17(4):509-528:
///
/// ```text
///   x    = lambda_max / lambda
///   S(x) = 1 / ( exp(A(a-x)) + exp(B(b-x)) + exp(C(c-x)) + D )
///   A = 69.7, a = 0.880
///   B = 28.0, b = 0.924
///   C = -14.9, c = 1.104
///   D = 0.674
/// ```
///
/// Only the alpha band is used. Salcedo et al. (1999) fit exactly this alpha-band
/// absorption to their measured Rh5 and Rh6 sensitivities (r = 0.995 / 0.998),
/// so the shape is the one those measurements were reported against. The
/// beta-band and the R1-R6 UV sensitising pigment are NOT modelled: the latter
/// needs a second pigment and a screening-pigment model that this scene has no
/// data for, and leaving it out makes R1-R6 *less* UV-sensitive than the real
/// cell, i.e. the colour channels are not being helped by the omission.
pub fn a1_template(lambda_max: f32, lambda: f32) -> f32 {
    let x = lambda_max / lambda;
    let t = (69.7 * (0.880 - x)).exp()
        + (28.0 * (0.924 - x)).exp()
        + (-14.9 * (1.104 - x)).exp()
        + 0.674;
    1.0 / t
}

/// Wavelength of band `k`, nm.
pub fn band_nm(k: usize) -> f32 {
    LAMBDA0 + k as f32 * DLAMBDA
}

/// Normalised spectral weights per class: `w[c][k]` sums to 1 over k, so a
/// class's quantum catch is a weighted mean of the surface's reflectance
/// spectrum and always lies in 0..=1.
///
/// Because the weights are normalised, a spectrally flat (grey) surface of
/// reflectance `t` gives the SAME catch `t` in every class. That is what makes
/// the default run's wall-only behaviour identical to the luminance-only retina
/// it replaces, and it is what makes "a coloured fruit and an equi-luminant grey
/// one" a clean comparison: the only thing that can separate them is the shape
/// of the spectrum.
pub fn class_weights() -> Box<[[f32; NBANDS]; NCLASS]> {
    let mut out = Box::new([[0.0f32; NBANDS]; NCLASS]);
    for ci in 0..NCLASS {
        let cls = class_from_index(ci);
        let lams = cls.template_lambda();
        // Average the template over however many pigments this class stands for.
        let mut raw = [0.0f64; NBANDS];
        for &lm in lams {
            for k in 0..NBANDS {
                raw[k] += a1_template(lm, band_nm(k)) as f64;
            }
        }
        for k in 0..NBANDS {
            raw[k] /= lams.len() as f64;
        }
        let sum: f64 = raw.iter().sum();
        for k in 0..NBANDS {
            out[ci][k] = (raw[k] / sum) as f32;
        }
    }
    out
}

fn class_from_index(i: usize) -> Spectral {
    match i {
        0 => Spectral::R16,
        1 => Spectral::R7p,
        2 => Spectral::R7y,
        3 => Spectral::R8p,
        4 => Spectral::R8y,
        5 => Spectral::R7u,
        _ => Spectral::R8u,
    }
}

/// Reflectance spectrum of a ripe tomato-like fruit, 400-700 nm, from the
/// measured shapes reported for red ripe tomato fruit:
///
///  * below 575 nm reflectance is under 0.1, with an absorption minimum between
///    450 and 475 nm (chlorophyll/carotenoid absorption);
///  * the reflectance rises sharply above the 560-590 nm "red edge";
///  * the maximum of a red fruit's reflectance is between 650 and 705 nm.
///
/// Sources: ElMasry & Sun, and Ciaccheri et al. (2018), as reported in
/// "Prediction of Soluble Solids and Lycopene Content of Processing Tomato
/// Cultivars by Vis-NIR Spectroscopy" (PMC9274195) -- "Reflectance value is
/// below 0.1 from 400 to 575 nm ... Above 560 nm, reflectance values rose
/// sharply because of the red coloration of ripened fruits"; and
/// "Comparison of lycopene and beta-carotene content in tomatoes" -- "The major
/// peak of reflectance for red coloured tomatoes fruits was at 652 nm ...
/// Rapid increase of reflectance ... for red, purple and brown ones at 583-587
/// nm wavelength zone".
///
/// The values are anchors on that measured shape, linearly interpolated; the
/// exact curve is not critical to anything measured here, because what matters
/// is only that the spectrum is NOT flat and that its shape matches the reported
/// one. Below 400 nm the fruit is taken to be as absorbing as it is at 450 nm.
pub const FRUIT_REFLECTANCE_ANCHORS: &[(f32, f32)] = &[
    (300.0, 0.05),
    (400.0, 0.07),
    (450.0, 0.05),
    (475.0, 0.05),
    (500.0, 0.07),
    (550.0, 0.09),
    (560.0, 0.12),
    (583.0, 0.25),
    (600.0, 0.42),
    (620.0, 0.55),
    (652.0, 0.62),
    (675.0, 0.63),
    (700.0, 0.62),
];

/// The fruit's reflectance spectrum, sampled onto the band grid.
pub fn fruit_reflectance() -> [f32; NBANDS] {
    let mut out = [0.0f32; NBANDS];
    let a = FRUIT_REFLECTANCE_ANCHORS;
    for k in 0..NBANDS {
        let lam = band_nm(k);
        let mut v = a[0].1;
        for w in a.windows(2) {
            let (l0, r0) = w[0];
            let (l1, r1) = w[1];
            if lam >= l0 && lam <= l1 {
                let t = (lam - l0) / (l1 - l0);
                v = r0 + (r1 - r0) * t;
            }
        }
        out[k] = v;
    }
    out
}

/// The spectrally flat reflectance that has the SAME quantum catch in the
/// luminance channel (R1-R6 / Rh1) as the fruit. This is the "equi-luminant
/// grey" control: whatever the fly does with the coloured fruit, it does not do
/// with this one, and the difference cannot be a luminance difference because
/// the luminance channel sees the two identically.
pub fn fruit_grey_level(w: &[[f32; NBANDS]; NCLASS]) -> f32 {
    let r = fruit_reflectance();
    let mut s = 0.0f32;
    for k in 0..NBANDS {
        s += w[Spectral::R16.index()][k] * r[k];
    }
    s.clamp(0.0, 1.0)
}

// ------------------------------------------------------------------- the hand

/// Diffuse reflectance of human skin, 300-700 nm: the measured SHAPE.
///
/// Anchors from the in-vivo diffuse-reflectance literature (Dawson et al. 1980,
/// *Br J Dermatol*; Anderson & Parrish 1981, *J Invest Dermatol*; and the
/// skin-optics summaries in Lister et al. 2012, *J Biomed Opt*): strongly
/// absorbing in the UV (the epidermis is the optical shield), rising through
/// the blue and green, and highest in the red because the dermal
/// haemoglobin/melanin absorption bands fall off there. Skin is therefore not
/// spectrally flat, and to a fly -- whose opsins all peak at or below 508 nm --
/// it is a green/blue object with a red tail outside the animal's range.
pub const HAND_REFLECTANCE_ANCHORS: &[(f32, f32)] = &[
    (300.0, 0.03),
    (350.0, 0.05),
    (400.0, 0.16),
    (450.0, 0.22),
    (500.0, 0.28),
    (550.0, 0.33),
    (600.0, 0.38),
    (650.0, 0.42),
    (700.0, 0.45),
];

/// Ambient shading factor for the palm's fly-facing surface.
///
/// This is the one number in the hand's rendering that is a modelling choice
/// rather than a measurement, and it is stated as such. The room has one
/// illuminant, the equal-energy reference E, reaching every surface; on top of
/// that, a palm coming between the fly and the room's light is lit on its
/// fly-facing side only by ambient scatter, because it occludes the direct
/// path to the ceiling for exactly the surface the fly is looking at. A hand
/// approaching a fly also typically arrives against the bright ceiling or wall
/// behind it, so the fly sees a dark expanding silhouette -- which is the
/// canonical *Drosophila* looming stimulus.
///
/// 0.18 means the palm's R1-R6 catch lands near 0.06, the same order as the
/// tomato fruit's (0.063) and about 8x darker than the room's mean wall
/// luminance -- which is what the runs show: with the palm covering half the
/// eye's field the mean luminance-channel catch per eye falls from 0.54 to
/// 0.45.
/// Nothing downstream depends on this number beyond how much light reaches the
/// photoreceptors: it is albedo, not a gain on any response.
pub const HAND_SHADING: f32 = 0.18;

/// The palm's reflectance spectrum, sampled onto the band grid.
pub fn hand_reflectance() -> [f32; NBANDS] {
    let mut out = [0.0f32; NBANDS];
    let a = HAND_REFLECTANCE_ANCHORS;
    for k in 0..NBANDS {
        let lam = band_nm(k);
        let mut v = a[0].1;
        for w in a.windows(2) {
            let (l0, r0) = w[0];
            let (l1, r1) = w[1];
            if lam >= l0 && lam <= l1 {
                let t = (lam - l0) / (l1 - l0);
                v = r0 + (r1 - r0) * t;
            }
        }
        out[k] = (v * HAND_SHADING).clamp(0.0, 1.0);
    }
    out
}

// ---------------------------------------------------------------------------
// The retina

/// Where a column's ray landed. The surface is what decides the reflectance
/// spectrum, so the renderer has to say which one was hit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    Wall = 0,
    Table = 1,
    Fruit = 2,
    /// The palm (`room::Hand`). Like the fruit it is an ordinary scene object
    /// with its own reflectance spectrum; unlike the fruit it MOVES, so the
    /// columns it lands on change from window to window and the silhouette it
    /// presents grows as it approaches.
    Hand = 3,
}

pub struct Retina {
    pub on: bool,
    /// Unit gaze direction per column, in the body frame (x forward, y left, z up).
    gaze: Vec<V3>,
    /// 0 for the left eye, 1 for the right.
    side: Vec<u8>,
    /// Distance along each column's own gaze ray to the surface it hit, mm.
    /// `f32::INFINITY` means the ray found nothing. This is the per-column
    /// retinotopic depth sample, and it is what makes a per-eye loom possible:
    /// the scalar the closed loop used to drive both `visual_loom` pools is the
    /// time-to-collision to the nearest wall along the BODY HEADING axis, which
    /// is the same number for both eyes; this is the same quantity taken over
    /// each eye's own set of gaze directions, so a wall on the left is near in
    /// the left eye's columns and far in the right eye's.
    dist: Vec<f32>,
    /// Which surface each column's ray hit, for telemetry and for the
    /// "does the optics see the fruit at all" check.
    surf: Vec<Surface>,
    /// Number of optic-lobe columns. One ray each.
    n_columns: usize,

    /// One Poisson drive per (column, spectral class) present in that column.
    /// Photoreceptors of different opsins in the same ommatidium catch
    /// different amounts of light and so must not share a drive; that sharing
    /// is exactly the defect this replaced.
    drives: Vec<Drive>,
    /// Column each drive belongs to, and the spectral class it drives.
    d_col: Vec<u32>,
    d_class: Vec<Spectral>,
    /// Class the drive's targets belong to; grouped so `col_range` is a slice.
    col_start: Vec<u32>,
    col_len: Vec<u32>,
    /// Normalised spectral weights, shared by every drive of a class.
    w: Box<[[f32; NBANDS]; NCLASS]>,
    /// The fruit's reflectance spectrum (or its equi-luminant grey level when
    /// `Room::fruit_grey` is set), precomputed.
    fruit_r: [f32; NBANDS],
    /// The palm's reflectance spectrum (`hand_reflectance`), precomputed. Used
    /// for whatever columns the moving palm's rays land on.
    hand_r: [f32; NBANDS],
    /// Luminance-channel value per column: the R1-R6 class's catch, or the mean
    /// over the column's classes when the column has no R1-R6 cell. A
    /// spectrally flat surface gives the same number here as the old scalar.
    col_lum: Vec<f32>,
    /// Per-drive catch, i.e. what that class's photoreceptors received.
    lum: Vec<f32>,
    /// Optional per-eye additive luminance (0..1 scale) applied on top of the
    /// rendered light in `update`. `None` -- the default -- leaves the scene
    /// untouched and is bit-identical. `Some((l, r))` is the open-loop probe's
    /// lateral stimulus: a common-mode luminance offset on one eye's
    /// photoreceptors, never a change in the wiring.
    imposed_lum: Option<(f32, f32)>,
    pub photons: usize,
    /// Membership mask over all model indices, so photoreceptor spikes can be
    /// counted separately from everything else the connectome is doing.
    mask: Vec<bool>,
    spikes: u64,
    steps: u64,
    /// Cells driven per class, for the banner and the colour check.
    pub class_cells: [usize; NCLASS],
}

impl Retina {
    pub fn asset_path() -> PathBuf {
        std::env::var("FLYVERSE_RETINOTOPY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("assets/male_cns_v1_retinotopy.json"))
    }

    pub fn spectral_path() -> PathBuf {
        std::env::var("FLYVERSE_SPECTRAL")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("assets/male_cns_v1_spectral_classes.json"))
    }

    /// Build the retina from the retinotopy asset and the spectral-class table,
    /// resolving each column's photoreceptors into pack indices.
    pub fn load(conn: &crate::pack::Connectome, path: &PathBuf, seed: u64) -> Result<Retina> {
        let raw = std::fs::read_to_string(path).with_context(|| {
            format!(
                "the retinotopy table {}; run scripts/derive_retinotopy.py",
                path.display()
            )
        })?;
        let v: Value = serde_json::from_str(&raw).context("parsing the retinotopy table")?;
        let cols = v["columns"].as_object().context("no `columns` in the table")?;

        // Spectral class per photoreceptor body id. A missing table is a hard
        // error for the same reason a missing sense organ is: silently falling
        // back to luminance would look exactly like a fly that cannot see
        // colour.
        let spath = Self::spectral_path();
        let sraw = std::fs::read_to_string(&spath).with_context(|| {
            format!(
                "the spectral class table {}; run scripts/derive_spectral_classes.py",
                spath.display()
            )
        })?;
        let sv: Value = serde_json::from_str(&sraw).context("parsing the spectral table")?;
        let class_map = sv["class"]
            .as_object()
            .context("no `class` map in the spectral table")?;
        let class_of = |b: u64| -> Option<Spectral> {
            class_map
                .get(&b.to_string())
                .and_then(|x| x.as_str())
                .and_then(Spectral::from_key)
        };

        let w = class_weights();
        let fruit_r = {
            let mut r = fruit_reflectance();
            if crate::room::fruit_grey() {
                let g = fruit_grey_level(&w);
                r = [g; NBANDS];
            }
            r
        };
        // The palm's spectrum is unconditional: whether a hand is in this room
        // is decided by the geometry (`Room::hand`), and a spectrum that costs
        // 41 floats is cheaper than a second code path.
        let hand_r = hand_reflectance();

        let mut gaze = Vec::new();
        let mut side = Vec::new();
        let mut drives = Vec::new();
        let mut d_col = Vec::new();
        let mut d_class = Vec::new();
        let mut col_start = Vec::new();
        let mut col_len = Vec::new();
        let mut class_cells = [0usize; NCLASS];
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
                let col = gaze.len() as u32;
                let start = drives.len() as u32;
                // Split the column's photoreceptors by spectral class and give
                // each class its own drive over its own cells.
                for cls in (0..NCLASS).map(class_from_index) {
                    let group: Vec<u64> = ids
                        .iter()
                        .copied()
                        .filter(|b| class_of(*b) == Some(cls))
                        .collect();
                    if group.is_empty() {
                        continue;
                    }
                    let idx = conn.indices_of_sorted(&group);
                    if idx.is_empty() {
                        continue;
                    }
                    photons += idx.len();
                    class_cells[cls.index()] += idx.len();
                    // Content-derived stream seed, so a drive's Poisson stream
                    // depends on WHICH cells it drives and not on the order the
                    // table happened to be walked in.
                    let tag = (col as u64)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ (cls.index() as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03);
                    d_col.push(col);
                    d_class.push(cls);
                    drives.push(Drive::new(idx, 0.0, seed ^ tag));
                }
                if drives.len() as u32 == start {
                    continue; // nothing drivable in this column
                }
                gaze.push(gv);
                side.push(si as u8);
                col_start.push(start);
                col_len.push(drives.len() as u32 - start);
            }
        }

        if drives.is_empty() {
            anyhow::bail!("the retinotopy table produced no drivable columns");
        }
        let n_columns = gaze.len();
        let n_drives = drives.len();
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
            dist: vec![f32::INFINITY; n_columns],
            surf: vec![Surface::Wall; n_columns],
            n_columns,
            drives,
            d_col,
            d_class,
            col_start,
            col_len,
            w,
            fruit_r,
            hand_r,
            col_lum: vec![0.0; n_columns],
            lum: vec![0.0; n_drives],
            imposed_lum: None,
            photons,
            mask,
            spikes: 0,
            steps: 0,
            class_cells,
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
        self.n_columns
    }

    /// Normalised spectral weights this retina is using, for the colour check.
    pub fn weights(&self) -> &[[f32; NBANDS]; NCLASS] {
        &self.w
    }

    /// The reflectance the retina gives the fruit this run (the measured
    /// spectrum, or its equi-luminant grey level under FLYVERSE_FRUIT_GREY).
    pub fn fruit_spectrum(&self) -> &[f32; NBANDS] {
        &self.fruit_r
    }

    /// The spectrally flat reflectance that has the same luminance-channel
    /// catch as the coloured fruit, i.e. the FLYVERSE_FRUIT_GREY control level.
    pub fn fruit_grey_level(&self) -> f32 {
        self.fruit_r[0]
    }

    /// Peak (R1-R6) catch the fruit produces, for the startup log.
    pub fn fruit_luminance_level(&self) -> f32 {
        let mut s = 0.0f32;
        for k in 0..NBANDS {
            s += self.w[Spectral::R16.index()][k] * self.fruit_r[k];
        }
        s
    }

    /// How many columns of each eye are looking at the fruit. Zero means the
    /// optics never see it, and any behavioural null is then uninformative
    /// rather than negative.
    pub fn fruit_columns_by_eye(&self) -> (usize, usize) {
        let mut n = [0usize; 2];
        for (i, s) in self.surf.iter().enumerate() {
            if *s == Surface::Fruit {
                n[self.side[i] as usize] += 1;
            }
        }
        (n[0], n[1])
    }

    /// How many columns of each eye are looking at the palm this window, and
    /// what fraction of that eye's columns that is. This is the silhouette the
    /// hand presents: it grows from 0 to a large fraction over the approach,
    /// and that growth IS the expanding-edge stimulus -- the retina produces it
    /// by ray-casting, not by any injected signal.
    pub fn hand_columns_by_eye(&self) -> ([usize; 2], [f32; 2]) {
        let mut n = [0usize; 2];
        let mut tot = [0usize; 2];
        for (i, s) in self.surf.iter().enumerate() {
            let e = self.side[i] as usize;
            tot[e] += 1;
            if *s == Surface::Hand {
                n[e] += 1;
            }
        }
        let f = |e: usize| {
            if tot[e] == 0 {
                0.0
            } else {
                n[e] as f32 / tot[e] as f32
            }
        };
        (n, [f(0), f(1)])
    }

    /// Nearest hand-surface distance each eye's own columns report, mm, or
    /// `INFINITY` when that eye has no column on the palm. The same quantity
    /// `loom_by_eye` reads, restricted to the palm, so the palm's own
    /// contribution to the looming signal can be told apart from the walls'.
    pub fn hand_distance_by_eye(&self) -> [f32; 2] {
        let mut best = [f32::INFINITY; 2];
        for (i, s) in self.surf.iter().enumerate() {
            if *s == Surface::Hand && self.dist[i] < best[self.side[i] as usize] {
                best[self.side[i] as usize] = self.dist[i];
            }
        }
        best
    }

    /// Mean catch of the colour (inner) photoreceptors per eye, split into the
    /// UV pair (R7 family) and the blue/green pair (R8 family). Their
    /// difference is the chromatic signal the medulla has to compare; the
    /// left-minus-right difference of either is the lateral colour drive.
    pub fn chroma_by_eye(&self) -> ([f32; 2], [f32; 2]) {
        let mut uv = [0.0f64; 2];
        let mut uv_n = [0usize; 2];
        let mut gr = [0.0f64; 2];
        let mut gr_n = [0usize; 2];
        for j in 0..self.drives.len() {
            let s = self.side[self.d_col[j] as usize] as usize;
            match self.d_class[j] {
                Spectral::R7p | Spectral::R7y | Spectral::R7u => {
                    uv[s] += self.lum[j] as f64;
                    uv_n[s] += 1;
                }
                Spectral::R8p | Spectral::R8y | Spectral::R8u => {
                    gr[s] += self.lum[j] as f64;
                    gr_n[s] += 1;
                }
                Spectral::R16 => {}
            }
        }
        let f = |v: f64, n: usize| if n == 0 { 0.0 } else { (v / n as f64) as f32 };
        ([f(uv[0], uv_n[0]), f(uv[1], uv_n[1])], [f(gr[0], gr_n[0]), f(gr[1], gr_n[1])])
    }

    /// Sample the room for every column and set each photoreceptor class's drive
    /// rate from the light that class's own pigment absorbs.
    pub fn update(&mut self, pos: V3, q: [f32; 4], room: &Room) {
        if !self.on {
            return;
        }
        for i in 0..self.n_columns {
            let eye = add(pos, qrot(q, EYE_OFF[self.side[i] as usize]));
            let dir = qrot(q, self.gaze[i]);
            // Walls and the table are spectrally flat (grey), so the one number
            // the old luminance-only retina used is still the whole story for
            // them. The fruit and the palm are the surfaces with a spectrum, so
            // they are the ones that need the per-band integral -- copied out
            // here rather than borrowed, so the drive rates below can be set.
            let mut neutral = 0.0f32;
            let mut spec = [0.0f32; NBANDS];
            match scene_hit(eye, dir, room) {
                Some((p, n, s)) => {
                    self.dist[i] = len(sub(p, eye));
                    self.surf[i] = s;
                    match s {
                        Surface::Fruit => spec = self.fruit_r,
                        Surface::Hand => spec = self.hand_r,
                        _ => neutral = luminance(p, n),
                    }
                }
                // A ray that leaves the room means the sampler is broken, not
                // that the fly sees the void; report darkness, not a crash.
                None => {
                    self.dist[i] = f32::INFINITY;
                    self.surf[i] = Surface::Wall;
                    neutral = 0.0;
                }
            }
            let spectral = matches!(self.surf[i], Surface::Fruit | Surface::Hand);
            let start = self.col_start[i] as usize;
            let end = start + self.col_len[i] as usize;
            let mut sum = 0.0f32;
            for j in start..end {
                // What this class's own pigment absorbs from that surface. A
                // spectrally flat surface gives the same number for every
                // class, which is exactly what the old luminance retina did.
                let mut catch = if spectral {
                    let wc = &self.w[self.d_class[j].index()];
                    let mut acc = 0.0f32;
                    for k in 0..NBANDS {
                        acc += wc[k] * spec[k];
                    }
                    acc
                } else {
                    neutral
                };
                if let Some((ol, or_)) = self.imposed_lum {
                    let off = if self.side[i] == 0 { ol } else { or_ };
                    catch = (catch + off).clamp(0.0, 1.0);
                }
                self.lum[j] = catch;
                sum += catch;
                self.drives[j].rate_hz = BASE_HZ + GAIN_HZ * catch as f64;
            }
            let n = (end - start).max(1) as f32;
            self.col_lum[i] = sum / n;
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
        if self.col_lum.is_empty() {
            return 0.0;
        }
        self.col_lum.iter().sum::<f32>() / self.col_lum.len() as f32
    }

    /// Mean luminance per eye, (left, right): what each eye actually received.
    pub fn mean_lum_by_eye(&self) -> (f32, f32) {
        let mut sum = [0.0f64; 2];
        let mut n = [0usize; 2];
        for (i, l) in self.col_lum.iter().enumerate() {
            let s = self.side[i] as usize;
            sum[s] += *l as f64;
            n[s] += 1;
        }
        let f = |s: usize| if n[s] == 0 { 0.0 } else { (sum[s] / n[s] as f64) as f32 };
        (f(0), f(1))
    }

    /// Per-eye looming signal: the SAME time-to-collision mapping the closed
    /// loop already uses (`sim::World::loom`, `1 - clamp(ttc/0.6, 0, 1)` with
    /// `ttc = dist / max(speed, 20 mm/s)`), but the distance is the nearest
    /// surface along THIS eye's own retinotopic columns rather than along the
    /// body heading axis.
    ///
    /// Why this exists. The scalar `World::loom` casts one ray along the body's
    /// forward axis, so it returns one number, and the closed loop delivered
    /// that same number to both `visual_loom` pools. The two eyes therefore
    /// received an identical stimulus in every window and no lateral visual
    /// information existed anywhere in the loop, regardless of what the retina
    /// was seeing. The retina already samples each eye's own field of view; this
    /// reads the same samples back per eye. Nothing is added on top of the room:
    /// the columns, their gaze directions and the raycast are the ones that
    /// already drive the photoreceptors.
    pub fn loom_by_eye(&self, speed: f32) -> (f32, f32) {
        let v = speed.max(20.0);
        let mut best = [f32::INFINITY; 2];
        for i in 0..self.dist.len() {
            let d = self.dist[i];
            let s = self.side[i] as usize;
            if d.is_finite() && d < best[s] {
                best[s] = d;
            }
        }
        let loom = |d: f32| {
            if !d.is_finite() {
                0.0
            } else {
                (1.0 - ((d / v) / 0.6).clamp(0.0, 1.0)).clamp(0.0, 1.0)
            }
        };
        (loom(best[0]), loom(best[1]))
    }

    /// Impose an additive per-eye luminance on the retina (0..1 scale added to
    /// each class's rendered catch before the photoreceptor rate is set). The
    /// open-loop lateral probe's stimulus: it changes what one eye's
    /// photoreceptors receive and nothing else. It is a common-mode luminance
    /// offset, exactly as it was before colour existed, so a probe result here
    /// is comparable with the luminance-only probes already on the record.
    pub fn set_imposed_lum(&mut self, left: f32, right: f32) {
        self.imposed_lum = Some((left.max(0.0), right.max(0.0)));
    }

    /// Restore the plain rendered light (the default state).
    pub fn clear_imposed_lum(&mut self) {
        self.imposed_lum = None;
    }

    /// Number of columns per eye in the loaded retina, for the probe header.
    pub fn columns_by_eye(&self) -> (usize, usize) {
        let l = self.side.iter().filter(|s| **s == 0).count();
        (l, self.side.len() - l)
    }

    /// Mean photoreceptor DRIVE rate per eye, Hz: the rate the columns' Poisson
    /// drives are actually being set to this window. This is the effective
    /// stimulus that reached each eye, so it is the positive control for a
    /// lateral retinal probe: if it does not move, nothing downstream can be
    /// attributed to the stimulus.
    pub fn drive_hz_by_eye(&self) -> (f32, f32) {
        let mut sum = [0.0f64; 2];
        let mut n = [0usize; 2];
        for j in 0..self.drives.len() {
            let s = self.side[self.d_col[j] as usize] as usize;
            sum[s] += self.drives[j].rate_hz;
            n[s] += 1;
        }
        let f = |s: usize| if n[s] == 0 { 0.0 } else { (sum[s] / n[s] as f64) as f32 };
        (f(0), f(1))
    }

    /// Per-eye catch of one spectral class, averaged over the drives of that
    /// class. Used by the colour check to show that a coloured and an
    /// equi-luminant grey fruit are distinguishable to this retina.
    pub fn catch_by_eye_for(&self, cls: Spectral) -> (f32, f32) {
        let mut sum = [0.0f64; 2];
        let mut n = [0usize; 2];
        for j in 0..self.drives.len() {
            if self.d_class[j] != cls {
                continue;
            }
            let s = self.side[self.d_col[j] as usize] as usize;
            sum[s] += self.lum[j] as f64;
            n[s] += 1;
        }
        let f = |s: usize| if n[s] == 0 { 0.0 } else { (sum[s] / n[s] as f64) as f32 };
        (f(0), f(1))
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

#[inline]
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Nearest positive intersection of a ray with a sphere, or `None`.
fn ray_sphere(o: V3, d: V3, c: V3, r: f32) -> Option<f32> {
    let oc = sub(o, c);
    let b = dot(oc, d);
    let cc = dot(oc, oc) - r * r;
    let disc = b * b - cc;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t = -b - s;
    if t > 1e-4 {
        Some(t)
    } else {
        let t2 = -b + s;
        if t2 > 1e-4 {
            Some(t2)
        } else {
            None
        }
    }
}

/// Nearest positive intersection of a ray with an oriented box, with the
/// outward normal of the face it entered through. Works for an origin inside
/// the box too (it then returns the exit face), which is the state a hand is
/// in at the instant of contact.
fn ray_obox(o: V3, d: V3, c: V3, half: V3, axes: [V3; 3]) -> Option<(f32, V3)> {
    let rel = sub(o, c);
    let oo = [dot(rel, axes[0]), dot(rel, axes[1]), dot(rel, axes[2])];
    let dd = [dot(d, axes[0]), dot(d, axes[1]), dot(d, axes[2])];
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    for a in 0..3 {
        if dd[a].abs() < 1e-9 {
            if oo[a] < -half[a] || oo[a] > half[a] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dd[a];
        let (mut ta, mut tb) = ((-half[a] - oo[a]) * inv, (half[a] - oo[a]) * inv);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
    }
    if t1 < 1e-4 || t0 > t1 {
        return None;
    }
    let t = if t0 > 1e-4 { t0 } else { t1 };
    // Which face: the one whose slab the hit point sits on.
    let lp = [oo[0] + dd[0] * t, oo[1] + dd[1] * t, oo[2] + dd[2] * t];
    let mut a = 0usize;
    let mut best = f32::INFINITY;
    for i in 0..3 {
        let dd2 = (lp[i].abs() - half[i]).abs();
        if dd2 < best {
            best = dd2;
            a = i;
        }
    }
    let s = if lp[a] >= 0.0 { 1.0f32 } else { -1.0 };
    Some((t, scale(axes[a], s)))
}

/// Stable per-face seed, so the six walls are not six copies of one patch.
fn face_seed(n: V3) -> u32 {
    let axis = if n[0] != 0.0 { 0u32 } else if n[1] != 0.0 { 1 } else { 2 };
    let sign = if n[axis as usize] > 0.0 { 1u32 } else { 0 };
    0x51ED_2701 ^ (axis << 3) ^ (sign << 7)
}

/// Nearest surface along a ray: the room's inner walls, the table, or the
/// fruit if one is in the room. Returns the hit point, the surface normal there
/// and which surface it was.
fn scene_hit(o: V3, d: V3, room: &Room) -> Option<(V3, V3, Surface)> {
    let lo = [room.x[0], room.y[0], room.z[0]];
    let hi = [room.x[1], room.y[1], room.z[1]];
    let mut best: Option<(f32, V3, V3, Surface)> = None;
    // The eye is normally inside the room, so the wall it sees is the exit
    // face; if it ever starts outside a box, the first surface is the entry
    // face. Taking whichever of the two is in front handles both.
    if let Some((t0, t1)) = ray_box(o, d, lo, hi) {
        let t = if t0 > 1e-4 { t0 } else { t1 };
        if t > 1e-4 {
            let p = add(o, scale(d, t));
            best = Some((t, p, box_normal(p, lo, hi), Surface::Wall));
        }
    }
    let tlo = [room.table.x[0], room.table.y[0], room.table.top - room.table.thickness];
    let thi = [room.table.x[1], room.table.y[1], room.table.top];
    if let Some((t0, t1)) = ray_box(o, d, tlo, thi) {
        let t = if t0 > 1e-4 { t0 } else { t1 };
        if t > 1e-4 && best.map_or(true, |b| t < b.0) {
            let p = add(o, scale(d, t));
            best = Some((t, p, box_normal(p, tlo, thi), Surface::Table));
        }
    }
    // The fruit: a solid sphere in the world, in front of whatever is behind it.
    // It has geometry and a reflectance spectrum and nothing else -- no
    // collision, no reward, no signal injected anywhere. The optics see it on
    // exactly the same terms as the walls.
    if let Some(f) = room.fruit {
        if let Some(t) = ray_sphere(o, d, f.c, f.r) {
            if t > 1e-4 && best.map_or(true, |b| t < b.0) {
                let p = add(o, scale(d, t));
                let n = scale(sub(p, f.c), 1.0 / f.r.max(1e-6));
                best = Some((t, p, n, Surface::Fruit));
            }
        }
    }
    // The palm: the same terms again -- a body in the world, in front of
    // whatever is behind it, whose position happens to change between windows.
    // Nothing is special-cased for it anywhere downstream; a column that lands
    // on it reports a hit, a distance and a normal like any other surface.
    if let Some(h) = room.hand {
        if let Some((t, n)) = ray_obox(o, d, h.c, h.half, h.axes) {
            if t > 1e-4 && best.map_or(true, |b| t < b.0) {
                best = Some((t, add(o, scale(d, t)), n, Surface::Hand));
            }
        }
    }
    best.map(|(_, p, n, s)| (p, n, s))
}

// ------------------------------------------------------------- the texture

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
            fruit: None,
            fruit_grey: false,
            hand: None,
            wind: [0.0, 0.0, 0.0],
            odor_amp: 0.0,
            odor_lambda: 1.0,
        }
    }

    #[test]
    fn a_ray_from_the_middle_hits_the_wall_it_points_at() {
        let r = room();
        let o = [160.0, 160.0, 100.0];
        let (hit, n, s) = scene_hit(o, [1.0, 0.0, 0.0], &r).expect("forward ray must hit");
        assert!((hit[0] - r.x[1]).abs() < 1e-2, "got {hit:?}");
        // Outward from the room box, which is the only thing the texture needs:
        // a stable identifier for which of the six faces this is.
        assert_eq!(n, [1.0, 0.0, 0.0], "the far wall's outward normal");
        assert_eq!(s, Surface::Wall);
        let (hit, _, s) = scene_hit(o, [0.0, 0.0, -1.0], &r).expect("downward ray must hit");
        assert!((hit[2] - r.table.top).abs() < 1e-2, "got {hit:?}");
        assert_eq!(s, Surface::Table);
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

    #[test]
    fn the_a1_template_peaks_at_the_opsin_wavelength() {
        // The template is normalised at x = lambda_max/lambda = 1 to within the
        // 0.5 % Govardovskii et al. report; if it did not, every class's
        // sensitivity would be shifted and the colour comparison would be
        // measuring the template rather than the pigments.
        for &lm in &LAMBDA_MAX_NM {
            let peak = a1_template(lm, lm);
            assert!(
                (peak - 1.0).abs() < 0.01,
                "lambda_max {lm} gives {peak} at its own peak"
            );
            // And it must fall away on both sides.
            assert!(a1_template(lm, lm * 0.75) < 0.9, "no blue-side falloff at {lm}");
            assert!(a1_template(lm, lm * 1.3) < 0.9, "no red-side falloff at {lm}");
        }
        // Ordering: Rh3 is the shortest pigment, Rh6 the longest.
        assert!(LAMBDA_MAX_NM[1] < LAMBDA_MAX_NM[2]);
        assert!(LAMBDA_MAX_NM[2] < LAMBDA_MAX_NM[3]);
        assert!(LAMBDA_MAX_NM[3] < LAMBDA_MAX_NM[4]);
    }

    #[test]
    fn a_flat_spectrum_gives_every_class_the_same_catch() {
        // This is the property that keeps the luminance pathway intact: for a
        // grey surface the colour change must be a no-op.
        let w = class_weights();
        for t in [0.0f32, 0.25, 0.5, 1.0] {
            for ci in 0..NCLASS {
                let s: f32 = (0..NBANDS).map(|k| w[ci][k] * t).sum();
                assert!(
                    (s - t).abs() < 1e-5,
                    "class {ci} catch {s} for a flat spectrum of {t}"
                );
            }
        }
    }

    #[test]
    fn a_coloured_fruit_differs_from_an_equi_luminant_grey_one() {
        // THE colour test. The fruit's measured spectrum and the spectrally flat
        // reflectance with the same R1-R6 (Rh1) catch must give the luminance
        // class the same response and the colour classes different ones. If they
        // did not, the retina would be blind to colour and any behavioural
        // result downstream would be a luminance result.
        let w = class_weights();
        let r = fruit_reflectance();
        let lum: f32 = (0..NBANDS).map(|k| w[Spectral::R16.index()][k] * r[k]).sum();
        let grey = fruit_grey_level(&w);
        assert!((0.0..=1.0).contains(&grey));

        let catch = |ci: usize, spec: &[f32; NBANDS]| -> f32 {
            (0..NBANDS).map(|k| w[ci][k] * spec[k]).sum()
        };
        // Luminance class: identical by construction (within float error).
        assert!(
            (catch(Spectral::R16.index(), &r) - grey).abs() < 1e-5,
            "the grey level is not equi-luminant"
        );
        // Colour classes: the coloured fruit must differ from the grey one.
        let g = [grey; NBANDS];
        // Report the whole table: absolute catch per class, and the ratio to the
        // grey one. It is the RATIO that is the colour signal -- the pigments are
        // narrow, so the absolute differences are small while the relative
        // differences are not.
        let mut ratios = Vec::new();
        for cls in [
            Spectral::R16,
            Spectral::R7p,
            Spectral::R7y,
            Spectral::R8p,
            Spectral::R8y,
        ] {
            let ci = cls.index();
            let a = catch(ci, &r);
            let b = catch(ci, &g);
            let ratio = if b > 0.0 { a / b } else { f32::NAN };
            eprintln!(
                "  {:<4} catch {:>8.5}  grey {:>8.5}  ratio {:.3}",
                cls.key(),
                a,
                b,
                ratio
            );
            ratios.push((cls, ratio));
        }
        // The luminance class is equi-luminant by construction.
        assert!(
            (catch(Spectral::R16.index(), &r) - grey).abs() < 1e-5,
            "the grey level is not equi-luminant"
        );
        // A colour signal means the chromatic classes do NOT all agree: relative
        // to grey some are brighter and some are dimmer. This is what the eye
        // must be able to see for colour to exist at all.
        let hi = ratios
            .iter()
            .filter(|(c, _)| *c != Spectral::R16)
            .map(|(_, q)| *q)
            .fold(f32::NEG_INFINITY, f32::max);
        let lo = ratios
            .iter()
            .filter(|(c, _)| *c != Spectral::R16)
            .map(|(_, q)| *q)
            .fold(f32::INFINITY, f32::min);
        eprintln!("  chromatic ratio spread: {lo:.3} .. {hi:.3}");
        // The measured signature. It is modest, and for a reason worth stating:
        // every fly opsin peaks at or below 508 nm, so the tomato's red edge --
        // its most conspicuous feature to a human -- is entirely outside this
        // retina. What is left is a real but small green/blue difference, and
        // that is the whole of the colour information available to the animal.
        assert!(
            hi > 1.2,
            "no chromatic class sees the coloured fruit brighter than grey: max ratio {hi}"
        );
        assert!(
            lo < 0.99,
            "no chromatic class sees the coloured fruit dimmer than grey: min ratio {lo}"
        );
        assert!(
            hi / lo > 1.25,
            "the chromatic channels barely disagree: spread ratio {}",
            hi / lo
        );
        // The green channel is the one that gains, which is what the spectrum
        // predicts: R8y sits at 508 nm, nearest the rising red edge.
        let r8y = ratios
            .iter()
            .find(|(c, _)| *c == Spectral::R8y)
            .map(|(_, q)| *q)
            .unwrap();
        assert!(
            r8y == hi,
            "the green (Rh6/508 nm) channel should be the one that gains, got {r8y}"
        );
        // And the red edge really is invisible: no class peaks beyond 508 nm.
        assert!(
            LAMBDA_MAX_NM.iter().cloned().fold(0.0f32, f32::max) <= 508.0,
            "a class peaks in the red, which would mean colour beyond the fly's range"
        );
        assert!(lum > 0.0);
    }

    #[test]
    fn the_fruit_matches_the_measured_ripe_tomato_shape() {
        // Reflectance low in the blue/green, sharp rise through the red edge,
        // peak in the red -- the measured shape cited on FRUIT_REFLECTANCE_ANCHORS.
        let r = fruit_reflectance();
        let at = |nm: f32| -> f32 {
            let k = ((nm - LAMBDA0) / DLAMBDA).round() as usize;
            r[k.min(NBANDS - 1)]
        };
        assert!(at(450.0) < 0.1, "450 nm should be strongly absorbed: {}", at(450.0));
        assert!(at(550.0) < 0.12, "550 nm should still be absorbed: {}", at(550.0));
        assert!(at(650.0) > 0.5, "the red band should reflect: {}", at(650.0));
        assert!(
            at(650.0) > at(550.0) * 4.0,
            "the red edge is not sharp enough: {} vs {}",
            at(650.0),
            at(550.0)
        );
        assert!(r.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn the_fruit_is_a_solid_body_the_optics_can_hit() {
        // A sphere in the room is hit from below and from above, and it occludes
        // whatever is behind it -- which is what makes it an object in the world
        // rather than a tint applied to a wall.
        let mut r = room();
        r.fruit = Some(crate::room::Fruit { c: [160.0, 160.0, 100.0], r: 20.0 });
        // Upward from below the sphere: the sphere comes first.
        let (_, _, s) = scene_hit([160.0, 160.0, 60.0], [0.0, 0.0, 1.0], &r).unwrap();
        assert_eq!(s, Surface::Fruit, "the sphere is in the way");
        // Downward from above: the sphere again, entering at its top.
        let (hp, hn, s) = scene_hit([160.0, 160.0, 140.0], [0.0, 0.0, -1.0], &r).unwrap();
        assert_eq!(s, Surface::Fruit);
        assert!((hp[2] - 120.0).abs() < 0.5, "hit at the sphere's top, got {hp:?}");
        assert!(hn[2] > 0.9, "normal points back at the viewer: {hn:?}");
        // A ray that misses it sees something else.
        let (_, _, s) = scene_hit([40.0, 40.0, 140.0], [0.0, 0.0, -1.0], &r).unwrap();
        assert_ne!(s, Surface::Fruit);
        // Below its bottom the ray passes under it to the table.
        let (_, _, s) = scene_hit([160.0, 160.0, 20.0], [0.0, 0.0, 1.0], &r).unwrap();
        assert_eq!(s, Surface::Table, "under the sphere is the table");
    }

    #[test]
    fn without_a_fruit_no_ray_reports_one() {
        // The cleared-room control must genuinely have nothing to see, or the
        // control run would still be receiving a fruit signal.
        let r = room();
        assert!(r.fruit.is_none());
        for i in 0..32 {
            for j in 0..16 {
                let a = i as f32 / 32.0 * std::f32::consts::TAU;
                let b = j as f32 / 16.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_2;
                let d = [a.cos() * b.cos(), a.sin() * b.cos(), b.sin()];
                let (_, _, s) = scene_hit([160.0, 160.0, 100.0], d, &r).unwrap();
                assert_ne!(s, Surface::Fruit);
            }
        }
    }
}