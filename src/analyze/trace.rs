//! Trace sample type, run options, and the trace-slicing helpers.

use crate::body::Mode;
use crate::lif::DT_MS;
use crate::sim::World;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Trace

#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct Sample {
    pub t: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub speed: f32,
    pub yaw: f32,
    pub yaw_rate: f32,
    pub roll: f32,
    pub pitch: f32,
    pub wing_amp: f32,
    pub mode: u8,
    /// Smoothed motor read-outs, 0..1 normalised.
    pub pow_l: f32,
    pub pow_r: f32,
    pub steer_l: f32,
    pub steer_r: f32,
    pub walk_l: f32,
    pub walk_r: f32,
    pub land_l: f32,
    pub land_r: f32,
    pub mn9: f32,
    /// Descending read-outs, 0..1 normalised.
    pub dn02: f32,
    pub dn07: f32,
    pub sapp: f32,
    pub land_dn: f32,
    pub dn_filt: f32,
    /// Sensory drive actually presented to the connectome.
    pub odor_l: f32,
    pub odor_r: f32,
    pub flow_l: f32,
    pub flow_r: f32,
    pub loom: f32,
    pub taste: f32,
    /// Horizontal distance to the nearest wall, mm.
    pub wall_dist: f32,
    pub wall_hits: u32,
    pub takeoffs: u32,
    pub landings: u32,
    pub eats: u32,
    pub win_spikes: u32,
    pub tot_spikes: u64,
}

pub struct Options {
    pub seconds: f64,
    pub seed: u64,
    /// Keep one sample every N control windows.
    pub sample_every: u64,
    pub out: PathBuf,
}

pub(crate) fn mode_code(m: Mode) -> u8 {
    match m {
        Mode::Ground => 0,
        Mode::Takeoff => 1,
        Mode::Cruise => 2,
        Mode::Landing => 3,
        Mode::Feeding => 4,
    }
}

pub(crate) fn sample_of(w: &World) -> Sample {
    let b = &w.body;
    let m = w.motors();
    let r = &w.rates;
    let s = w.sensors();
    let p = b.pos;
    let wall_dist = (p[0] - w.room.x[0])
        .min(w.room.x[1] - p[0])
        .min(p[1] - w.room.y[0])
        .min(w.room.y[1] - p[1]);
    Sample {
        t: w.step as f32 * DT_MS / 1000.0,
        x: p[0],
        y: p[1],
        z: p[2],
        speed: b.speed(),
        yaw: b.yaw,
        yaw_rate: w.yaw_rate_rad_s(),
        roll: b.roll,
        pitch: b.pitch,
        wing_amp: b.wing_amp,
        mode: mode_code(b.mode),
        pow_l: m.flight_power_l,
        pow_r: m.flight_power_r,
        steer_l: m.flight_steer_l,
        steer_r: m.flight_steer_r,
        walk_l: m.walk_l,
        walk_r: m.walk_r,
        land_l: m.land_l,
        land_r: m.land_r,
        mn9: m.mn9,
        dn02: w.o_dn02.norm(r),
        dn07: w.o_dn07.norm(r),
        sapp: w.o_sapp.norm(r),
        land_dn: w.o_land_dn.norm(r),
        dn_filt: w.dn_filter(),
        odor_l: s.odor_l,
        odor_r: s.odor_r,
        flow_l: s.flow_l,
        flow_r: s.flow_r,
        loom: s.loom,
        taste: s.taste_hz,
        wall_dist,
        wall_hits: b.wall_hits,
        takeoffs: b.takeoffs,
        landings: b.landings,
        eats: b.eats,
        win_spikes: w.window_spikes.len() as u32,
        tot_spikes: w.lif.total_spikes,
    }
}

/// Sub-slice helper: values of `s` where `keep(sample)` holds.
pub(crate) fn sel<F: Fn(&Sample) -> bool>(s: &[Sample], f: F) -> Vec<Sample> {
    s.iter().copied().filter(|x| f(x)).collect()
}

pub(crate) fn col<F: Fn(&Sample) -> f32>(s: &[Sample], f: F) -> Vec<f32> {
    s.iter().map(f).collect()
}