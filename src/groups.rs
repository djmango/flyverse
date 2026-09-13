//! Neural-I/O groups from the pinned MaleCNS artifact.
//!
//! `assets/neuromechfly/male_cns_v1_neural_io.json` selects annotated
//! populations by cell type from Janelia's own annotation table. Every group
//! resolves to real MaleCNS body IDs, all of which are present in the pack
//! (the artifact reports zero missing roots). Nothing here is fabricated
//! wiring: these are the published motor neurons, descending neurons and
//! sensory receptor populations, used as explicit readout and stimulation
//! interfaces.

use crate::lif::DT_MS;
use crate::pack::Connectome;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Neuron-group annotations shipped with the repo (from the MaleCNS /
/// neuromechanistic-fly release). Override with `FLYVERSE_IO_JSON`.
pub const IO_JSON: &str = "assets/male_cns_v1_neural_io.json";

/// Resolve the group annotation table: `$FLYVERSE_IO_JSON`, else IO_JSON.
pub fn io_json_path() -> PathBuf {
    std::env::var("FLYVERSE_IO_JSON")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(IO_JSON))
}

/// Region buckets used by the HUD. Order is the wire order.
pub const REGION_NAMES: [&str; 12] = [
    "sensory_leg",
    "olfaction",
    "taste",
    "visual_motion",
    "visual_loom",
    "flight_descending",
    "flight_motor",
    "walking_motor",
    "landing_motor",
    "feeding_motor",
    "central",
    "other",
];

fn region_of(group: &str) -> u8 {
    match group {
        "olfaction_left" | "olfaction_right" => 1,
        "taste_sugar" => 2,
        "visual_motion_left" | "visual_motion_right" => 3,
        "visual_loom_left" | "visual_loom_right" => 4,
        "flight_dng02_left" | "flight_dng02_right" | "flight_dng07_left"
        | "flight_dng07_right" | "flight_state_sapp_left" | "flight_state_sapp_right"
        | "flight_steering_dn_left" | "flight_steering_dn_right" => 5,
        "motor_flight_power_left" | "motor_flight_power_right" | "motor_flight_steering_left"
        | "motor_flight_steering_right" => 6,
        "motor_walking_left" | "motor_walking_right" | "walking_dn_left" | "walking_dn_right" => 7,
        "motor_landing_left" | "motor_landing_right" | "landing_dn_left" | "landing_dn_right" => 8,
        "feeding_mn9" => 9,
        _ => 11,
    }
}

pub struct Groups {
    pub names: Vec<String>,
    pub members: Vec<Vec<u32>>,
    /// name -> group index
    pub by_name: HashMap<String, usize>,
    /// neuron -> group index + 1 (0 means the neuron is in no group)
    pub neuron_group: Vec<u16>,
    /// group -> region bucket
    pub group_region: Vec<u8>,
    /// group -> normalising divisor (member count, at least 1)
    pub group_size: Vec<u32>,
    /// food-odour channels: (glomerulus, attractive?, member indices)
    pub odor_channels: Vec<(String, bool, Vec<u32>)>,
}

impl Groups {
    pub fn get(&self, name: &str) -> &[u32] {
        match self.by_name.get(name) {
            Some(&i) => &self.members[i],
            None => &[],
        }
    }

    pub fn idx(&self, name: &str) -> usize {
        *self.by_name.get(name).unwrap_or(&usize::MAX)
    }

    pub fn load(c: &Connectome, path: &Path) -> Result<Groups> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let root: Value = serde_json::from_str(&text)?;

        let mut names = Vec::new();
        let mut members: Vec<Vec<u32>> = Vec::new();
        let mut by_name = HashMap::new();

        let groups = root
            .get("groups")
            .and_then(|v| v.as_object())
            .context("neural_io has no groups object")?;
        let mut keys: Vec<&String> = groups.keys().collect();
        keys.sort();
        for k in keys {
            let g = &groups[k];
            let mut roots: Vec<u64> = g
                .get("root_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
                .unwrap_or_default();
            roots.sort_unstable();
            roots.dedup();
            let idx = c.indices_of_sorted(&roots);
            by_name.insert(k.clone(), names.len());
            names.push(k.clone());
            members.push(idx);
        }

        // Food-odour channels, grouped per glomerulus and response band.
        let mut odor_channels = Vec::new();
        if let Some(ch) = root
            .get("food_olfaction")
            .and_then(|f| f.get("channels"))
            .and_then(|v| v.as_object())
        {
            let mut cks: Vec<&String> = ch.keys().collect();
            cks.sort();
            for ck in cks {
                let v = &ch[ck];
                let mut roots: Vec<u64> = v
                    .get("root_ids")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
                    .unwrap_or_default();
                roots.sort_unstable();
                roots.dedup();
                let band = v.get("response_band").and_then(|b| b.as_str()).unwrap_or("");
                let glomerulus = v
                    .get("glomerulus")
                    .and_then(|b| b.as_str())
                    .unwrap_or(ck)
                    .to_string();
                let attractive = band == "attractive";
                odor_channels.push((glomerulus, attractive, c.indices_of_sorted(&roots)));
            }
        }

        let n = c.n;
        let mut neuron_group = vec![0u16; n];
        for (gi, mem) in members.iter().enumerate() {
            for &mi in mem {
                // A neuron may appear in several groups; keep the first so the
                // per-step accounting stays a partition.
                if neuron_group[mi as usize] == 0 {
                    neuron_group[mi as usize] = (gi + 1) as u16;
                }
            }
        }
        let group_region: Vec<u8> = names.iter().map(|n| region_of(n)).collect();
        let group_size: Vec<u32> = members.iter().map(|m| m.len().max(1) as u32).collect();

        Ok(Groups { names, members, by_name, neuron_group, group_region, group_size, odor_channels })
    }

    pub fn io_path() -> PathBuf {
        io_json_path()
    }

    /// The mechanosensory / proprioceptive inventory, which is a separate file
    /// so neither it nor the main I/O set has to know about the other.
    pub fn mechano_path() -> PathBuf {
        std::env::var_os("FLYVERSE_MECHANO_JSON")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("assets/male_cns_v1_mechanosensory_io.json"))
    }

    /// Merge the groups of another neural_io-format file into this set.
    ///
    /// A group name that already exists is replaced; a new one is appended.
    /// The derived tables (`neuron_group`, `group_region`, `group_size`) are
    /// rebuilt afterwards so they stay consistent with `members`, which is
    /// what the per-neuron accounting in the LIF loop depends on.
    pub fn merge_file(&mut self, c: &Connectome, path: &Path) -> Result<usize> {
        if !path.exists() {
            anyhow::bail!("missing group file {}", path.display());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let root: Value = serde_json::from_str(&text)
            .with_context(|| format!("parse {}", path.display()))?;
        let groups = root
            .get("groups")
            .and_then(|v| v.as_object())
            .with_context(|| format!("{} has no groups object", path.display()))?;
        let mut keys: Vec<&String> = groups.keys().collect();
        keys.sort();

        let mut added = 0usize;
        for k in keys {
            let g = &groups[k];
            let mut roots: Vec<u64> = g
                .get("root_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_u64()).collect())
                .unwrap_or_default();
            roots.sort_unstable();
            roots.dedup();
            if roots.is_empty() {
                // A group that selected nothing would silently contribute a
                // dead channel, so refuse it loudly instead.
                anyhow::bail!("group {k} in {} selected no neurons", path.display());
            }
            let idx = c.indices_of_sorted(&roots);
            match self.by_name.get(k) {
                Some(&gi) => self.members[gi] = idx,
                None => {
                    self.by_name.insert(k.clone(), self.names.len());
                    self.names.push(k.clone());
                    self.members.push(idx);
                    added += 1;
                }
            }
        }
        self.rebuild(c.n);
        Ok(added)
    }

    fn rebuild(&mut self, n: usize) {
        let mut neuron_group = vec![0u16; n];
        for (gi, mem) in self.members.iter().enumerate() {
            for &mi in mem {
                if neuron_group[mi as usize] == 0 {
                    neuron_group[mi as usize] = (gi + 1) as u16;
                }
            }
        }
        self.neuron_group = neuron_group;
        self.group_region = self.names.iter().map(|n| region_of(n)).collect();
        self.group_size = self.members.iter().map(|m| m.len().max(1) as u32).collect();
    }
}

/// Poisson driver over a fixed set of target neurons, with an independent
/// counter-based stream per channel so channels do not share phase.
pub struct Drive {
    pub targets: Vec<u32>,
    pub rate_hz: f64,
    seed: u64,
    step: u64,
    buf: Vec<(u32, u32)>,
}

impl Drive {
    pub fn new(targets: Vec<u32>, rate_hz: f64, seed: u64) -> Drive {
        Drive { targets, rate_hz, seed, step: 0, buf: Vec::with_capacity(64) }
    }

    pub fn empty() -> Drive {
        Drive::new(Vec::new(), 0.0, 0)
    }

    #[inline]
    fn u01(seed: u64, step: u64, j: u64) -> f64 {
        let mut z = seed
            .wrapping_add(step.wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add(j.wrapping_mul(0xBF58_476D_1CE4_E5B9));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn events(&mut self) -> &[(u32, u32)] {
        let p = self.rate_hz * DT_MS as f64 / 1000.0;
        self.buf.clear();
        if p > 0.0 && !self.targets.is_empty() {
            for (j, &t) in self.targets.iter().enumerate() {
                if Self::u01(self.seed, self.step, j as u64) < p {
                    self.buf.push((t, 1));
                }
            }
        }
        self.step += 1;
        &self.buf
    }
}

/// Per-group spike accounting refreshed at the control rate.
pub struct GroupRates {
    pub counts: Vec<u32>,
    pub rate_hz: Vec<f32>,
    pub window_steps: u32,
    acc: Vec<u32>,
}

impl GroupRates {
    pub fn new(ngroups: usize, window_steps: u32) -> GroupRates {
        GroupRates {
            counts: vec![0; ngroups],
            rate_hz: vec![0.0; ngroups],
            window_steps,
            acc: vec![0; ngroups],
        }
    }

    #[inline]
    pub fn observe(&mut self, g: u16) {
        if g != 0 {
            self.acc[(g - 1) as usize] += 1;
        }
    }

    pub fn commit(&mut self, groups: &Groups) {
        let ms = self.window_steps as f32 * DT_MS;
        for i in 0..self.acc.len() {
            self.counts[i] = self.acc[i];
            self.rate_hz[i] = self.acc[i] as f32 / (ms / 1000.0) / groups.group_size[i] as f32;
            self.acc[i] = 0;
        }
    }

    /// 0..1 normalised activity: group mean rate mapped through a saturating
    /// curve anchored at `full` Hz.
    pub fn norm(&self, gi: usize, full_hz: f32) -> f32 {
        if gi == usize::MAX {
            return 0.0;
        }
        (self.rate_hz[gi] / full_hz).clamp(0.0, 1.0)
    }
}
