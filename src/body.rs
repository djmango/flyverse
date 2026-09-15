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
        // The map is the spec's activation set point, not a fitted curve: the
        // spec defines the activation a (a linear interpolation between the
        // resting and full stroke amplitude) and then fixes its endpoint —
        // `muscle activation gain k_a: scale to give F/W≈1.2 at a=1`
        // (physical-model-spec.md §6.5, [D] from §6.3) — so a = 1 means the
        // full stroke amplitude and a = 0 the resting floor.
        //
        // SHAPE: linear in `a` between a floor and the full-stroke ceiling is
        // what the in-vivo activation data support, so it is left alone. The
        // steeply sigmoidal part of the activation relation belongs to the
        // calcium/cross-bridge stage: in skinned IFM fibres positive power
        // starts at pCa 5.8 and reaches its maximum at pCa 5.25 (Wang, Zhao &
        // Swank 2011, Biophys. J. 101:2207, doi:10.1016/j.bpj.2011.09.034), and
        // the flight working range (pCa 5.4-5.7) sits on that steep flank. In
        // vivo, over that range, intramuscular calcium and muscle power are
        // *linear* (R^2 ~ 0.95, 20-120 W/kg; Lehmann, Skandalis & Berthe 2013,
        // doi:10.1098/rsif.2012.1050), and stroke amplitude rises approximately
        // linearly with drive before saturating at the measured ~160 deg
        // (Namiki et al. 2022, Curr. Biol. 32:1189, doi:10.1016/j.cub.2022.01.008).
        // MAX is that ~160 deg. The 0.12 floor is an [E] holdover that only
        // adds force, and is left documented rather than fitted.
        //
        // SCALE: `m.flight_power_*` is the fraction of the physiological
        // maximum rate of the flight power motor neurons that the pool reached
        // (groups.rs, `POWER_MN_MAX_HZ` = 20 Hz/neuron). It is NOT the fraction
        // of the pool that fired in the window — those differ by 25x, and using
        // the window's arithmetic ceiling as the actuator's full scale is what
        // made 0.67 look like "two-thirds of full power" when the pool is in
        // fact driven far beyond the maximum a flight muscle is asked for.
        let a = 1.0 - (-dt / MUSCLE_TAU).exp();
        let span = wing::STROKE_AMP_MAX - wing::STROKE_AMP_MIN;
        let amp_l = wing::STROKE_AMP_MIN + span * m.flight_power_l.clamp(0.0, 1.0);
        let amp_r = wing::STROKE_AMP_MIN + span * m.flight_power_r.clamp(0.0, 1.0);
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