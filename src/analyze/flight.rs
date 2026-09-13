//! Open-air free-flight test.

use crate::body::Mode;
use crate::sim::{World, WINDOW_S};
use anyhow::Result;
use std::path::Path;

use super::stats::mean64;
use super::trace::Options;

/// Open-air free-flight test: does the airframe hold altitude, sink, or fall?
///
/// The body is placed at `altitude` mm with no surface beneath it and left
/// entirely to the connectome and the aerodynamics. The decisive number is the
/// mean lift-to-weight ratio: if it is not above 1 the fly cannot sustain
/// altitude in mid-air, and no amount of brain work will change that.
pub fn flight_test(pack: &Path, o: &Options, altitude: f32) -> Result<()> {
    let mut w = World::new(pack, o.seed, None)?;
    w.body.pos = [0.0, 0.0, altitude];
    w.body.vel = [0.0, 0.0, 0.0];
    // Spawn AIRBORNE. Spawning in Mode::Ground lets update_ground clamp the
    // body to the support surface on the very first window, which erases the
    // requested altitude and makes the whole test meaningless.
    w.body.mode = Mode::Cruise;
    let windows = ((o.seconds / WINDOW_S as f64) as u64).max(1);
    println!(
        "flight-test: start z {:.0} mm, no surface beneath, {:.0}s, seed {}",
        altitude, o.seconds, o.seed
    );
    let mut ratio: Vec<f64> = Vec::new();
    let mut ratio_w: Vec<f64> = Vec::new();
    let mut tilts: Vec<f64> = Vec::new();
    let mut vz: Vec<f64> = Vec::new();
    for i in 0..windows {
        w.advance();
        ratio.push((w.body.fz_body / w.body.weight()) as f64);
        ratio_w.push((w.body.fz_world / w.body.weight()) as f64);
        vz.push(w.body.vel[2] as f64);
        tilts.push(w.body.tilt_deg() as f64);
        if i % (windows / 12).max(1) == 0 {
            println!(
                "  t {:>5.2}s  z {:>7.1} mm  vz {:>+8.1}  tilt {:>6.1}deg  body-up {:.2}  world-up {:.2}  {}",
                (i + 1) as f32 * WINDOW_S,
                w.body.pos[2],
                w.body.vel[2],
                w.body.tilt_deg(),
                w.body.fz_body / w.body.weight(),
                w.body.fz_world / w.body.weight(),
                w.body.mode.as_str()
            );
            println!(
                "              tau_aero {:>+9.1} {:>+9.1} {:>+9.1} | tau_damp {:>+9.1} {:>+9.1} {:>+9.1} | omega {:>+7.3} {:>+7.3} {:>+7.3}",
                w.body.tau_aero[0],
                w.body.tau_aero[1],
                w.body.tau_aero[2],
                w.body.tau_damp[0],
                w.body.tau_damp[1],
                w.body.tau_damp[2],
                w.body.omega[0],
                w.body.omega[1],
                w.body.omega[2]
            );
        }
    }
    let mut s = ratio.clone();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let (lo, hi) = (*s.first().unwrap(), *s.last().unwrap());
    let above = ratio.iter().filter(|r| **r > 1.0).count() as f64 / ratio.len() as f64;
    let above_w = ratio_w.iter().filter(|r| **r > 1.0).count() as f64 / ratio_w.len() as f64;
    println!("\n=== FREE FLIGHT, NO SURFACE ===");
    println!(
        "  body-up force   mean {:.3}, median {:.3}, min {:.3}, max {:.3}",
        mean64(&ratio),
        s[s.len() / 2],
        lo,
        hi
    );
    let mut sw = ratio_w.clone();
    sw.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "  world-up force  mean {:.3}, median {:.3}, min {:.3}, max {:.3}",
        mean64(&ratio_w),
        sw[sw.len() / 2],
        *sw.first().unwrap(),
        *sw.last().unwrap()
    );
    println!(
        "  above weight    body-frame {:.1}%, world-frame {:.1}% of windows",
        100.0 * above,
        100.0 * above_w
    );
    let mut st = tilts.clone();
    st.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "  attitude        mean {:.1} deg, median {:.1}, p90 {:.1}, max {:.1}   (0 = level)",
        mean64(&tilts),
        st[st.len() / 2],
        st[(st.len() * 9) / 10],
        *st.last().unwrap()
    );
    println!(
        "  vertical      mean vz {:+.1} mm/s, z {:.1} -> {:.1} mm (net {:+.1})",
        mean64(&vz),
        altitude,
        w.body.pos[2],
        w.body.pos[2] - altitude
    );
    Ok(())
}