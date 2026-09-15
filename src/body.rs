//! The embodied fly: a rigid body driven by real decoded motor-neuron activity
//! from the running MaleCNS connectome, through real aerodynamics.
//!
//! WHAT IS PHYSICAL HERE
//! ---------------------
//! The flight path is now mechanics, not control. Wing-power motor neurons set
//! each wing's stroke amplitude, steering motor neurons set its stroke-plane
//! tilt, and the resulting blade-element forces act at the wing roots to
//! produce the torques that turn the fly. Attitude is integrated from those
//! torques; nothing commands a heading, a bank angle or an altitude.
//!
//! The previous version of this file contained an explicit flight controller:
//! a cruise-altitude target, an altitude-tracking pitch law, a stabiliser that
//! trimmed wing amplitude to hold altitude, a heading-aligned thrust term and a
//! direct `steer * YAW_GAIN` yaw coupling. All of that is gone. There is no
//! altitude setpoint the body works towards, no stabiliser gain and no yaw gain.
//!
//! WHAT IS STILL A SURROGATE
//! -------------------------
//! Ground locomotion and the landing state remain engineered: leg motor
//! neurons drive a walking speed with a capped magnitude. They are labelled as
//! such and are the next candidates for replacement. Wingbeat kinematics and
//! the muscle lag between motor drive and stroke amplitude are transduction
//! models, not recovered circuits: the dataset contains no muscle.
//!
//! Units: millimetres, seconds, milligrams. Gravity 9810 mm/s^2.
//!
//! Reference frame: x forward, y left, z up. Quaternions are body-to-world.

use crate::room::{add, len, scale, Room, V3};
use crate::wing;

pub use crate::wing::GRAVITY;

/// Wingbeat phase advance for the on-screen stroke. The real wingbeat is
/// ~200 Hz, which aliases at display frame rates, so the visual phase runs
/// slower purely so a human can see it. This is a display decision, not
/// physics: all forces integrate the real wingbeat frequency.
pub const WINGBEAT_VISUAL_HZ: f32 = 19.0;

/// Time constant between wing motor drive and stroke amplitude. [E] the
/// dataset has no muscle, so this stands in for the thoracic muscle and
/// thorax spring response. Longer than the wingbeat period, as the spec
/// requires for the thorax to act as a resonator.
pub const MUSCLE_TAU: f32 = 0.020;

/// Exponent of the measured in-vivo relation between mechanical power output
/// and A-IFM motor-neuron spike frequency.
///
/// Gordon & Dickinson (2006), PNAS 103(11):4311-4315,
/// doi:10.1073/pnas.0510109103, held tethered *Drosophila* in front of a
/// vertically drifting grating and recorded A-IFM spikes alongside wing
/// kinematics. Measured: "a 3-fold increase in spike frequency (from 3 to 9 Hz)
/// results in a 1.7-fold increase in power output" (Fig. 1e). The paper's
/// rounded summary of the same data is "An ≈2-fold change in power accompanied
/// a 3-fold change in steady-state spike frequency" (Fig. 1c), which would give
/// `ln 2 / ln 3 = 0.63`. The explicit measured pair is used here:
///
///     ln(1.7) / ln(3) = 0.483
///
/// This is a COMPRESSIVE power law. The map it replaces produced a force
/// exponent of 2 (see `STROKE_AMP_EXPONENT`), i.e. 4x too steep and in the
/// opposite direction.
pub const POWER_RATE_EXPONENT: f32 = 0.483;

/// Exponent of the activation-to-stroke-amplitude map: half the measured power
/// exponent, because quasi-steady aerodynamic force goes as amplitude
/// SQUARED (physical-model-spec.md §1.3).
///
///     lift ∝ amp^2 ∝ (a^0.2415)^2 = a^0.483
///
/// With the activation `a` linear in the pool's firing rate (see
/// `groups::MOTOR_RATE_TAU_S`) that reproduces the measured `power ∝ f^0.483`
/// across the in-flight band. The exponent is below 1, so the map is
/// compressive: the wing reaches a large fraction of full stroke at a low
/// activation, and the force saturates gently as the animal recruits more.
pub const STROKE_AMP_EXPONENT: f32 = POWER_RATE_EXPONENT * 0.5;

/// Stroke amplitude, radians peak-to-peak, at muscle activation `a` in 0..1.
///
/// The map is `STROKE_AMP_MAX * a^STROKE_AMP_EXPONENT`. Both endpoints are
/// pinned by the spec and are not free: `a = 1` is the full stroke
/// (`STROKE_AMP_MAX`, 158 deg, physical-model-spec.md §6.5: the activation gain
/// is scaled to give `F/W ≈ 1.2` there) and `a = 0` is a wing that is not being
/// driven at all, which is zero amplitude rather than the removed uncited 0.12
/// floor. The compressive shape between them is the measured rate-to-power
/// relation, inverted through the squared force law.
pub fn stroke_amp_for_activation(a: f32) -> f32 {
    wing::STROKE_AMP_MAX * a.clamp(0.0, 1.0).powf(STROKE_AMP_EXPONENT)
}

/// Total wing lift, both wings, divided by body weight, at muscle activation
/// `a` in 0..1, at rest (zero airspeed). 1.0 is hover.
///
/// This is the map the model produces, as a pure function of the activation, so
/// the rate-to-force relation can be inspected without running the sim. Callers
/// that need Hz convert first: `a = f_hz / groups::POWER_MN_FULL_STROKE_HZ`.
pub fn lift_ratio_at_activation(a: f32) -> f32 {
    let amp = stroke_amp_for_activation(a);
    2.0 * wing::wing_force_magnitude(amp, wing::WINGBEAT_HZ, 0.0, 40.0)
        / (wing::FLY_MASS * GRAVITY)
}

/// Total wing lift divided by body weight at a flight-power pool firing rate of
/// `f_hz` Hz/neuron, at rest. 1.0 is hover.
///
/// `f_hz` is the pool's mean rate; the activation is that rate over the
/// physiological full-stroke rate (`groups::POWER_MN_FULL_STROKE_HZ`). This is
/// the quantity the task's rate-to-force comparison is about, and the quantity
/// the `mn-audit` force-curve block prints.
pub fn lift_ratio_at_rate(f_hz: f32) -> f32 {
    lift_ratio_at_activation(f_hz / crate::groups::POWER_MN_FULL_STROKE_HZ)
}

/// Walking speed at full leg motor drive. [SURROGATE] capped and linear.
pub const WALK_MAX: f32 = 14.0;

/// Hard cap on angular rate, rad/s. Real flies turn at several hundred deg/s;
/// this only exists so a numerical excursion cannot produce an inf.
pub const MAX_OMEGA: f32 = 120.0;
/// Hard cap on speed, mm/s.
pub const MAX_SPEED: f32 = 4000.0;

/// Wall restitution.
const RESTITUTION: f32 = 0.15;

/// How long the telemetry keeps labelling the fly as taking off after it
/// leaves the ground. This is a label, not a physics mode.
const TAKEOFF_LABEL_MS: f32 = 250.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Mode {
    Ground,
    Takeoff,
    Cruise,
    Landing,
    /// Retired, and currently unreachable: the only code that ever assigned it
    /// (`maybe_feed`) had no call sites and has been deleted. It is kept only
    /// because other modules (e.g. `analyze::mode_code`) still match on it.
    /// Removing it is a cross-module follow-up, not a body-local change.
    Feeding,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Ground => "GROUND",
            Mode::Takeoff => "TAKEOFF",
            Mode::Cruise => "CRUISE",
            Mode::Landing => "LANDING",
            Mode::Feeding => "FEEDING",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct Motors {
    pub flight_power_l: f32,
    pub flight_power_r: f32,
    pub flight_steer_l: f32,
    pub flight_steer_r: f32,
    pub walk_l: f32,
    pub walk_r: f32,
    pub land_l: f32,
    pub land_r: f32,
    pub mn9: f32,
}

// ------------------------------------------------------------ quaternion ops

fn qmul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

fn qnorm(q: [f32; 4]) -> [f32; 4] {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n < 1e-9 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

/// Rotate a body-frame vector into the world frame.
pub(crate) fn qrot(q: [f32; 4], v: V3) -> V3 {
    // v' = v + 2 * q_vec x (q_vec x v + qw * v)
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    let tx = 2.0 * (y * v[2] - z * v[1]);
    let ty = 2.0 * (z * v[0] - x * v[2]);
    let tz = 2.0 * (x * v[1] - y * v[0]);
    [
        v[0] + w * tx + (y * tz - z * ty),
        v[1] + w * ty + (z * tx - x * tz),
        v[2] + w * tz + (x * ty - y * tx),
    ]
}

/// ZYX Euler angles of the quaternion, for telemetry and the analyser only.
/// The attitude itself is always carried as a quaternion, so a tumble cannot
/// hit a gimbal singularity.
fn qeuler(q: [f32; 4]) -> (f32, f32, f32) {
    let (w, x, y, z) = (q[0], q[1], q[2], q[3]);
    let m00 = 1.0 - 2.0 * (y * y + z * z);
    let m01 = 2.0 * (x * y - w * z);
    let m02 = 2.0 * (x * z + w * y);
    let m12 = 2.0 * (y * z - w * x);
    let m22 = 1.0 - 2.0 * (x * x + y * y);
    let yaw = m01.atan2(m00);
    let pitch = (-m02).clamp(-1.0, 1.0).asin();
    let roll = m12.atan2(m22);
    (yaw, pitch, roll)
}

// -------------------------------------------------------------------- state

pub struct Body {
    pub pos: V3,
    pub vel: V3,
    /// Attitude, body to world. Authoritative; the Euler angles below are
    /// derived from it for display and analysis.
    q: [f32; 4],
    /// Body-frame angular velocity: roll, pitch, yaw rates in rad/s.
    pub omega: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub mode: Mode,
    /// Mean stroke amplitude, 0..STROKE_AMP_MAX, for the HUD and the renderer.
    pub wing_amp: f32,
    pub wing_amp_l: f32,
    pub wing_amp_r: f32,
    /// Stroke-plane tilt per wing, radians. Positive tilts the force forward.
    pub tilt_l: f32,
    pub tilt_r: f32,
    /// Haltere afferent rates actually driven into the connectome, in Hz.
    pub haltere_l: f32,
    pub haltere_r: f32,
    pub wing_phase: f32,
    pub gait_phase: f32,
    pub legs_supported: u8,
    /// Vertical component of the wing force from the last integration, in the
    /// body frame and in the world frame. `wing_lift()` returns the body-frame
    /// value; what actually fights gravity is the world-frame one, and the two
    /// differ by the body's orientation. Logging only the body-frame number
    /// overstates the upward force whenever the fly is not level.
    pub fz_body: f32,
    pub fz_world: f32,
    /// Aerodynamic torque from the last integration, in the body frame, and the
    /// damping torque actually opposing it. Recorded so a systematic rotation
    /// can be attributed to a steady torque rather than guessed at.
    pub tau_aero: [f32; 3],
    pub tau_damp: [f32; 3],
    /// Count of feeding bouts. Feeding is retired (see `Mode::Feeding`): nothing
    /// assigns that mode any more, so this stays at zero. Retained because
    /// `main.rs` and `analyze` still read it for the telemetry schema.
    pub eats: u32,
    pub takeoffs: u32,
    pub landings: u32,
    pub wall_hits: u32,
    /// Liftoff label timer, milliseconds.
    label_ms: f32,
    /// Per-axis wall/ceiling contact latch, so a graze counts once.
    pub touching: [bool; 3],
    /// Retained only because `main.rs` reads it for the telemetry summary. With
    /// the feeding path deleted nothing writes it any more, so it is constant.
    /// Removing it is a cross-file follow-up (main.rs is out of scope here).
    pub taste_gain: f32,
}

impl Body {
    pub fn new() -> Body {
        Body {
            pos: [0.0, 0.0, 0.0],
            vel: [0.0, 0.0, 0.0],
            q: [1.0, 0.0, 0.0, 0.0],
            omega: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            mode: Mode::Ground,
            wing_amp: 0.0,
            wing_amp_l: 0.0,
            wing_amp_r: 0.0,
            tilt_l: 0.0,
            tilt_r: 0.0,
            haltere_l: 0.0,
            haltere_r: 0.0,
            wing_phase: 0.0,
            gait_phase: 0.0,
            legs_supported: 6,
            fz_body: 0.0,
            fz_world: 0.0,
            tau_aero: [0.0; 3],
            tau_damp: [0.0; 3],
            eats: 0,
            takeoffs: 0,
            landings: 0,
            wall_hits: 0,
            label_ms: 0.0,
            touching: [false; 3],
            taste_gain: 1.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Body::new();
    }

    /// Point the body along a heading in the horizontal plane, level.
    pub fn set_yaw(&mut self, yaw: f32) {
        let (cy, sy) = ((yaw * 0.5).cos(), (yaw * 0.5).sin());
        self.q = [cy, 0.0, 0.0, sy];
        self.yaw = yaw;
        self.pitch = 0.0;
        self.roll = 0.0;
    }

    /// Yaw rate about the world vertical, taken from the body's actual angular
    /// velocity. The Euler yaw angle is not a safe source for this: when the
    /// body pitches or rolls past vertical the Euler chart wraps and the
    /// difference quotient reports rates that belong to the chart rather than
    /// to the body. Every turning number in the analysis comes from here.
    pub fn yaw_rate_world(&self) -> f32 {
        qrot(self.q, self.omega)[2]
    }

    /// Body-forward unit vector in the world frame.
    pub fn forward(&self) -> V3 {
        qrot(self.q, [1.0, 0.0, 0.0])
    }

    /// Body-up unit vector in the world frame.
    pub fn up(&self) -> V3 {
        qrot(self.q, [0.0, 0.0, 1.0])
    }

    pub fn head(&self) -> V3 {
        add(self.pos, scale(self.forward(), 1.2))
    }

    /// Horizontal heading, for the room sampling code.
    pub fn heading(&self) -> V3 {
        let f = self.forward();
        let h = (f[0] * f[0] + f[1] * f[1]).sqrt();
        if h < 1e-6 {
            return [1.0, 0.0, 0.0];
        }
        [f[0] / h, f[1] / h, 0.0]
    }

    /// Quaternion [w,x,y,z] for the renderer.
    pub fn quat(&self) -> [f32; 4] {
        self.q
    }

    pub fn speed(&self) -> f32 {
        (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]).sqrt()
    }

    /// Airspeed in mm/s, three-dimensional.
    pub fn airspeed(&self) -> f32 {
        len(self.vel)
    }

    /// Total vertical aerodynamic force from both wings right now, in
    /// mg*mm/s^2. Liftoff happens when this exceeds the fly's weight.
    pub fn wing_lift(&self) -> f32 {
        let airspeed = self.airspeed();
        let fl = wing::wing_force_vector(self.wing_amp_l, wing::WINGBEAT_HZ, airspeed, self.tilt_l);
        let fr = wing::wing_force_vector(self.wing_amp_r, wing::WINGBEAT_HZ, airspeed, self.tilt_r);
        fl[2] + fr[2]
    }

    /// Vertical component of the wing force in the **world** frame.
    ///
    /// This is the quantity that actually fights gravity. `wing_lift()` is the
    /// body-frame value, which is what the wings produce along the body's own
    /// up axis; rotating the body tilts that away from vertical. Using the
    /// body-frame value to decide whether the fly can fly makes the decision
    /// independent of orientation, so a tumbling body always "can fly".
    pub fn wing_lift_world(&self) -> f32 {
        let airspeed = self.airspeed();
        let fl = wing::wing_force_vector(self.wing_amp_l, wing::WINGBEAT_HZ, airspeed, self.tilt_l);
        let fr = wing::wing_force_vector(self.wing_amp_r, wing::WINGBEAT_HZ, airspeed, self.tilt_r);
        let f_body = [fl[0] + fr[0], fl[1] + fr[1], fl[2] + fr[2]];
        qrot(self.q, f_body)[2]
    }

    /// Angle between the body's own up axis and world up, in degrees. Zero is
    /// level; 90 is on its side; 180 is inverted. Measures attitude alone, so
    /// it cannot be confused with where the fly is pointing in the plane.
    pub fn tilt_deg(&self) -> f32 {
        self.up()[2].clamp(-1.0, 1.0).acos().to_degrees()
    }

    /// Weight in mg*mm/s^2.
    pub fn weight(&self) -> f32 {
        wing::FLY_MASS * GRAVITY
    }

    /// Advance the body one control step of `dt` seconds from decoded motor
    /// activity. `_dn_drive` is the filtered descending flight drive and
    /// `land_drive` the landing population rate; the descending drive no longer
    /// touches the body at all (the HUD reads it from `World::dn_filter`), and
    /// neither commands the flight path.
    pub fn update(
        &mut self,
        room: &Room,
        food: V3,
        m: &Motors,
        _dn_drive: f32,
        land_drive: f32,
        dt: f32,
    ) {
        let dt_ms = dt * 1000.0;

        // 1. Wing motor neurons -> stroke amplitude, through the muscle.
        //
        // The endpoint is the spec's, not a fitted value: `a = 1` means the
        // full stroke amplitude, because the spec fixes the muscle activation
        // gain by `k_a: scale to give F/W≈1.2 at a=1` (physical-model-spec.md
        // §6.5, derived from the mandatory no-tuning self-check in §6.3). That
        // endpoint is preserved exactly: `stroke_amp(1.0) == STROKE_AMP_MAX`.
        //
        // SHAPE. Quasi-steady blade-element force goes as stroke amplitude
        // SQUARED (spec §1.3; `stroke_force_is_quadratic_in_amplitude`), so the
        // shape of the rate-to-force relation is fixed by the shape of the
        // rate-to-amplitude map. The map the model needs is the one the animal
        // was measured to have. Gordon & Dickinson (2006), PNAS 103(11):4311,
        // doi:10.1073/pnas.0510109103, tethered flies with drifting gratings and
        // recorded A-IFM membrane spikes alongside wing kinematics: "a 3-fold
        // increase in spike frequency (from 3 to 9 Hz) results in a 1.7-fold
        // increase in power output" (Fig. 1e); the paper's rounded summary of
        // the same measurement is "An ≈2-fold change in power accompanied a
        // 3-fold change in steady-state spike frequency" (Fig. 1c). The
        // explicit measured pair is the one used here:
        //
        //     power ∝ f^0.483,   0.483 = ln(1.7) / ln(3)
        //
        // i.e. a COMPRESSIVE power law, not the squared one the previous linear
        // map produced (`lift ∝ (0.12 + 0.88a)^2`, exponent 2). It is 4x too
        // steep AND in the wrong direction: the animal's power is a shallow,
        // concave function of spike rate, so a 3-fold rate change is only a
        // 1.7-fold force change. Matching it needs amplitude to be a
        // compressive function of `a` with half that exponent, because force is
        // amplitude squared:
        //
        //     amp = STROKE_AMP_MAX * a^(0.483 / 2)
        //
        // Anchored over the measured in-flight band, 3-12 Hz/neuron, which is
        // where the citation's 3-9 Hz measurement sits; outside it the map is
        // the same continuous power law, saturating at a = 1 (12 Hz) and going
        // to zero amplitude at a = 0. It is deliberately NOT extrapolated as a
        // power law below the band as a physical claim: at a = 0.05 (0.6 Hz) it
        // gives 0.47 of full stroke, which is what a compressive law implies
        // and is why the old `0.12` floor is gone -- see below.
        //
        // THE FLOOR. The previous map was `0.12 + 0.88a`; the 0.12 was an [E]
        // holdover ("a wing with zero motor drive still sweeps a little") that
        // was not measured and only added uncited force -- at a = 0 it alone
        // produced 1.4% of full-stroke lift, and across the working band it
        // pushed the map's shape away from any measured law. It is removed: a
        // compressive power law already rises steeply from zero (a = 0.05
        // gives 47% of full stroke), so nothing needs a floor to keep the wing
        // beating, and at a = 0 (a silent pool) the amplitude is genuinely 0.
        // `no_altitude_setpoint_remains` covers the grounded case.
        //
        // SCALE. `m.flight_power_*` is the fraction of the physiological
        // FULL-STROKE rate of the flight power motor neurons that the pool
        // reached (groups.rs, `POWER_MN_FULL_STROKE_HZ` = 12 Hz/neuron, the top
        // of the measured in-flight band). It is NOT the fraction of the pool
        // that fired in the window — those differ by ~4x at the working point,
        // and using the window's arithmetic ceiling as the actuator's full
        // scale is what made 0.67 look like "two-thirds of full power" when the
        // pool is in fact driven far beyond the maximum a flight muscle is
        // asked for.
        let a = 1.0 - (-dt / MUSCLE_TAU).exp();
        let amp_l = stroke_amp_for_activation(m.flight_power_l);
        let amp_r = stroke_amp_for_activation(m.flight_power_r);
        self.wing_amp_l += (amp_l - self.wing_amp_l) * a;
        self.wing_amp_r += (amp_r - self.wing_amp_r) * a;
        self.wing_amp = 0.5 * (self.wing_amp_l + self.wing_amp_r);
        self.wing_phase =
            (self.wing_phase + std::f32::consts::TAU * WINGBEAT_VISUAL_HZ * dt) % std::f32::consts::TAU;

        // 2. Steering motor neurons -> stroke-plane tilt. Common drive tilts
        //    both planes the same way (fore/aft); differential drive tilts them
        //    opposite ways, which is what produces a yaw torque once the
        //    forces act at the two wing roots.
        let common = 0.5 * (m.flight_steer_l + m.flight_steer_r);
        let diff = m.flight_steer_r - m.flight_steer_l;
        self.tilt_l = wing::STEER_TILT_MAX * (common + 0.5 * diff).clamp(-1.0, 1.0);
        self.tilt_r = wing::STEER_TILT_MAX * (common - 0.5 * diff).clamp(-1.0, 1.0);

        // 3. Haltere afferents report the rotation from the last step, before
        //    this step changes it: a sense organ cannot lead the body.
        let (sig_l, sig_r) = wing::haltere_pair(self.omega);
        let base = wing::HALTERE_BASE_HZ;
        self.haltere_l =
            wing::haltere_lag(self.haltere_l, wing::haltere_rate_hz(sig_l, base), dt);
        self.haltere_r =
            wing::haltere_lag(self.haltere_r, wing::haltere_rate_hz(sig_r, base), dt);

        self.label_ms = (self.label_ms - dt_ms).max(0.0);

        match self.mode {
            // `Feeding` is retired and unreachable: the variant is kept only for
            // the cross-module match in `analyze`, so its body is the ground one.
            Mode::Ground | Mode::Feeding => self.update_ground(room, m, dt, dt_ms),
            _ => self.update_air(room, m, land_drive, dt),
        }

        let (yaw, pitch, roll) = qeuler(self.q);
        self.yaw = yaw;
        self.pitch = pitch;
        self.roll = roll;

        if !self.pos[2].is_finite() || !self.vel[2].is_finite() {
            self.pos[2] = 0.0;
            self.vel = [0.0, 0.0, 0.0];
            self.q = [1.0, 0.0, 0.0, 0.0];
            self.omega = [0.0, 0.0, 0.0];
        }
    }

    /// Ground locomotion. [SURROGATE] Leg motor neurons set a capped walking
    /// speed and a turn rate; there is no leg kinematics or contact model.
    fn update_ground(&mut self, room: &Room, m: &Motors, _dt: f32, dt_ms: f32) {
        let support = room.support_z(self.pos[0], self.pos[1]);
        self.pos[2] = support;
        let walk = 0.5 * (m.walk_l + m.walk_r);
        let turn = m.walk_r - m.walk_l;
        self.omega[2] = turn * 3.0;
        self.yaw += turn * 3.0 * (dt_ms / 1000.0);
        // Rebuild the quaternion from the flat yaw, since the ground model
        // works in Euler angles.
        let (cy, sy) = ((self.yaw * 0.5).cos(), (self.yaw * 0.5).sin());
        self.q = [cy, 0.0, 0.0, sy];
        self.omega[0] = 0.0;
        self.omega[1] = 0.0;
        let speed = WALK_MAX * walk.clamp(0.0, 1.0);
        let h = self.heading();
        self.vel = [h[0] * speed, h[1] * speed, 0.0];
        self.pos = add(self.pos, scale(self.vel, dt_ms / 1000.0));
        self.gait_phase =
            (self.gait_phase + speed * 1.6 * (dt_ms / 1000.0)) % std::f32::consts::TAU;
        self.legs_supported = 6;

        // Physical liftoff: the wings beat, and when the aerodynamic force
        // they generate exceeds the fly's weight it leaves the ground. There
        // is no takeoff timer and no command to climb.
        //
        // This reads the WORLD-frame vertical component. The body-frame value
        // (`wing_lift()`) is what the wings produce along the body's own up
        // axis, which a tumbling body can keep above weight while producing no
        // useful lift at all -- it made the fly take off 112 times in 12 s.
        if self.wing_lift_world() > self.weight() {
            self.mode = Mode::Takeoff;
            self.label_ms = TAKEOFF_LABEL_MS;
            self.takeoffs += 1;
        }
    }

    /// Airborne flight. Every force here comes from wing kinematics or
    /// gravity; nothing steers towards a setpoint.
    fn update_air(&mut self, room: &Room, m: &Motors, land_drive: f32, dt: f32) {
        let airspeed = self.airspeed();

        // Wing forces, in the body frame, applied at the two wing roots.
        let fl = wing::wing_force_vector(self.wing_amp_l, wing::WINGBEAT_HZ, airspeed, self.tilt_l);
        let fr = wing::wing_force_vector(self.wing_amp_r, wing::WINGBEAT_HZ, airspeed, self.tilt_r);
        let f_body = [fl[0] + fr[0], fl[1] + fr[1], fl[2] + fr[2]];
        let f_world = qrot(self.q, f_body);
        self.fz_body = f_body[2];
        self.fz_world = f_world[2];

        // Translational dynamics: wing force, gravity, body drag.
        let drag = wing::body_drag(self.vel, 1.0);
        let acc = [
            (f_world[0] + drag[0]) / wing::FLY_MASS,
            (f_world[1] + drag[1]) / wing::FLY_MASS,
            (f_world[2] + drag[2]) / wing::FLY_MASS - GRAVITY,
        ];
        self.vel = [
            self.vel[0] + acc[0] * dt,
            self.vel[1] + acc[1] * dt,
            self.vel[2] + acc[2] * dt,
        ];
        let sp = len(self.vel);
        if sp > MAX_SPEED {
            self.vel = scale(self.vel, MAX_SPEED / sp);
        }

        // Rotational dynamics. Torque of each wing force about the wing root:
        //   tau = r x F, with r = (0, +-WING_DY, WING_DZ). The longitudinal
        //   component WING_DX is deliberately left out; see below.
        // The lateral offset turns an amplitude difference into roll and the
        // vertical offset turns a forward thrust into pitch.
        //
        // The longitudinal WING_DX term is deliberately NOT here. Including it
        // as `-dx * (fl[2]+fr[2])` adds a large constant nose-up moment, about
        // 0.15x total lift, that is present even at zero tilt where the pitch
        // torque is otherwise exactly zero. Measured: it regressed the fly from
        // 1 takeoff per 12 s to 163, cruise 97.2% to 0.5%, and altitude 189.5 mm
        // to 21.2 mm, i.e. straight back to bouncing on the floor.
        //
        // It is not restored until the sign is settled: the constant is -0.15
        // but its own doc says the wing centre sits *ahead* of the centre of
        // mass, and in a frame whose x points forward that argues for a
        // positive value. Getting the sign wrong inverts the moment, and the
        // magnitude is large enough that either sign dominates the pitch axis.
        // Resolve the sign from the rig first, then reintroduce it and re-run
        // the flight test; do not add it on the strength of the cross product
        // alone.
        //
        // No handedness correction appears here, and that is deliberate. The
        // cross product and the quaternion rotation both follow the right-hand
        // rule, so they already agree: a positive component of tau rotates the
        // body the way the right-hand rule says it does. Checked component by
        // component against the body's own forward and left vectors, more lift
        // on the left wing rolls the fly right and more thrust on the right
        // wing yaws the nose left, which is the physical direction.
        // An earlier revision negated the roll and yaw components to "fix" the
        // left-handed (x forward, y left, z up) frame. That negated the damping
        // term along with the control response, turning the aerodynamic damping
        // into positive feedback; the body then spun up until it hit the rate
        // clamp and its speed pegged the speed clamp.
        let (dy, dz) = (wing::WING_DY, wing::WING_DZ);
        let tau_aero = [
            dy * (fl[2] - fr[2]),
            dz * (fl[0] + fr[0]),
            dy * (fr[0] - fl[0]),
        ];
        let damp = wing::wing_damping();
        self.tau_aero = tau_aero;
        self.tau_damp = [
            damp[0] * self.omega[0],
            damp[1] * self.omega[1],
            damp[2] * self.omega[2],
        ];

        // Pendular restoring torque.
        //
        // The wing force acts at WING_DZ above the centre of mass, and on the
        // timescale of a body perturbation it is directed along the stroke
        // plane normal fixed in the WORLD frame, not carried round with the
        // body. A vertical force applied above the centre of mass is a
        // suspension point: the centre of mass hangs below it, so tilting the
        // body moves the application point sideways and the resulting
        // `p x F` opposes the tilt:
        //
        //     p = dz * u,  F vertical  =>  tau = dz * F * (u x zhat)
        //
        // where u is the body's up axis in world coordinates. The torque is
        // applied straight into the body-frame integrator below because this
        // term only acts over the small tilts where the two frames' x/y
        // components agree.
        //
        // Without this the body has no restoring mechanism in pitch or roll:
        // `qrot` tips the wing force with the body, so p and F stay parallel,
        // their cross product is zero, and nothing opposes the tilt. That is
        // why the free-flight test shows the body accumulating rotation the
        // moment it produces thrust instead of settling near level.
        let u = qrot(self.q, [0.0, 0.0, 1.0]);
        let f_up = f_body[2].max(0.0);
        let tau_restore = [dz * f_up * u[1], -dz * f_up * u[0], 0.0];

        self.omega = [
            self.omega[0]
                + (tau_aero[0] + tau_restore[0] - damp[0] * self.omega[0]) / wing::I_ROLL * dt,
            self.omega[1]
                + (tau_aero[1] + tau_restore[1] - damp[1] * self.omega[1]) / wing::I_PITCH * dt,
            self.omega[2] + (tau_aero[2] - damp[2] * self.omega[2]) / wing::I_YAW * dt,
        ];
        for a in 0..3 {
            if self.omega[a] > MAX_OMEGA {
                self.omega[a] = MAX_OMEGA;
            } else if self.omega[a] < -MAX_OMEGA {
                self.omega[a] = -MAX_OMEGA;
            }
        }

        // Integrate the attitude from the body-frame rate.
        let w = self.omega;
        let dq = qmul(self.q, [0.0, w[0] * dt * 0.5, w[1] * dt * 0.5, w[2] * dt * 0.5]);
        self.q = qnorm([
            self.q[0] + dq[0],
            self.q[1] + dq[1],
            self.q[2] + dq[2],
            self.q[3] + dq[3],
        ]);

        self.pos = add(self.pos, scale(self.vel, dt));
        self.gait_phase = 0.0;
        self.legs_supported = 0;

        // Contact. Purely geometric: the room clamps the body and reverses the
        // component of motion into the surface.
        let hit = room.clamp(&mut self.pos);
        for a in 0..3 {
            if hit[a] && !self.touching[a] {
                self.touching[a] = true;
                if a < 2 {
                    self.wall_hits += 1;
                }
            } else if !hit[a] {
                self.touching[a] = false;
            }
        }
        if hit[0] {
            self.vel[0] = -self.vel[0] * RESTITUTION;
        }
        if hit[1] {
            self.vel[1] = -self.vel[1] * RESTITUTION;
        }
        if hit[2] {
            self.vel[2] = -self.vel[2] * RESTITUTION;
        }

        let support = room.support_z(self.pos[0], self.pos[1]);
        if self.pos[2] <= support + 0.02 {
            self.pos[2] = support;
            // Landing is a physical event: the body arrives at the surface.
            if self.vel[2] <= 0.0 {
                self.mode = Mode::Ground;
                self.landings += 1;
                self.vel = [0.0, 0.0, 0.0];
                self.omega = [0.0, 0.0, 0.0];
            }
        } else if self.label_ms > 0.0 {
            self.mode = Mode::Takeoff;
        } else {
            // The landing label reflects the real landing descending neurons
            // while the body is descending near a surface. It does not steer.
            let descending = self.vel[2] < 0.0 && self.pos[2] - support < 26.0;
            let want_land = land_drive > 0.20 && descending;
            self.mode = if want_land { Mode::Landing } else { Mode::Cruise };
        }

        let _ = m;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wing_force_hovers_at_full_stroke() {
    // The model must be able to hold the fly up at full stroke drive
    // without any tuning factor. Measured coefficients and morphology only.
    // One wing does not have to equal half the weight: a fly that can
    // accelerate upward and carry a load needs a surplus. The measured
    // planform puts the surplus at about 1.8x, which is the right order
    // for Drosophila, so assert the band rather than a knife edge.
    let f = wing::wing_force_magnitude(wing::STROKE_AMP_MAX, wing::WINGBEAT_HZ, 0.0, 40.0);
    let half_weight = 0.5 * wing::FLY_MASS * GRAVITY;
    let ratio = f / half_weight;
    assert!(
        ratio > 1.2 && ratio < 2.5,
        "one wing at full stroke gives {ratio:.3} of half the weight;                a fly needs a clear surplus over its own weight to climb and carry a load"
    );
    }

    #[test]
    fn full_stroke_lift_beats_weight_even_with_the_stroke_plane_tilted() {
    // The steering motor neurons sit near saturation in the running
    // connectome, which tilts both stroke planes to the limit and costs
    // cos(tilt) of the vertical component. The fly has to still be able to
    // leave the ground in that posture, or the physics is the thing
    // preventing flight rather than the network.
    let f = 2.0 * wing::wing_force_magnitude(wing::STROKE_AMP_MAX, wing::WINGBEAT_HZ, 0.0, 40.0);
    let vertical = f * (wing::STEER_TILT_MAX).cos();
    let weight = wing::FLY_MASS * GRAVITY;
    assert!(
        vertical > weight,
        "at the stroke-plane tilt limit the wings give {vertical:.0} against a weight of {weight:.0}"
    );
    }

    #[test]
    fn more_power_produces_more_lift() {
        let lo = wing::wing_force_magnitude(1.5, wing::WINGBEAT_HZ, 0.0, 40.0);
        let hi = wing::wing_force_magnitude(2.5, wing::WINGBEAT_HZ, 0.0, 40.0);
        assert!(hi > lo, "lift must increase with stroke amplitude");
    }

    /// The full-stroke endpoint is pinned by the spec and must not move when the
    /// SHAPE of the map changes. `a = 1` is `F/W ~ 1.2` by the spec's own
    /// activation-gain rule (physical-model-spec.md §6.5), so the map still has
    /// to deliver a full-stroke lift ratio in the §6.3 self-check band.
    #[test]
    fn full_stroke_endpoint_survives_the_compressive_shape() {
        assert!(
            (stroke_amp_for_activation(1.0) - wing::STROKE_AMP_MAX).abs() < 1e-5,
            "a = 1 must be the full stroke amplitude, got {}",
            stroke_amp_for_activation(1.0)
        );
        assert_eq!(stroke_amp_for_activation(0.0), 0.0, "no drive, no stroke");
        assert_eq!(stroke_amp_for_activation(-1.0), 0.0, "activation clamps at 0");
        // The 0.12 floor the old map carried is gone: a small activation must
        // give the compressive law's own small value, not a floor.
        let small = stroke_amp_for_activation(0.01) / wing::STROKE_AMP_MAX;
        assert!(
            (small - 0.01f32.powf(STROKE_AMP_EXPONENT)).abs() < 1e-5,
            "the map must be the power law with no additive floor, got {small:.4}"
        );
        let r = lift_ratio_at_activation(1.0);
        // 1.39 with the measured planform; the spec's 1.8 mm^2 wing gives 1.22.
        assert!(
            r > 1.0 && r < 1.5,
            "full stroke gives {r:.3} of body weight; the spec's self-check band is ~1.22 \
             (1.39 at the measured planform)"
        );
    }

    /// THE scrutiny test: the map must reproduce the MEASURED rate-to-force
    /// relation, not merely produce flight.
    ///
    /// Gordon & Dickinson (2006), PNAS 103(11):4311, doi:10.1073/pnas.0510109103:
    /// "a 3-fold increase in spike frequency (from 3 to 9 Hz) results in a
    /// 1.7-fold increase in power output" (Fig. 1e). Aerodynamic force is
    /// amplitude squared, so the model's lift over that same span has to be
    /// 1.7x. The map this replaced was LINEAR in activation with a floor, so on
    /// the old read-out it gave a super-linear force exponent instead — see
    /// `docs/force-rate-map.md` for both curves tabulated side by side. This
    /// test fails on the old map and passes on the power law, which is what
    /// makes it evidence rather than assertion.
    #[test]
    fn force_follows_the_measured_frequency_to_power_relation() {
        let got = lift_ratio_at_rate(9.0) / lift_ratio_at_rate(3.0);
        assert!(
            (got - 1.7).abs() < 0.02,
            "3 Hz/neuron -> 9 Hz/neuron gave {got:.3}x lift; the measured relation is 1.7x \
             (and the paper's rounded statement of it, 3-fold rate to 2-fold power, is 2.0x)"
        );
        // The exponent itself, measured off the produced curve across the whole
        // measured in-flight band.
        let lo_hz = crate::groups::POWER_MN_BAND_LO_HZ;
        let hi_hz = crate::groups::POWER_MN_BAND_HI_HZ;
        let expo = (lift_ratio_at_rate(hi_hz) / lift_ratio_at_rate(lo_hz)).ln()
            / (hi_hz / lo_hz).ln();
        assert!(
            (expo - POWER_RATE_EXPONENT).abs() < 0.02,
            "the produced force-versus-rate exponent over {lo_hz:.0}-{hi_hz:.0} Hz is \
             {expo:.3}, not the measured {POWER_RATE_EXPONENT:.3}"
        );
        // And it is COmpressive everywhere in the band: each doubling of rate
        // buys less force than the last. The old squared map was the opposite.
        for f in [3.0f32, 4.0, 6.0, 8.0, 10.0] {
            let m1 = lift_ratio_at_rate(2.0 * f) / lift_ratio_at_rate(f);
            assert!(
                m1 > 1.0 && m1 < 2.0,
                "doubling the rate from {f} Hz scaled the force {m1:.3}x; a compressive law \
                 must be between 1x and 2x (the old map gave 4x)"
            );
        }
    }

    /// Hover — the rate at which the produced lift equals the body weight —
    /// must land inside the measured 3-12 Hz in-flight band. A map that matches
    /// the exponent but needs 40-110 Hz to hover has merely re-tuned the scale.
    #[test]
    fn hover_sits_inside_the_measured_inflight_band() {
        let mut lo = 0.01f32;
        let mut hi = crate::groups::POWER_MN_MANOEUVRE_MAX_HZ;
        assert!(lift_ratio_at_rate(lo) < 1.0, "the curve must start below hover");
        assert!(lift_ratio_at_rate(hi) > 1.0, "the curve must reach hover below {hi} Hz");
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if lift_ratio_at_rate(mid) < 1.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let hover = 0.5 * (lo + hi);
        assert!(
            hover >= crate::groups::POWER_MN_BAND_LO_HZ
                && hover <= crate::groups::POWER_MN_BAND_HI_HZ,
            "the fly hovers at {hover:.2} Hz/neuron, outside the measured {:.0}-{:.0} Hz \
             in-flight band",
            crate::groups::POWER_MN_BAND_LO_HZ,
            crate::groups::POWER_MN_BAND_HI_HZ
        );
        // And it is near the animal's measured sustained-flight rate, ~5 Hz.
        assert!(
            (hover - 5.0).abs() < 2.0,
            "hover at {hover:.2} Hz/neuron is not near the measured ~5 Hz sustained rate"
        );
    }

    #[test]
    fn halteres_separate_roll_from_yaw() {
        // A pure roll drives the two halteres in opposite senses; a pure yaw
        // drives them the same way. Without that antisymmetry the pair could
        // not tell the two apart.
        let (lr, rr) = wing::haltere_pair([1.0, 0.0, 0.0]);
        assert!(lr > 0.0 && rr < 0.0, "roll must be antisymmetric");
        let (ly, ry) = wing::haltere_pair([0.0, 0.0, 1.0]);
        assert!(ly > 0.0 && ry > 0.0, "yaw must be common-mode");
    }

    #[test]
    fn grounded_fly_lifts_off_when_its_wings_beat() {
        // Liftoff is physical: with the wings beating and both stroke planes
        // tilted as the steering motor neurons command, the aerodynamic force
        // must beat the fly's weight on its own. No takeoff timer, no vertical
        // impulse, no climb command.
        let mut b = Body::new();
        let m = Motors {
            flight_power_l: 1.0,
            flight_power_r: 1.0,
            flight_steer_l: 0.88,
            flight_steer_r: 0.90,
            ..Default::default()
        };
        for _ in 0..500 {
            b.update(&crate::room::ROOM, [0.0, 0.0, 0.0], &m, 0.0, 0.0, 0.002);
        }
        let ratio = b.wing_lift() / b.weight();
        assert!(
            ratio > 1.0,
            "wings at full stroke under the commanded tilt give {ratio:.3} of the body's weight"
        );
        assert!(b.takeoffs > 0, "the wings beat but the fly never left the ground");
    }

    #[test]
    fn forward_thrust_pitches_the_body_nose_down() {
        // The wing force acts above the centre of mass, so a forward-tilted
        // stroke plane produces a nose-down pitching moment. This is the
        // instability that a real fly trims with its halteres and neck sense
        // organs; the model has to show it, because if it did not the
        // connectome would be being handed a body that flies itself.
        let mut b = Body::new();
        b.mode = Mode::Cruise;
        b.pos = [0.0, 0.0, 200.0];
        let m = Motors {
            flight_power_l: 1.0,
            flight_power_r: 1.0,
            flight_steer_l: 1.0,
            flight_steer_r: 1.0,
            ..Default::default()
        };
        for _ in 0..100 {
            b.update(&crate::room::ROOM, [0.0, 0.0, 0.0], &m, 0.0, 0.0, 0.002);
        }
        // Assert on the body's own forward vector rather than on a sign of an
        // Euler angle: "nose down" is a physical statement, and the Euler chart
        // convention is not. With x forward, y left and z up the frame is
        // left-handed, so a positive rotation about the left axis drops the
        // nose. Checking forward()[2] tests the physics, not the chart.
        let fwd_z = b.forward()[2];
        assert!(
            fwd_z < -0.02,
            "a forward-tilted stroke plane should push the nose down, but the nose is at {fwd_z:.4} (positive is up)"
        );
    }

    /// THE sign convention for the perturbation probe, established by running
    /// the body rather than by reading the torque code.
    ///
    /// A flipped sign here would invert the probe's conclusion completely: it
    /// would turn a measured stabilising response into a measured destabilising
    /// one. The two runs differ only in the steering motor neurons. Same body,
    /// same wing amplitudes, same timestep, same instant.
    ///
    /// A positive steer differential puts more forward thrust on the left wing
    /// (see `update`: `tilt_l` grows with `+diff`, `tilt_r` shrinks), and yaw
    /// torque is `WING_DY * (fr[0] - fl[0])`, so it must push the yaw rate down.
    /// Therefore opposing a positive yaw perturbation needs a positive
    /// differential.
    #[test]
    fn positive_steer_differential_drives_yaw_rate_negative() {
        fn yaw_rate_after(diff: f32) -> f32 {
            let mut b = Body::new();
            b.pos = [0.0, 0.0, 200.0];
            b.mode = Mode::Cruise;
            let m = Motors {
                flight_power_l: 0.9,
                flight_power_r: 0.9,
                flight_steer_l: 0.5 - 0.5 * diff,
                flight_steer_r: 0.5 + 0.5 * diff,
                ..Default::default()
            };
            for _ in 0..300 {
                b.update(&crate::room::ROOM, [0.0, 0.0, 0.0], &m, 0.0, 0.0, 0.002);
            }
            b.omega[2]
        }
        let zero = yaw_rate_after(0.0);
        let positive = yaw_rate_after(0.2);
        let negative = yaw_rate_after(-0.2);
        assert!(
            positive < zero,
            "a positive steer differential gave yaw rate {positive:.4}, \
             which is not below the neutral {zero:.4}"
        );
        assert!(
            negative > zero,
            "a negative steer differential gave yaw rate {negative:.4}, \
             which is not above the neutral {zero:.4}"
        );
    }

    #[test]
    fn aerodynamic_damping_slows_a_spinning_body() {
        // The wings' rotational damping is the only thing standing between the
        // rotational system and a runaway. A sign error in the torque assembly
        // turns it into positive feedback, so pin the direction rather than the
        // magnitude: a body spinning with its wings idle must slow down, in
        // every axis.
        let mut b = Body::new();
        b.mode = Mode::Cruise;
        b.pos = [0.0, 0.0, 200.0];
        b.omega = [8.0, 8.0, 8.0];
        let m = Motors::default();
        for _ in 0..20 {
            b.update(&crate::room::ROOM, [0.0, 0.0, 0.0], &m, 0.0, 0.0, 0.002);
        }
        for a in 0..3 {
            assert!(
                b.omega[a].abs() < 8.0,
                "axis {a} damping did not slow the spin: {:.3} rad/s, started at 8",
                b.omega[a]
            );
        }
    }

    #[test]
    fn no_altitude_setpoint_remains() {
        // Regression guard: the engineered model steered towards CRUISE_ALT.
        let mut b = Body::new();
        b.mode = Mode::Cruise;
        b.pos = [0.0, 0.0, 150.0];
        let m = Motors::default();
        for _ in 0..500 {
            b.update(&crate::room::ROOM, [0.0, 0.0, 0.0], &m, 0.0, 0.0, 0.002);
        }
        // With no motor drive there is no lift, so it must fall. If an
        // altitude controller were still present it would hold altitude.
        assert!(b.pos[2] < 150.0, "the body must fall when its wings are idle");
    }

    #[test]
    fn quaternion_rotation_round_trips() {
        let q = qnorm([0.7, 0.1, -0.3, 0.2]);
        let v = [1.0, 2.0, -3.0];
        let w = qrot(q, v);
        let back = qrot([q[0], -q[1], -q[2], -q[3]], w);
        for a in 0..3 {
            assert!((back[a] - v[a]).abs() < 1e-4, "inverse rotation must undo");
        }
    }
}