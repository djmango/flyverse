//! The JSON summary. Every key here is consumed by scripts/behavior.sh and the
//! report template, so the names and values must not change.

use crate::lif::DT_MS;
use crate::sim::{World, WINDOW_S};

use super::stats::{mean, pct, pearson};
use super::trace::{col, sel, Options, Sample};

// ---------------------------------------------------------------------------
// Summary

pub(crate) fn summarize(
    s: &[Sample],
    w: &World,
    sim_seconds: f64,
    wall_seconds: f64,
    o: &Options,
) -> serde_json::Value {
    let n = s.len().max(1);
    let air = sel(s, |x| x.mode == 1 || x.mode == 2 || x.mode == 3);
    let cruise = sel(s, |x| x.mode == 2);
    let minutes = (sim_seconds / 60.0).max(1e-9);

    // Mode occupancy.
    let mut mode_time = [0f64; 5];
    for x in s {
        mode_time[x.mode as usize] += 1.0;
    }
    let mode_pct = |i: usize| mode_time[i] / n as f64 * 100.0;

    // Path length and displacement (mm).
    let mut path = 0f64;
    let mut path_h = 0f64;
    for i in 1..s.len() {
        let dx = (s[i].x - s[i - 1].x) as f64;
        let dy = (s[i].y - s[i - 1].y) as f64;
        let dz = (s[i].z - s[i - 1].z) as f64;
        path += (dx * dx + dy * dy + dz * dz).sqrt();
        path_h += (dx * dx + dy * dy).sqrt();
    }
    let first = s.first().copied().unwrap_or_default();
    let last = s.last().copied().unwrap_or_default();
    let net_h = (((last.x - first.x) as f64).powi(2) + ((last.y - first.y) as f64).powi(2)).sqrt();

    // Wall behaviour.
    let near_wall = s.iter().filter(|x| x.wall_dist < 20.0).count();
    let hits = last.wall_hits - first.wall_hits;
    let near_wall_air = air.iter().filter(|x| x.wall_dist < 20.0).count();

    // Turning.
    let air_yaw_rate = col(&air, |x| x.yaw_rate.abs());
    let total_turn: f64 = if air.len() > 1 {
        let mut t = 0f64;
        for i in 1..air.len() {
            let mut d = air[i].yaw - air[i - 1].yaw;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            t += d.abs() as f64;
        }
        t
    } else {
        0.0
    };

    // The key test: does a looming wall move the steering motor neurons?
    let steer_pair: Vec<(f32, f32)> = air.iter().map(|x| (x.loom, x.steer_r - x.steer_l)).collect();
    let loom_v = steer_pair.iter().map(|p| p.0).collect::<Vec<f32>>();
    let steer_v = steer_pair.iter().map(|p| p.1).collect::<Vec<f32>>();
    let loom_steer_r = pearson(&loom_v, &steer_v);
    let yaw_v = col(&air, |x| x.yaw_rate);
    let loom_yaw_r = pearson(&loom_v, &yaw_v);

    let imminent = sel(&air, |x| x.loom > 0.5);
    let calm = sel(&air, |x| x.loom < 0.2);
    let looming_yaw = mean(&col(&imminent, |x| x.yaw_rate.abs()));
    let calm_yaw = mean(&col(&calm, |x| x.yaw_rate.abs()));
    let looming_steer = mean(&col(&imminent, |x| (x.steer_r - x.steer_l).abs()));
    let calm_steer = mean(&col(&calm, |x| (x.steer_r - x.steer_l).abs()));

    // Occupancy grids over the floor plan.
    const NX: usize = 12;
    const NY: usize = 9;
    let grid = |set: &Vec<Sample>| -> Vec<Vec<f64>> {
        let mut g = vec![vec![0f64; NX]; NY];
        for x in set {
            let cx = (((x.x + 300.0) / 600.0) * NX as f32).floor().clamp(0.0, NX as f32 - 1.0) as usize;
            let cy = (((x.y + 220.0) / 440.0) * NY as f32).floor().clamp(0.0, NY as f32 - 1.0) as usize;
            g[cy][cx] += 1.0;
        }
        let tot: f64 = g.iter().flat_map(|r| r.iter()).sum::<f64>().max(1.0);
        for r in g.iter_mut() {
            for v in r.iter_mut() {
                *v /= tot;
            }
        }
        g
    };

    // Histograms.
    let mut alt_hist = vec![0f64; 22];
    for x in &air {
        let b = (x.z / 10.0).floor().clamp(0.0, 21.0) as usize;
        alt_hist[b] += 1.0;
    }
    let at = air.len().max(1) as f64;
    for v in alt_hist.iter_mut() {
        *v /= at;
    }
    let mut speed_hist = vec![0f64; 16];
    for x in &air {
        let b = (x.speed / 25.0).floor().clamp(0.0, 15.0) as usize;
        speed_hist[b] += 1.0;
    }
    for v in speed_hist.iter_mut() {
        *v /= at;
    }

    let alt_v = col(&air, |x| x.z);
    let spd_v = col(&air, |x| x.speed);

    // Yaw attribution.
    //
    // The pitch investigation found a *constant* pitching moment that the body
    // had no way to oppose. Yaw has no gravity term, so a constant yaw torque
    // legitimately produces constant rotation: the useful question is not
    // "does it spin" but "is the torque persistently one-signed", and if so,
    // whether it is a body-model bias or the connectome's own motor output.
    // Both candidates are measured here so neither has to be assumed.
    let tau_yaw = col(&air, |x| x.tau_aero[2]);
    let pos_y = tau_yaw.iter().filter(|v| **v > 0.0).count() as f64 / at;
    let neg_y = tau_yaw.iter().filter(|v| **v < 0.0).count() as f64 / at;
    // The same torque split into what the tilt differential commands and what
    // the amplitude differential commands (see `trace::yaw_torque_split`).
    let tilt_term = col(&air, |x| x.tau_yaw_tilt);
    let amp_term = col(&air, |x| x.tau_yaw_amp);

    // Clamp occupancy: a rate pinned at MAX_OMEGA is the numerical guard, not a
    // measurement, so it is reported separately from the rate itself.
    const CLAMP_EPS: f32 = 0.999;
    let clamp_pct = |a: usize| -> f64 {
        air.iter()
            .filter(|x| {
                let w = [x.wroll, x.wpitch, x.wyaw][a];
                w.abs() >= CLAMP_EPS * crate::body::MAX_OMEGA
            })
            .count() as f64
            / at
    };
    let yaw_clamp = air
        .iter()
        .filter(|x| x.yaw_rate.abs() >= CLAMP_EPS * crate::body::MAX_OMEGA)
        .count() as f64
        / at;
    let parked = air.iter().filter(|x| x.speed < 10.0).count() as f64 / at;
    let cornered = air
        .iter()
        .filter(|x| x.touch[0] && x.touch[1])
        .count() as f64
        / at;
    let steer_readout_sat = air
        .iter()
        .filter(|x| x.steer_l >= 0.999 || x.steer_r >= 0.999)
        .count() as f64
        / at;
    let steer_hz_l = mean(&col(&air, |x| x.steer_l_hz));
    let steer_hz_r = mean(&col(&air, |x| x.steer_r_hz));

    serde_json::json!({
        "config": {
            "seed": o.seed,
            "seconds": o.seconds,
            "sample_every_windows": o.sample_every,
            "window_ms": WINDOW_S * 1000.0,
            "dt_ms": DT_MS,
            "samples": s.len(),
            "pack": "official-pack",
        },
        "run": {
            "sim_seconds": sim_seconds,
            "wall_seconds": wall_seconds,
            "steps": w.step,
            "steps_per_second": w.step as f64 / wall_seconds,
            "realtime_factor": sim_seconds / wall_seconds,
        },
        "neural": {
            "total_spikes": w.lif.total_spikes,
            "spikes_per_sim_second": w.lif.total_spikes as f64 / sim_seconds.max(1e-9),
            "active_neurons": w.lif.active_neurons,
            "mean_rate_hz": w.lif.total_spikes as f64 / w.conn.n as f64 / sim_seconds.max(1e-9),
            "neurons": w.conn.n,
            "edges": w.conn.m,
            "stimulus_targets": w.vnc_targets,
        },
        "vision": {
            "retina_on": w.retina.on,
            "columns": w.retina.columns(),
            "photoreceptors": w.retina.photons,
            "photoreceptor_hz": w.retina.photo_hz(DT_MS as f64),
            "mean_luminance": w.retina.mean_lum(),
            "optic_flow_proxy": w.flow_on && !w.retina.on,
        },
        "mode_percent": {
            "GROUND": mode_pct(0),
            "TAKEOFF": mode_pct(1),
            "CRUISE": mode_pct(2),
            "LANDING": mode_pct(3),
            "FEEDING": mode_pct(4),
        },
        "events": {
            "takeoffs": last.takeoffs - first.takeoffs,
            "landings": last.landings - first.landings,
            "wall_hits": hits,
            "eats": last.eats - first.eats,
            "wall_hits_per_min": hits as f64 / minutes,
            "takeoffs_per_min": (last.takeoffs - first.takeoffs) as f64 / minutes,
            "mean_seconds_between_wall_hits":
                if hits > 0 { sim_seconds / hits as f64 } else { f64::INFINITY },
        },
        "movement": {
            "path_total_mm": path,
            "path_horizontal_mm": path_h,
            "net_horizontal_displacement_mm": net_h,
            "tortuosity": if net_h > 1e-6 { path_h / net_h } else { f64::INFINITY },
            "mean_speed_mm_s": mean(&spd_v),
            "p50_speed_mm_s": pct(&spd_v, 0.5),
            "p95_speed_mm_s": pct(&spd_v, 0.95),
            "max_speed_mm_s": spd_v.iter().cloned().fold(0.0f32, f32::max),
        },
        "altitude_mm": {
            "mean": mean(&alt_v),
            "p05": pct(&alt_v, 0.05),
            "p50": pct(&alt_v, 0.5),
            "p95": pct(&alt_v, 0.95),
            "max": alt_v.iter().cloned().fold(0.0f32, f32::max),
            "ceiling_contacts_pct_of_airborne": 100.0 * air.iter().filter(|x| x.z > 200.0).count() as f64 / at,
            "hist_10mm": alt_hist,
        },
        "walls": {
            "pct_samples_within_20mm": 100.0 * near_wall as f64 / n as f64,
            "pct_airborne_within_20mm": 100.0 * near_wall_air as f64 / at,
            "mean_wall_distance_airborne_mm": mean(&col(&air, |x| x.wall_dist)),
        },
        "turning": {
            "mean_abs_yaw_rate_rad_s": mean(&air_yaw_rate),
            "p95_abs_yaw_rate_rad_s": pct(&air_yaw_rate, 0.95),
            "pct_airborne_turning_gt_0p5": 100.0 * air.iter().filter(|x| x.yaw_rate.abs() > 0.5).count() as f64 / at,
            "total_turn_rad_airborne": total_turn,
            "full_circles_airborne": total_turn / std::f64::consts::TAU,
            "net_yaw_change_rad": (last.yaw - first.yaw) as f64,
            "steer_differential_mean_abs": mean(&col(&air, |x| (x.steer_r - x.steer_l).abs())),
        },
        // Yaw attribution. Is the continuous turning a constant body-frame
        // torque (a body-model bias, as pitch was) or the connectome's own
        // asymmetric motor output (real behaviour the body is executing)?
        // `frac_tau_yaw_positive` answers the first: a value near 0 or near 1
        // is a one-signed torque; near 0.5 is a torque that changes sign.
        "yaw": {
            "note": "tau_yaw is the whole yaw moment: tau_aero[2] = WING_DY * (fr[0] - fl[0]). \
                     Split into the component commanded by the stroke-plane tilt difference and \
                     the component commanded by the wing-amplitude difference.",
            "mean_tau_yaw": mean(&tau_yaw),
            "mean_abs_tau_yaw": mean(&col(&air, |x| x.tau_aero[2].abs())),
            "tau_yaw_tilt_component_mean": mean(&tilt_term),
            "tau_yaw_amp_component_mean": mean(&amp_term),
            "frac_tau_yaw_positive": pos_y,
            "frac_tau_yaw_negative": neg_y,
            "mean_body_yaw_rate_rad_s": mean(&col(&air, |x| x.wyaw)),
            "mean_abs_body_yaw_rate_rad_s": mean(&col(&air, |x| x.wyaw.abs())),
            "mean_world_yaw_rate_rad_s": mean(&col(&air, |x| x.yaw_rate)),
            "mean_tau_damp_yaw": mean(&col(&air, |x| x.tau_damp[2])),
            "mean_tilt_l_rad": mean(&col(&air, |x| x.tilt_l)),
            "mean_tilt_r_rad": mean(&col(&air, |x| x.tilt_r)),
            "mean_tilt_differential_rad": mean(&col(&air, |x| x.tilt_r - x.tilt_l)),
            // A rate sitting on MAX_OMEGA is the numerical guard, not a
            // measurement, so clamp occupancy is reported per axis.
            "clamp_pct_of_airborne": {
                "roll": 100.0 * clamp_pct(0),
                "pitch": 100.0 * clamp_pct(1),
                "yaw_body": 100.0 * clamp_pct(2),
                "yaw_world": 100.0 * yaw_clamp,
            },
            "max_omega_clamp_rad_s": crate::body::MAX_OMEGA,
            "pct_parked_speed_lt_10mm_s": 100.0 * parked,
            "pct_touching_two_walls": 100.0 * cornered,
            "steering_pool_hz_left": steer_hz_l,
            "steering_pool_hz_right": steer_hz_r,
            "steering_pool_hz_difference": steer_hz_l - steer_hz_r,
            "pct_steering_readout_saturated": 100.0 * steer_readout_sat,
        },
        "loom_response": {
            "note": "Direct test of whether the room reaches the brain: the looming-wall \
                     channel is the only wall proximity signal the connectome receives.",
            "pearson_loom_vs_steer_differential": loom_steer_r,
            "pearson_loom_vs_yaw_rate": loom_yaw_r,
            "mean_abs_yaw_rate_when_loom_gt_0p5": looming_yaw,
            "mean_abs_yaw_rate_when_loom_lt_0p2": calm_yaw,
            "mean_abs_steer_when_loom_gt_0p5": looming_steer,
            "mean_abs_steer_when_loom_lt_0p2": calm_steer,
            "samples_imminent_wall": imminent.len(),
            "samples_calm": calm.len(),
        },
        "motors_mean": {
            "flight_power_l": mean(&col(s, |x| x.pow_l)),
            "flight_power_r": mean(&col(s, |x| x.pow_r)),
            "flight_steer_l": mean(&col(s, |x| x.steer_l)),
            "flight_steer_r": mean(&col(s, |x| x.steer_r)),
            "walk_l": mean(&col(s, |x| x.walk_l)),
            "walk_r": mean(&col(s, |x| x.walk_r)),
            "land_l": mean(&col(s, |x| x.land_l)),
            "land_r": mean(&col(s, |x| x.land_r)),
            "mn9": mean(&col(s, |x| x.mn9)),
        },
        "descending_mean": {
            "dn02_flight_power": mean(&col(s, |x| x.dn02)),
            "dn07_power_decrease": mean(&col(s, |x| x.dn07)),
            "sapp_flight_state": mean(&col(s, |x| x.sapp)),
            "landing_dn": mean(&col(s, |x| x.land_dn)),
            "dn_filtered": mean(&col(s, |x| x.dn_filt)),
        },
        "sensory_mean": {
            "odor_l": mean(&col(s, |x| x.odor_l)),
            "odor_r": mean(&col(s, |x| x.odor_r)),
            "flow_l": mean(&col(s, |x| x.flow_l)),
            "flow_r": mean(&col(s, |x| x.flow_r)),
            "loom": mean(&col(s, |x| x.loom)),
            "taste_hz": mean(&col(s, |x| x.taste)),
        },
        "occupancy_airborne": grid(&air),
        "occupancy_ground": grid(&sel(s, |x| x.mode == 0)),
        "speed_hist_25mm_s": speed_hist,
        "cruise_samples": cruise.len(),
    })
}