//! Measurement only: is the fly's fixed endpoint the fixed food/odour attractor?
//!
//! `run::run` calls this after a normal `analyze` run, when FLYVERSE_ATTRACTOR=1
//! is set, re-reading the trace that run already recorded. It changes nothing
//! about the simulation: it is a second reading of the same samples, plus two
//! derived quantities the JSON summary does not carry (distance-to-food over
//! time, and the fly's direction of motion against the odour gradient).
//!
//! Why the question matters: `World::food` is a fixed position (`room.food_home`,
//! src/sim.rs), the odour field is built around it (src/room.rs), and the only
//! path from `food` into the fly's motion is the two odour drives. If the fly
//! homes to a fixed source and stops, its gross motion is a hand-built surrogate
//! attractor and not the connectome exploring a room. The test is threefold:
//! where does it end up relative to `food`, does the distance to `food` fall,
//! and does its heading track the odour gradient - with FLYVERSE_NO_ODOR=1 as
//! the direct control that removes the attractor without adding behaviour.

use crate::sim::World;

use super::stats::{mean, pearson};
use super::trace::Sample;

#[inline]
fn wrap_pi(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let mut x = a % t;
    if x > std::f32::consts::PI {
        x -= t;
    } else if x < -std::f32::consts::PI {
        x += t;
    }
    x
}

fn dist3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn dist_h(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}

/// Horizontal gradient of the odour field at `p`, from the same field the
/// antennae sample (`Room::odor`). Central differences at +/-0.5 mm, which is
/// far inside the 95 mm decay length, so this is the analytic gradient to
/// within f32 rounding without re-deriving the source terms and risking drift
/// if `Room::odor` is edited. Returns (gx, gy).
fn odor_grad(w: &World, p: [f32; 3]) -> (f32, f32) {
    let h = 0.5f32;
    let gx = (w.room.odor([p[0] + h, p[1], p[2]], w.food)
        - w.room.odor([p[0] - h, p[1], p[2]], w.food))
        / (2.0 * h);
    let gy = (w.room.odor([p[0], p[1] + h, p[2]], w.food)
        - w.room.odor([p[0], p[1] - h, p[2]], w.food))
        / (2.0 * h);
    (gx, gy)
}

/// The measurement. `s` is the trace a normal run already produced; `w` is the
/// world at the end of that run, so `food` and the room bounds come from the
/// simulation rather than being typed in twice.
pub(crate) fn report(s: &[Sample], w: &World) {
    if s.len() < 3 {
        println!("\nATTRACTOR: trace too short");
        return;
    }
    let food = w.food;
    let centre = [
        0.5 * (w.room.x[0] + w.room.x[1]),
        0.5 * (w.room.y[0] + w.room.y[1]),
        0.5 * (w.room.z[0] + w.room.z[1]),
    ];
    let first = s[0];
    let last = *s.last().unwrap();
    let a = [first.x, first.y, first.z];
    let b = [last.x, last.y, last.z];
    let net = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let net_h = dist_h(a, b);
    let net_3 = dist3(a, b);

    println!("\nATTRACTOR / FOOD-HOMING MEASUREMENT (measurement only; nothing steered)");
    println!("  food, the fixed odour source   ({:.6}, {:.6}, {:.6})", food[0], food[1], food[2]);
    println!("  room centre                    ({:.6}, {:.6}, {:.6})", centre[0], centre[1], centre[2]);
    println!("  start position                 ({:.6}, {:.6}, {:.6})", a[0], a[1], a[2]);
    println!("  final position                 ({:.6}, {:.6}, {:.6})", b[0], b[1], b[2]);
    println!(
        "  net displacement vector        ({:.6}, {:.6}, {:.6})   |horizontal| {:.6} mm   |3d| {:.6} mm",
        net[0], net[1], net[2], net_h, net_3
    );
    println!(
        "  final -> food                  {:.4} mm  (horizontal {:.4} mm)",
        dist3(b, food),
        dist_h(b, food)
    );
    println!(
        "  final -> room centre           {:.4} mm  (horizontal {:.4} mm)",
        dist3(b, centre),
        dist_h(b, centre)
    );
    // The alternative hypothesis, stated so it can be rejected by the same
    // numbers: the +x/+y/ceiling corner. Distances are printed for both, so the
    // final position is compared against each candidate rather than asserted.
    let corners = [
        [w.room.x[1], w.room.y[1], w.room.z[1]],
        [w.room.x[1], w.room.y[0], w.room.z[1]],
        [w.room.x[0], w.room.y[1], w.room.z[1]],
        [w.room.x[0], w.room.y[0], w.room.z[1]],
    ];
    let (mut ci, mut cd) = (0usize, f32::INFINITY);
    for (i, c) in corners.iter().enumerate() {
        let d = dist3(b, *c);
        if d < cd {
            ci = i;
            cd = d;
        }
    }
    println!(
        "  final -> nearest ceiling corner ({:.1}, {:.1}, {:.1})  {:.4} mm  ({})",
        corners[ci][0],
        corners[ci][1],
        corners[ci][2],
        cd,
        if cd < 1.0 { "EXACTLY ON IT" } else { "not on it" }
    );
    println!(
        "  final state                    mode {} speed {:.3} mm/s touching x/y/z = {}/{}/{}",
        last.mode, last.speed, last.touch[0], last.touch[1], last.touch[2]
    );

    // Time to first contact with each room face, and how much of the run is
    // spent parked afterwards. A fixed endpoint reached in the first second and
    // held is a clamp, not a destination.
    let mut t_face = [f32::NAN; 3];
    for x in s {
        let near = [
            x.x >= w.room.x[1] - 0.5 || x.x <= w.room.x[0] + 0.5,
            x.y >= w.room.y[1] - 0.5 || x.y <= w.room.y[0] + 0.5,
            x.z >= w.room.z[1] - 0.5 || x.z <= w.room.z[0] + 0.5,
        ];
        for k in 0..3 {
            if near[k] && t_face[k].is_nan() {
                t_face[k] = x.t;
            }
        }
    }
    println!(
        "  first room-face contact        x {:.3} s, y {:.3} s, z {:.3} s (NaN = never)",
        t_face[0], t_face[1], t_face[2]
    );
    let t_first = t_face[0].min(t_face[1]);
    if t_first.is_finite() {
        let after: Vec<&Sample> = s.iter().filter(|x| x.t > t_first).collect();
        let parked = after.iter().filter(|x| x.speed < 10.0).count() as f64;
        let path_after: f32 = after
            .windows(2)
            .map(|p| {
                let dx = p[1].x - p[0].x;
                let dy = p[1].y - p[0].y;
                (dx * dx + dy * dy).sqrt()
            })
            .sum();
        println!(
            "  after first vertical-wall contact (x or y face, t={:.3} s): {:.1}% of {} samples below 10 mm/s; those samples cover {:.1} mm of horizontal path",
            t_first,
            if after.is_empty() { 0.0 } else { 100.0 * parked / after.len() as f64 },
            after.len(),
            path_after
        );
    }

    // Distance to food over the run, and whether it is a monotone approach.
    let d_food: Vec<f32> = s.iter().map(|x| dist3([x.x, x.y, x.z], food)).collect();
    let mut dec = 0usize;
    let mut inc = 0usize;
    for i in 1..d_food.len() {
        if d_food[i] < d_food[i - 1] {
            dec += 1;
        } else if d_food[i] > d_food[i - 1] {
            inc += 1;
        }
    }
    let (mut dmin, mut tmin) = (f32::INFINITY, 0.0f32);
    for (i, d) in d_food.iter().enumerate() {
        if *d < dmin {
            dmin = *d;
            tmin = s[i].t;
        }
    }
    let times: Vec<f32> = s.iter().map(|x| x.t).collect();
    let r_td = pearson(&times, &d_food);
    println!(
        "  distance to food               t=0 {:.3} mm, final {:.3} mm, minimum {:.3} mm at t={:.3} s",
        d_food[0],
        *d_food.last().unwrap(),
        dmin,
        tmin
    );
    println!(
        "  monotone approach to food?     NO unless inc=0: {} steps decrease, {} increase ({:.1}% decreasing); pearson(t, d_food) = {:.4}",
        dec,
        inc,
        100.0 * dec as f64 / (dec + inc).max(1) as f64,
        r_td
    );
    print!("  distance to food every 1 s    ");
    let mut next = 0.0f32;
    for x in s {
        if x.t >= next {
            print!(" {:.0}:{:.0}", x.t, dist3([x.x, x.y, x.z], food));
            next += 1.0;
        }
    }
    println!();

    // Does the fly's motion follow the odour gradient (or, equivalently here,
    // the direction to the food)? The gradient is evaluated at the body, whose
    // offset from the head is ~1.2 mm, negligible against the 95 mm decay
    // length. Alignment is path-weighted as well as unweighted: 11 s of a
    // parked fly jittering at 3 mm/s would otherwise swamp the one transit that
    // carries the whole path.
    let mut align_g: Vec<f32> = Vec::new();
    let mut align_f: Vec<f32> = Vec::new();
    let mut w_align_g = 0.0f64;
    let mut w_align_f = 0.0f64;
    let mut wsum = 0.0f64;
    let mut grad_mag: Vec<f32> = Vec::new();
    let mut hd_dot_g: Vec<f32> = Vec::new();
    for i in 1..s.len() {
        let p0 = &s[i - 1];
        let p1 = &s[i];
        let (dx, dy) = (p1.x - p0.x, p1.y - p0.y);
        let l = (dx * dx + dy * dy).sqrt();
        let (gx, gy) = odor_grad(w, [p1.x, p1.y, p1.z]);
        let gm = (gx * gx + gy * gy).sqrt();
        grad_mag.push(gm);
        // fly heading as a unit vector, derived from the displacement so it is
        // frame-free (the Euler yaw chart is not).
        if l > 1e-4 {
            let ux = dx / l;
            let uy = dy / l;
            // world heading from the Euler yaw (see Body::update_ground: the
            // Euler yaw is the negative of the body's rotation about z).
            let hx = (-p1.yaw).cos();
            let hy = (-p1.yaw).sin();
            if gm > 1e-6 {
                let ag = ux * gx / gm + uy * gy / gm;
                align_g.push(ag);
                w_align_g += ag as f64 * l as f64;
            }
            let (fx, fy) = (food[0] - p1.x, food[1] - p1.y);
            let fl = (fx * fx + fy * fy).sqrt();
            if fl > 1e-6 {
                let af = ux * fx / fl + uy * fy / fl;
                align_f.push(af);
                w_align_f += af as f64 * l as f64;
                wsum += l as f64;
            }
            hd_dot_g.push(hx * gx + hy * gy);
        }
    }
    let mean_mag = mean(&grad_mag);
    let (pw_g, pw_f) = if wsum > 0.0 {
        (w_align_g / wsum, w_align_f / wsum)
    } else {
        (0.0, 0.0)
    };
    println!(
        "  motion vs odour gradient       mean cos(direction of motion, up-gradient) = {:.4} (n={}), path-weighted {:.4}",
        mean(&align_g),
        align_g.len(),
        pw_g
    );
    println!(
        "  motion vs direction to food    mean cos(direction of motion, towards food) = {:.4} (n={}), path-weighted {:.4}",
        mean(&align_f),
        align_f.len(),
        pw_f
    );
    println!(
        "  odour gradient at the head     mean |grad| = {:.6} /mm (decay length {} mm); mean heading . grad = {:.6}",
        mean_mag, w.room.odor_lambda, mean(&hd_dot_g)
    );

    // Turning: does the fly turn towards the gradient / towards the stronger
    // antenna? Direction change is measured from the displacement bearing, not
    // from the Euler chart, and only between consecutive samples that both
    // moved, so park jitter does not enter as fake rotation. `cross > 0` means
    // the up-gradient direction lies to the left of the heading, and a left
    // turn is a positive change in the displacement bearing.
    let mut ddir: Vec<f32> = Vec::new();
    let mut cross_v: Vec<f32> = Vec::new();
    let mut odor_diff: Vec<f32> = Vec::new();
    let mut prev_dir: Option<f32> = None;
    for i in 1..s.len() {
        let p0 = &s[i - 1];
        let p1 = &s[i];
        let (dx, dy) = (p1.x - p0.x, p1.y - p0.y);
        if (dx * dx + dy * dy).sqrt() < 2.0 {
            prev_dir = None;
            continue;
        }
        let dir = dy.atan2(dx);
        let (gx, gy) = odor_grad(w, [p1.x, p1.y, p1.z]);
        let gm = (gx * gx + gy * gy).sqrt();
        if gm <= 1e-6 {
            prev_dir = None;
            continue;
        }
        let hx = dir.cos();
        let hy = dir.sin();
        if let Some(pd) = prev_dir {
            ddir.push(wrap_pi(dir - pd));
            cross_v.push(hx * gy / gm - hy * gx / gm);
            odor_diff.push(p1.odor_l - p1.odor_r);
        }
        prev_dir = Some(dir);
    }
    println!(
        "  turning vs odour gradient      pearson(dheading, cross(heading, up-gradient)) = {:.4} (n={}) [>0 = turns up-gradient]",
        pearson(&ddir, &cross_v),
        ddir.len()
    );
    println!(
        "  turning vs antenna odour diff  pearson(dheading, odor_left - odor_right) = {:.4} (n={}) [>0 = turns to the stronger antenna]",
        pearson(&ddir, &odor_diff),
        ddir.len()
    );
    let ol = mean(&s.iter().map(|x| x.odor_l).collect::<Vec<f32>>());
    let or = mean(&s.iter().map(|x| x.odor_r).collect::<Vec<f32>>());
    let diff = mean(&s.iter().map(|x| (x.odor_l - x.odor_r).abs()).collect::<Vec<f32>>());
    println!(
        "  odour at the antennae          mean left {:.6}, mean right {:.6}, mean |left-right| {:.6} (field max 1.0)",
        ol, or, diff
    );

    // Where the food channel would place the fly, for the record: the odour
    // drive in Hz is 120x the concentration at each antenna (sim.rs).
    println!(
        "  odour drive at t=0             left {:.3} Hz, right {:.3} Hz (120 Hz per unit concentration; vnc drive 150 Hz)",
        s[0].odor_l * 120.0,
        s[0].odor_r * 120.0
    );
}