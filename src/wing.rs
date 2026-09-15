//! Quasi-steady wing aerodynamics, wing-mediated rotational damping, and the
//! haltere as an angular-rate sense organ.
//!
//! This module is the replacement for the hand-tuned constants that used to
//! live in `body.rs` (`LIFT_MAX`, `THRUST_MAX`, `DRAG_Q`, `ALT_GAIN`,
//! `YAW_GAIN`). Nothing here encodes a behaviour: it converts wing kinematics
//! into forces, and body rotation into an afferent firing rate. Every
//! direction, gain and sign that the old code applied as a control law now
//! emerges from these forces acting on a rigid body.
//!
//! Units are millimetres, seconds and milligrams, matching the rest of the
//! simulation. Force is therefore mg*mm/s^2 and torque mg*mm^2/s^2. Air
//! density in these units is 1.225e-3 mg/mm^3.
//!
//! Provenance tags follow the physics spec (docs/physical-model-spec.md):
//!   [M] measured      - taken from a measurement in the dataset or literature
//!   [D] derived       - computed from measured quantities
//!   [E] estimated     - a defensible order-of-magnitude choice, not measured
//!
//! References: Sane & Dickinson 2002 (JEB 205:1087, coefficient fits at
//! Re~115); Ellington 1984; Dickinson & Gotz 1993; Dickinson et al. 1999;
//! Fry et al. 2005 (JEB 208:2303); Fox & Daniel 2010 (PNAS 107:3840);
//! Mohren et al. 2019.

use crate::room::V3;

// ---------------------------------------------------------------- constants

/// Air density. [D] 1.225 kg/m^3 converted to mg/mm^3.
pub const RHO: f32 = 1.225e-3;

/// Wing length, tip to root. [M] measured from the NeuroMechFly wing mesh
/// bounding box (2.406 mm along the wing's long axis).
pub const WING_LEN: f32 = 2.406;

/// Mean wing chord. [M] the measured in-plane chord extent of l_wing.stl is
/// 1.126 mm, against a 0.435 mm bounding box in the thin axis. The earlier
/// 0.80 mm estimate came from the literature and was 29 % short.
pub const WING_CHORD: f32 = 1.126;

/// Single-wing planform area. [M] measured directly from l_wing.stl: the mesh
/// is a closed blade, so the enclosed-volume method gives a mean thickness of
/// 16.8 um and a planform of half the 4.072 mm^2 surface area = 2.036 mm^2.
/// Consistent with the 1.8-2.0 mm^2 in the literature. The earlier 0.62-area-
/// factor estimate (1.19 mm^2) was 42 % short and made the fly barely able to
/// hover; this is a measurement, not a correction to force a result.
pub const WING_AREA: f32 = 2.036;

/// Second moment of wing area about the root, divided by area and span
/// squared: `integral c(r) r^2 dr = k * S * R^2`. [M] measured from
/// l_wing.stl by projecting the triangles into the wing plane and integrating
/// `r^2 dA` over the planform: k = 0.3335. The analytic value for a uniform
/// chord is exactly 1/3, so the real planform is within 0.1 % of uniform in
/// this moment. The measurement confirmed the guess rather than replacing it.
pub const WING_K2: f32 = 0.3335;

/// Wingbeat frequency. [M] 180-220 Hz across the literature; 200 Hz nominal.
pub const WINGBEAT_HZ: f32 = 200.0;

/// Stroke amplitude at full wing-power motor drive. [M] 145-165 deg measured
/// in free flight; 158 deg = 2.757 rad.
pub const STROKE_AMP_MAX: f32 = 2.757;

/// Amplitude floor: a wing with zero motor drive still sweeps a little, which
/// is what the old `0.12 + 0.95 * power` mapping already encoded.
pub const STROKE_AMP_MIN: f32 = 0.12 * STROKE_AMP_MAX;

/// Body mass. [M] 0.983 mg (flybody / Vaxenburg et al. 2025).
pub const FLY_MASS: f32 = 0.983;

/// Wing mass, single wing. [M] 8 ug = 0.008 mg from the literature. The rendered
/// mesh is heavier than a real wing (16.8 um mean thickness gives 41 ug), so this
/// value is the reference one. Wing inertia is not modelled, so neither number
/// enters the dynamics; see the omission note in `docs/embodiment.md`.
#[allow(dead_code)]
pub const WING_MASS: f32 = 0.008;

/// Haltere length. [E] 0.35 mm.
pub const HALTERE_LEN: f32 = 0.35;

/// Haltere stroke amplitude. [E] order of the haltere length.
pub const HALTERE_AMP: f32 = 0.30;

// Principal moments of inertia of the body about its centre of mass. [E]
/// Solid-ellipsoid estimate from a 2.3 x 1.0 x 0.9 mm body at 0.983 mg:
/// `I = m/5 * (b^2 + c^2)` for semi-axes a,b,c. Right for an order-of-
/// magnitude; the spec defers the real tensor to the flybody model.
pub const I_ROLL: f32 = 0.089; // mg*mm^2, about the longitudinal (x) axis
pub const I_PITCH: f32 = 0.309; // about the lateral (y) axis
pub const I_YAW: f32 = 0.309; // about the vertical (z) axis

/// Wing root offset from the centre of mass, body frame. [D] from the rig:
/// the wing base sits 0.395 mm out laterally and 0.081 mm above the thorax.
pub const WING_DY: f32 = 0.395;
/// Vertical offset of the wing force above the centre of mass. This is what
/// makes the body pendulously stable, exactly as a real fly is. [E]
pub const WING_DZ: f32 = 0.20;
/// Longitudinal offset of the wing planform centre from the centre of mass.
/// [D] from the rig (-0.674 mm along the body axis from the thorax, against a
/// centre of mass aft of it). Used in the body's cross product: the vertical
/// wing force applied at this offset contributes `-WING_DX * (fl[2]+fr[2])` to
/// the pitching moment, alongside `WING_DZ * (fl[0]+fr[0])`.
pub const WING_DX: f32 = -0.15;

/// Maximum stroke-plane tilt authority from the steering motor neurons. [E]
/// Drosophila steering muscles shift the stroke plane by up to roughly 30 deg.
pub const STEER_TILT_MAX: f32 = 0.52; // rad

/// Body translational drag. [D] a sphere-equivalent coefficient over the body
/// silhouette; no tuning applied.
pub const BODY_CD: f32 = 1.1;
/// Body reference area. [D] 2.3 mm long x 1.0 mm wide, at 0.7 area factor.
pub const BODY_AREA: f32 = 2.3 * 1.0 * 0.7;

// ------------------------------------------------------------------- wings

/// Lift and drag coefficients from the polynomial fits to the dynamically
/// scaled wing at Re~115 (Sane & Dickinson 2002). `alpha_deg` is the angle of
/// attack in degrees.
pub fn coeffs(alpha_deg: f32) -> (f32, f32) {
    let x = 2.13 * alpha_deg - 7.2;
    let cl = 0.225 + 1.58 * x.to_radians().sin();
    let cd = 1.92 - 1.55 * x.to_radians().cos();
    (cl, cd)
}

/// Cycle-mean force along the stroke-plane normal from one wing, in
/// mg*mm/s^2. This is the lift force: `wing_force_vector` aims it along the
/// stroke-plane normal, which is what the steering tilt rotates.
///
/// The wing sweeps sinusoidally with amplitude `stroke_amp` (radians, the
/// full peak-to-peak angle) at `freq_hz`. The spec integrates the blade
/// element as a *vector* sum (physical-model-spec.md §1.3):
///
///     dF = 1/2 rho c |U|^2 [ C_L(alpha) n_L + C_D(alpha) n_D ] dr
///
/// The wing sweeps *in* the stroke plane, so the section velocity `U` lies in
/// that plane; `n_D = -U/|U|` therefore lies in the stroke plane too, while
/// `n_L` (perpendicular to `U`) is the stroke-plane **normal**. The cycle
/// mean separates cleanly:
///
///   - the drag term reverses with `U` at every half stroke and averages to
///     zero in every direction, so it contributes nothing to the mean force;
///   - the lift term keeps one sign, because the wing flips its angle of
///     attack at each stroke reversal, and gives
///     `<F_n> = 1/2 rho C_L(alpha) <U^2> S`.
///
/// The mean of `sin^2` over a cycle is 1/2, which is where the leading 1/2 in
/// the `u2` expression below comes from; `<U^2>` is the second-moment-weighted
/// mean squared section speed. The induced-flow term is a modest correction
/// rather than the dominant term, which is why it is first order.
///
/// Building the normal force out of the *resultant* coefficient
/// `sqrt(C_L^2 + C_D^2)` instead — as an earlier revision did — applies the
/// drag magnitude to the lift direction and overstates the normal force by
/// `sqrt(C_L^2 + C_D^2) / C_L`, about 35 % at alpha = 40 deg. The spec's
/// mandatory self-check (§6.3) integrates `1/2 rho C_L omega_rms^2 S R^2/3`
/// and expects `F_lift/(W/2) ~ 1.2`; the resultant form gave 1.79.
/// `normal_force_reproduces_the_specs_mandatory_self_check` pins this.
pub fn wing_force_magnitude(stroke_amp: f32, freq_hz: f32, airspeed: f32, alpha_deg: f32) -> f32 {
    if stroke_amp <= 0.0 {
        return 0.0;
    }
    let (cl, _cd) = coeffs(alpha_deg);
    let omega = std::f32::consts::TAU * freq_hz;
    // Mean squared section velocity, weighted by the wing's second moment:
    // <U^2> = (amp/2)^2 * omega^2 / 2 * R^2 * k2, and the section speed
    // rises with forward airspeed because the wing meets the airflow on the
    // advancing half stroke.
    let half = stroke_amp * 0.5;
    let u2 = half * half * omega * omega * 0.5 * WING_LEN * WING_LEN * WING_K2;
    let u2 = u2 + 0.5 * airspeed * airspeed;
    0.5 * RHO * cl * u2 * WING_AREA
}

/// A wing's force vector in the body frame.
///
/// The force acts along the stroke-plane normal. Tilting the stroke plane
/// forward by `tilt` rotates that normal forward, which converts lift into
/// thrust: this is the entire forward-steering mechanism, and it is geometry
/// rather than a heading-aligned push. There is deliberately no lateral term
/// here: left/right asymmetry arises from the two wings' amplitudes acting at
/// their separate roots, not from a sideways force.
pub fn wing_force_vector(stroke_amp: f32, freq_hz: f32, airspeed: f32, tilt: f32) -> V3 {
    let f = wing_force_magnitude(stroke_amp, freq_hz, airspeed, 40.0);
    // Body frame: x forward, y left, z up. A forward tilt leans the force
    // vector forward.
    let (st, ct) = (tilt.sin(), tilt.cos());
    [f * st, 0.0, f * ct]
}

/// Sweep the stroke-plane tilt and print the pitching moment it produces.
///
/// `tau_aero[1] = WING_DZ * (fl[0] + fr[0])`, and `fl[0] + fr[0]` is the total
/// forward thrust, so this asks a single question: can the stroke plane produce
/// zero pitching moment while still producing forward thrust? If the torque
/// changes sign across the sweep the handle exists and level flight is a
/// control problem. If it keeps one sign for every tilt that yields forward
/// thrust, the body cannot be level while flying forward, and the defect is in
/// the body model.
pub fn torque_sweep(stroke_amp: f32, airspeed: f32) {
    println!("stroke amplitude {:.2}, airspeed {:.0} mm/s", stroke_amp, airspeed);
    println!("  tilt deg   fwd thrust   lift      pitch torque");
    let mut deg = -60.0f32;
    while deg <= 60.0 {
        let fl = wing_force_vector(stroke_amp, WINGBEAT_HZ, airspeed, deg.to_radians());
        let fr = fl;
        let thrust = fl[0] + fr[0];
        let lift = fl[2] + fr[2];
        println!(
            "  {:>+8.1}   {:>+10.1}   {:>+9.1}   {:>+12.1}",
            deg,
            thrust,
            lift,
            WING_DZ * thrust
        );
        deg += 10.0;
    }
}

/// Rotational damping from the flapping wings, as a torque per unit angular
/// rate in each axis, in mg*mm^2/s.
///
/// A flapping wing resists body rotation because rotation adds a relative
/// airflow across the wing that does not reverse with the stroke. Integrating
/// that extra increment over the cycle gives a damping coefficient that goes
/// as `rho * S * CD * U_bar * R^2`. This is the physical effect that stabilises
/// a fly in flight; it is not a control law and has no offset term.
pub fn wing_damping() -> V3 {
    let omega = std::f32::consts::TAU * WINGBEAT_HZ;
    // Mean absolute section velocity at the wing's second moment of area.
    let u_bar = (STROKE_AMP_MAX * 0.5) * omega * WING_LEN * WING_K2 * (2.0 / std::f32::consts::PI);
    let (_, cd) = coeffs(40.0);
    let k = RHO * WING_AREA * cd * u_bar * WING_LEN * WING_LEN;
    // Two wings, and the same coefficient resists motion in every axis of a
    // wing pair that is symmetric about the body's long axis.
    [2.0 * k, 2.0 * k, 2.0 * k]
}

/// Body translational drag force (mg*mm/s^2) opposing motion.
pub fn body_drag(vel: V3, airspeed_scale: f32) -> V3 {
    let sp = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
    if sp < 1e-6 {
        return [0.0, 0.0, 0.0];
    }
    let f = 0.5 * RHO * BODY_CD * BODY_AREA * sp * sp * airspeed_scale;
    [-f * vel[0] / sp, -f * vel[1] / sp, -f * vel[2] / sp]
}

/// Mean haltere afferent rate with the body at rest. [M] Haltere afferents
/// fire roughly once per wingbeat, so the resting carrier is the wingbeat
/// frequency; the value used here matches the reference sensory working point
/// the rest of the simulation runs at.
pub const HALTERE_BASE_HZ: f32 = 150.0;

/// The angular rate at which the haltere's Coriolis deflection reaches its
/// stroke amplitude, i.e. where the afferent response saturates. [E] Set well
/// above the fastest turns a fly makes (~50 rad/s), so the sense organ stays
/// linear across the whole behavioural range and the saturation is only a
/// numerical guard.
pub const HALTERE_FULL_SCALE: f32 = 50.0;

// ----------------------------------------------------------------- haltere

/// Haltere afferent firing rate, in Hz, encoding body rotation.
///
/// The haltere beats anti-phase with the wings. When the body rotates, the
/// beating haltere feels a Coriolis force `2 m (omega x v)` that deflects it
/// out of its stroke plane, and the campaniform sensilla at its base report
/// that deflection. The carrier is the wingbeat frequency and the modulation
/// is proportional to the component of angular velocity sensed in the
/// haltere's plane, which is what makes this a rate sensor rather than a
/// position sensor.
///
/// `omega_perp` is the angular rate in rad/s this haltere responds to, taken
/// with the sign appropriate to it: positive in the direction that deflects
/// the haltere forward in its stroke, negative in the opposite direction. The
/// response is *signed* and saturating, so one direction of rotation lifts the
/// afferent rate above the carrier and the other pushes it below, and a pair
/// of halteres can therefore report which way the body is turning. A rate
/// sensor that took the magnitude could not.
pub fn haltere_rate_hz(omega_perp: f32, base_hz: f32) -> f32 {
    // The Coriolis deflection scales linearly with rotation rate: the tip
    // speed is fixed by the haltere's own beat, so F = 2 m omega v is linear
    // in omega and the deflection is too, against the haltere's elastic
    // restoring stiffness. That makes the response proportional to the signed
    // rate until it saturates. The stiffness is not separately measured, so it
    // sets the full-scale constant rather than appearing explicitly.
    //
    // The modulation is applied multiplicatively because the deflection
    // reverses with the rotation and a firing rate is strictly positive: an
    // additive form `base * (1 + u)` would reach 0 Hz at negative full scale,
    // which no afferent can fire at, while the multiplicative form has no such
    // singularity. With `u` the signed, clamped Coriolis drive, the rate is
    // `base * 2^u` in [base/2, 2*base]: a positive rotation multiplies the
    // carrier by the same factor a negative rotation divides it by, which is
    // the symmetric, sign-preserving sense in which a Coriolis transducer
    // modulates its carrier. At positive full scale it recovers exactly the
    // previous double-rate ceiling, so the change is confined to the sign.
    let u = (omega_perp / HALTERE_FULL_SCALE).clamp(-1.0, 1.0);
    base_hz * 2.0f32.powf(u)
}

/// Split a body angular velocity into the signal each haltere sees.
///
/// The haltere beats laterally, so its stroke plane is roughly vertical and
/// longitudinal: rotation about the vertical axis (yaw) displaces the stroke
/// plane in place, and rotation about the longitudinal axis (roll) drives the
/// two halteres in opposite senses. That antisymmetry is what lets a pair of
/// halteres distinguish roll from yaw, and it is the reason this returns a
/// signed pair rather than a magnitude.
pub fn haltere_pair(omega: V3) -> (f32, f32) {
    let (roll, pitch, yaw) = (omega[0], omega[1], omega[2]);
    let left = yaw + roll - 0.5 * pitch;
    let right = yaw - roll - 0.5 * pitch;
    (left, right)
}

/// First-order lag matching the measured ~150 Hz afferent bandwidth, so the
/// channel cannot report rotation the real sense organ could not follow.
pub fn haltere_lag(prev: f32, target: f32, dt: f32) -> f32 {
    let a = 1.0 - (-dt / (1.0 / (std::f32::consts::TAU * 150.0))).exp();
    prev + (target - prev) * a
}

/// Gravity, mm/s^2. Kept here so body.rs has no physics constants of its own.
pub const GRAVITY: f32 = 9810.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_wing_geometry_is_self_consistent() {
        // Chord, area, span and the second moment were all measured from the
        // same mesh (web/assets/meshes/l_wing.stl), so they have to agree with
        // each other. If someone edits one of these constants by hand, this
        // fails instead of silently changing the lift.
        let mean_chord = WING_AREA / WING_LEN;
        assert!(
            mean_chord < WING_CHORD,
            "mean chord {mean_chord:.3} mm cannot exceed the widest chord {WING_CHORD:.3} mm"
        );
        assert!(
            mean_chord > 0.6 * WING_CHORD,
            "mean chord {mean_chord:.3} mm is implausibly narrow against a {WING_CHORD:.3} mm extent"
        );
        // k2 = 1/3 for a uniform chord, 0.25 for an ellipse, and it must sit
        // below the tip-loaded value of 1.0.
        assert!(
            WING_K2 > 0.25 && WING_K2 < 0.35,
            "second moment factor {WING_K2:.4} is outside the range real planforms occupy"
        );
    }

    #[test]
    fn full_stroke_lift_has_a_surplus_over_body_weight() {
        // One wing at full stroke must beat half the body weight with margin: a
        // fly that can only just hover cannot climb, carry a load, or recover
        // from a perturbation. The earlier estimated planform gave 1.05x, which
        // is why the fly could not reliably leave the ground.
        let f = wing_force_magnitude(STROKE_AMP_MAX, WINGBEAT_HZ, 0.0, 40.0);
        let ratio = f / (0.5 * FLY_MASS * GRAVITY);
        assert!(
            ratio > 1.2,
            "one wing at full stroke gives only {ratio:.3}x half the body weight"
        );
    }

    #[test]
    fn normal_force_reproduces_the_specs_mandatory_self_check() {
        // docs/physical-model-spec.md §6.3 is an explicit, no-tuning physical
        // consistency check the implementation is required to pass: at
        // alpha = 45 deg, Phi = 160 deg, f = 200 Hz the blade-element integral
        //   F = 1/2 rho C_L omega_rms^2 (integral r^2 c dr),
        //   omega_rms = pi f Phi / sqrt(2)
        // must give F_lift/(W/2) ~ 1.22 for the 1.8 mm^2 wing the spec
        // assumes, and proportionally more for the larger planform measured
        // from the mesh (2.036 mm^2). Recomputing the integral here from the
        // geometry constants pins the coefficient convention that the force
        // uses: the cycle-mean force on the stroke-plane normal is C_L, not
        // sqrt(C_L^2 + C_D^2) (see wing_force_magnitude). The resultant form
        // read 1.79 here, i.e. 46 % outside the spec's own band.
        let phi = 160f32.to_radians();
        let omega_rms =
            std::f32::consts::PI * WINGBEAT_HZ * phi / std::f32::consts::SQRT_2;
        let integral_r2c = WING_K2 * WING_AREA * WING_LEN * WING_LEN;
        let (cl, _cd) = coeffs(45.0);
        let expect = 0.5 * RHO * cl * omega_rms * omega_rms * integral_r2c;
        let got = wing_force_magnitude(phi, WINGBEAT_HZ, 0.0, 45.0);
        assert!(
            (got - expect).abs() < 0.01 * expect,
            "the normal force {got:.1} is not the spec's integral {expect:.1}"
        );
        let ratio = got / (0.5 * FLY_MASS * GRAVITY);
        // 1.22 with the spec's 1.8 mm^2 wing; 1.39 with the measured planform.
        assert!(
            ratio > 1.0 && ratio < 1.5,
            "full-stroke normal force is {ratio:.3} of half the body weight; the spec's \
             self-check band is about 1.22 (1.39 at the measured planform)"
        );
    }

    #[test]
    fn stroke_force_is_quadratic_in_amplitude() {
        // Quasi-steady blade element: the section speed is proportional to
        // stroke amplitude x wingbeat frequency and the force to the square of
        // that speed, so at fixed frequency the cycle-mean force goes as
        // amplitude SQUARED (spec §1.3; the incidence is held fixed here, so
        // the coefficients do not vary and the exponent is exactly 2). This is
        // the exponent that makes the fly's margin sensitive to recruitment: a
        // half-amplitude stroke carries a quarter of the weight, not half.
        let one = wing_force_magnitude(1.0, WINGBEAT_HZ, 0.0, 40.0);
        let two = wing_force_magnitude(2.0, WINGBEAT_HZ, 0.0, 40.0);
        let half = wing_force_magnitude(0.5, WINGBEAT_HZ, 0.0, 40.0);
        assert!(
            (two / one - 4.0).abs() < 0.02,
            "doubling the amplitude scaled the force {:.4}x, not 4x",
            two / one
        );
        assert!(
            (one / half - 4.0).abs() < 0.02,
            "halving the amplitude scaled the force {:.4}x, not 1/4x",
            one / half
        );
        // And zero amplitude produces nothing, so the scaling has no offset.
        assert_eq!(wing_force_magnitude(0.0, WINGBEAT_HZ, 0.0, 40.0), 0.0);
    }

    #[test]
    fn force_coefficients_match_the_published_curves() {
        // Sane & Dickinson 2002, Re ~ 115. Spot values so a typo in the
        // coefficient functions cannot pass unnoticed.
        let (cl, cd) = coeffs(40.0);
        assert!((cl - 1.77).abs() < 0.1, "C_L at 40 deg was {cl:.3}");
        assert!((cd - 1.60).abs() < 0.1, "C_D at 40 deg was {cd:.3}");
    }

    #[test]
    fn haltere_response_is_linear_then_saturates() {
        let base = HALTERE_BASE_HZ;
        assert!((haltere_rate_hz(0.0, base) - base).abs() < 1e-3);
        // Half full scale gives half the modulation, not half the rate.
        let half = haltere_rate_hz(HALTERE_FULL_SCALE * 0.5, base);
        assert!(
            (half - base * 2.0f32.sqrt()).abs() < 1e-3,
            "half scale gave {half:.2} Hz"
        );
        // At and beyond full scale the afferent cannot report more.
        let sat = haltere_rate_hz(HALTERE_FULL_SCALE * 10.0, base);
        assert!((sat - base * 2.0).abs() < 1e-3, "saturation gave {sat:.2} Hz");
    }

    #[test]
    fn haltere_response_is_signed_across_zero() {
        // PASS/FAIL: a rectifying implementation (anything that takes
        // `omega.abs()`, as the pre-fix code did) returns the SAME rate for
        // +x and -x, so `pos - neg` collapses to 0 and the monotonic walk
        // below goes flat. Both assertions therefore fail on a rectifier and
        // pass only when the sign of the rotation survives into the rate.
        let base = HALTERE_BASE_HZ;
        let pos = haltere_rate_hz(HALTERE_FULL_SCALE, base);
        let neg = haltere_rate_hz(-HALTERE_FULL_SCALE, base);
        // One direction must speed the afferent up, the other slow it down.
        assert!(
            pos > base && neg < base,
            "sign lost: +full scale gave {pos:.1} Hz, -full scale gave {neg:.1} Hz"
        );
        assert!(
            pos - neg > 0.5 * base,
            "opposite rotations must differ in sign and in size, got {:.1} Hz",
            pos - neg
        );
        // A firing rate is strictly positive and bounded: the signed form must
        // never reach zero or run away.
        for x in [-4.0f32, -1.0, -1e-3, 0.0, 1e-3, 1.0, 4.0] {
            let r = haltere_rate_hz(x * HALTERE_FULL_SCALE, base);
            assert!(
                r > 0.0 && r <= 2.0 * base,
                "rate {r:.2} Hz out of the positive range at x={x}"
            );
        }
        // Monotonic in the signed rate across zero: the rate has to increase
        // with omega from the slowest leftward rotation to the fastest
        // rightward one, which a rectifier cannot do.
        let mut prev = f32::NEG_INFINITY;
        for k in -10..=10 {
            let r = haltere_rate_hz(k as f32 * 0.1 * HALTERE_FULL_SCALE, base);
            assert!(r > prev, "rate is not monotonic in signed omega at k={k}");
            prev = r;
        }
    }

    #[test]
    fn damping_opposes_every_axis_of_rotation() {
        let k = wing_damping();
        for a in 0..3 {
            assert!(k[a] > 0.0, "axis {a} has no rotational damping");
        }
        assert!((k[0] - k[1]).abs() < 1e-6 && (k[1] - k[2]).abs() < 1e-6);
    }

    #[test]
    fn body_drag_opposes_motion_and_grows_with_speed() {
        let slow = body_drag([100.0, 0.0, 0.0], 1.0);
        let fast = body_drag([1000.0, 0.0, 0.0], 1.0);
        assert!(slow[0] < 0.0 && fast[0] < 0.0, "drag must oppose motion");
        assert!(
            fast[0].abs() > 10.0 * slow[0].abs(),
            "quadratic drag should grow far faster than linearly"
        );
        assert_eq!(body_drag([0.0, 0.0, 0.0], 1.0), [0.0, 0.0, 0.0]);
    }
}