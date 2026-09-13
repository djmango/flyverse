//! The embodied fly: a bounded flight and walking surrogate driven by real
//! decoded motor-neuron activity from the running MaleCNS connectome.
//!
//! This is an engineered embodiment, not a recovered biological flight
//! circuit. The connectome supplies descending flight drive, wing-power motor
//! neurons, wing-steering motor neurons, leg motor neurons, landing motor
//! neurons and the feeding MN9. Everything between those rates and a rigid
//! body in a room -- wingbeat waveform, aerodynamics, attitude stabilisation,
//! altitude-target tracking, gait and collision clearance -- is an explicit
//! surrogate, because the public dataset contains no muscles, no VNC and no
//! donor body state.
//!
//! Units: millimetres, seconds. Gravity 9810 mm/s^2.

use crate::room::{add, len, scale, sub, Room, V3};

pub const GRAVITY: f32 = 9810.0;
/// Wing amplitude at which lift exactly cancels gravity.
pub const WING_AMP_HOVER: f32 = 0.35;
pub const LIFT_MAX: f32 = GRAVITY / (WING_AMP_HOVER * WING_AMP_HOVER);
pub const THRUST_MAX: f32 = 3000.0;
/// Quadratic drag coefficient, tuned so full wing amplitude gives ~350 mm/s.
pub const DRAG_Q: f32 = THRUST_MAX / (350.0 * 350.0);
pub const DRAG_L: f32 = 1.2;
pub const MAX_SPEED: f32 = 520.0;
pub const WALK_MAX: f32 = 14.0;
/// Real wingbeat is ~200 Hz, which aliases at display frame rates; the visual
/// phase advances at this rate so the stroke is visible.
pub const WINGBEAT_VISUAL_HZ: f32 = 19.0;
pub const CRUISE_ALT: f32 = 28.0;
pub const FLIGHT_ENVELOPE: [f32; 2] = [5.0, 208.0];
pub const ALT_GAIN: f32 = 55.0;
pub const YAW_GAIN: f32 = 7.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Mode {
    Ground,
    Takeoff,
    Cruise,
    Landing,
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

pub struct Body {
    pub pos: V3,
    pub vel: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub mode: Mode,
    pub wing_amp: f32,
    pub wing_phase: f32,
    pub gait_phase: f32,
    pub legs_supported: u8,
    pub takeoff_timer_ms: f32,
    pub land_timer_ms: f32,
    pub takeoff_drive: f32,
    pub alt_drive: f32,
    pub alt_target: f32,
    pub feed_timer_ms: f32,
    pub post_meal_ms: f32,
    pub eats: u32,
    pub takeoffs: u32,
    pub landings: u32,
    pub wall_hits: u32,
    /// Per-axis wall/ceiling contact latch, so a graze counts once.
    pub touching: [bool; 3],
    /// Satiety of the taste pathway, 0..1, decays over the refractory window.
    pub taste_gain: f32,
}

impl Body {
    pub fn new() -> Body {
        Body {
            pos: [0.0, 0.0, 0.0],
            vel: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            mode: Mode::Ground,
            wing_amp: 0.0,
            wing_phase: 0.0,
            gait_phase: 0.0,
            legs_supported: 6,
            takeoff_timer_ms: 0.0,
            land_timer_ms: 0.0,
            takeoff_drive: 0.0,
            alt_drive: 0.0,
            alt_target: CRUISE_ALT,
            feed_timer_ms: 0.0,
            post_meal_ms: 0.0,
            eats: 0,
            takeoffs: 0,
            landings: 0,
            wall_hits: 0,
            touching: [false; 3],
            taste_gain: 1.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Body::new();
    }

    pub fn head(&self) -> V3 {
        let h = self.heading();
        add(self.pos, scale(h, 1.2))
    }

    pub fn heading(&self) -> V3 {
        [self.yaw.cos(), self.yaw.sin(), 0.0]
    }

    /// Quaternion [w,x,y,z] for R = Rz(yaw) * Ry(pitch) * Rx(roll).
    pub fn quat(&self) -> [f32; 4] {
        let (cy, sy) = ((self.yaw * 0.5).cos(), (self.yaw * 0.5).sin());
        let (cp, sp) = ((self.pitch * 0.5).cos(), (self.pitch * 0.5).sin());
        let (cr, sr) = ((self.roll * 0.5).cos(), (self.roll * 0.5).sin());
        [
            cy * cp * cr + sy * sp * sr,
            cy * cp * sr - sy * sp * cr,
            cy * sp * cr + sy * cp * sr,
            sy * cp * cr - cy * sp * sr,
        ]
    }

    /// Advance the body one control step of `dt` seconds from decoded motor
    /// activity. `dn_drive` is the normalised filtered descending flight drive
    /// and `land_drive` the normalised landing population rate.
    pub fn update(
        &mut self,
        room: &Room,
        food: V3,
        m: &Motors,
        dn_drive: f32,
        land_drive: f32,
        dt: f32,
    ) {
        let dt_ms = dt * 1000.0;
        let power = 0.5 * (m.flight_power_l + m.flight_power_r);
        let steer = m.flight_steer_r - m.flight_steer_l;
        let wing_target = (0.12 + 0.95 * power).clamp(0.0, 1.08);
        // Wing amplitude follows decoded wing-power motor neurons with a fast
        // first-order lag (the muscle/thorax time constant is not in the data).
        self.wing_amp += (wing_target - self.wing_amp) * (1.0 - (-dt / 0.02).exp());
        self.wing_phase = (self.wing_phase + std::f32::consts::TAU * WINGBEAT_VISUAL_HZ * dt)
            % std::f32::consts::TAU;

        self.takeoff_drive = dn_drive;
        self.alt_drive = (dn_drive * 1.4 - 0.35).clamp(-1.0, 1.0);

        match self.mode {
            Mode::Ground => self.update_ground(room, m, dn_drive, land_drive, dt, dt_ms),
            Mode::Takeoff => self.update_air(room, m, steer, dn_drive, land_drive, dt, 1.6),
            Mode::Cruise => self.update_air(room, m, steer, dn_drive, land_drive, dt, 1.0),
            Mode::Landing => self.update_landing(room, m, steer, dn_drive, dt, dt_ms),
            Mode::Feeding => self.update_feeding(room, food, m, dt, dt_ms),
        }

        if !self.pos[2].is_finite() {
            self.pos[2] = 0.0;
            self.vel = [0.0, 0.0, 0.0];
        }
    }

    fn update_ground(
        &mut self,
        room: &Room,
        m: &Motors,
        dn_drive: f32,
        _land: f32,
        _dt: f32,
        dt_ms: f32,
    ) {
        let support = room.support_z(self.pos[0], self.pos[1]);
        self.pos[2] = support;

        let walk = 0.5 * (m.walk_l + m.walk_r);
        let turn = m.walk_r - m.walk_l;
        self.yaw += turn * 3.0 * (dt_ms / 1000.0);
        let speed = WALK_MAX * walk.clamp(0.0, 1.0);
        let h = self.heading();
        self.vel = [h[0] * speed, h[1] * speed, 0.0];
        self.pitch += (0.0 - self.pitch) * 0.1;
        self.roll += (0.0 - self.roll) * 0.1;
        self.gait_phase = (self.gait_phase + speed * 1.6 * (dt_ms / 1000.0)) % std::f32::consts::TAU;
        self.legs_supported = if walk > 0.02 { 6 } else { 6 };

        // Neural takeoff gate: filtered descending flight drive above threshold
        // for 150 ms, with enough wing power behind it.
        if dn_drive > 0.14 {
            self.takeoff_timer_ms += dt_ms;
        } else {
            self.takeoff_timer_ms = 0.0;
        }
        if self.takeoff_timer_ms >= 150.0 && self.wing_amp > 0.22 {
            self.mode = Mode::Takeoff;
            self.alt_target = 18.0;
            self.takeoff_timer_ms = 0.0;
            self.takeoffs += 1;
            self.vel[2] = 40.0;
        }
    }

    fn update_air(
        &mut self,
        room: &Room,
        m: &Motors,
        steer: f32,
        dn_drive: f32,
        land_drive: f32,
        dt: f32,
        lift_bias: f32,
    ) {
        // Yaw from decoded wing-steering motor neurons.
        self.yaw += steer * YAW_GAIN * dt;
        // Bank into the turn, nose follows climb intent.
        let roll_target = (-0.55 * steer).clamp(-0.7, 0.7);
        let power_asym = 0.5 * (m.flight_power_r - m.flight_power_l);
        let roll_target = (roll_target - 0.8 * power_asym).clamp(-0.8, 0.8);
        self.roll += (roll_target - self.roll) * (1.0 - (-dt / 0.08).exp());
        let alt_span = FLIGHT_ENVELOPE[1] - CRUISE_ALT;
        // The base altitude is fixed; the neural climb command offsets it. (If
        // this read back the previous target it would ratchet to the ceiling.)
        let neutral = if self.mode == Mode::Takeoff { 18.0 } else { CRUISE_ALT };
        let alt_cmd = (neutral + alt_span * self.alt_drive.max(0.0)).clamp(FLIGHT_ENVELOPE[0], FLIGHT_ENVELOPE[1]);
        self.alt_target = alt_cmd;
        let pitch_target = (0.030 * (alt_cmd - self.pos[2]) + 0.20 * self.alt_drive).clamp(-0.5, 0.6);
        self.pitch += (pitch_target - self.pitch) * (1.0 - (-dt / 0.10).exp());

        // The neural drive sets the wingbeat amplitude. A fast stabilising loop
        // — the fly's wingbeat-synchronous feedback, not modelled neuronally
        // here — can only trim that amplitude down. Full authority to descend,
        // none to exceed the commanded power, so the commanded altitude is both
        // reachable and stable.
        let alt_err = alt_cmd - self.pos[2];
        let stab = (0.5 + alt_err / 40.0 - 0.006 * self.vel[2]).clamp(0.0, 1.0);
        let amp = self.wing_amp * stab;
        let lift = LIFT_MAX * amp * amp * self.roll.cos() * self.pitch.cos() * lift_bias;
        let h = self.heading();
        let thrust = THRUST_MAX * self.wing_amp * h[0] * (1.0 - 0.35 * self.roll.abs());
        let thrust_y = THRUST_MAX * self.wing_amp * h[1] * (1.0 - 0.35 * self.roll.abs());
        let v = self.vel;
        let sp = len(v).max(1e-6);
        let drag = scale(v, -(DRAG_Q * sp + DRAG_L));

        self.vel = [
            v[0] + (thrust + drag[0]) * dt,
            v[1] + (thrust_y + drag[1]) * dt,
            v[2] + (lift - GRAVITY + drag[2]) * dt,
        ];
        let sp = len(self.vel);
        if sp > MAX_SPEED {
            self.vel = scale(self.vel, MAX_SPEED / sp);
        }
        self.pos = add(self.pos, scale(self.vel, dt));

        // Room and table collision, plus the support surface. Contacts are
        // latched so grazing a wall for a second counts once, not 500 times.
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
            self.vel[0] = -self.vel[0] * 0.15;
        }
        if hit[1] {
            self.vel[1] = -self.vel[1] * 0.15;
        }
        if hit[2] {
            if self.pos[2] <= room.z[0] + 0.01 {
                self.mode = Mode::Ground;
                self.pos[2] = room.support_z(self.pos[0], self.pos[1]);
                self.vel[2] = 0.0;
                self.landings += 1;
            }
            self.vel[2] = -self.vel[2] * 0.15;
        }
        let support = room.support_z(self.pos[0], self.pos[1]);
        if self.pos[2] < support {
            self.pos[2] = support;
            self.vel[2] = 0.0;
            self.mode = Mode::Ground;
            self.landings += 1;
        }

        // Landing decision: decoded landing motor neurons, close to a surface.
        if land_drive > 0.20 && self.pos[2] - support < 26.0 && self.mode == Mode::Cruise {
            self.land_timer_ms += dt * 1000.0;
            if self.land_timer_ms > 90.0 {
                self.mode = Mode::Landing;
                self.land_timer_ms = 0.0;
            }
        } else {
            self.land_timer_ms = 0.0;
        }
        if self.mode == Mode::Takeoff && self.pos[2] > 24.0 {
            self.mode = Mode::Cruise;
            self.alt_target = CRUISE_ALT;
        }
    }

    fn update_landing(
        &mut self,
        room: &Room,
        m: &Motors,
        steer: f32,
        _dn: f32,
        dt: f32,
        dt_ms: f32,
    ) {
        self.yaw += steer * YAW_GAIN * 0.5 * dt;
        self.roll += (0.0 - self.roll) * 0.1;
        let land = 0.5 * (m.land_l + m.land_r);
        let support = room.support_z(self.pos[0], self.pos[1]);
        let sink = 30.0 + 60.0 * land;
        self.vel[2] = -sink;
        let h = self.heading();
        self.vel[0] = h[0] * 40.0;
        self.vel[1] = h[1] * 40.0;
        self.pos = add(self.pos, scale(self.vel, dt));
        if self.pos[2] <= support + 0.05 {
            self.pos[2] = support;
            self.vel = [0.0, 0.0, 0.0];
            self.mode = Mode::Ground;
            self.landings += 1;
        }
        let _ = dt_ms;
    }

    fn update_feeding(&mut self, room: &Room, _food: V3, m: &Motors, _dt: f32, dt_ms: f32) {
        let support = room.support_z(self.pos[0], self.pos[1]);
        self.pos[2] = support;
        self.vel = [0.0, 0.0, 0.0];
        self.gait_phase = 0.0;
        self.legs_supported = 6;
        self.feed_timer_ms += dt_ms;
        // The engineered meal holds while MN9 keeps firing, then departs.
        if self.feed_timer_ms > 800.0 && m.mn9 < 0.04 {
            self.finish_meal();
        } else if self.feed_timer_ms > 3000.0 {
            self.finish_meal();
        }
    }

    fn finish_meal(&mut self) {
        self.mode = Mode::Ground;
        self.feed_timer_ms = 0.0;
        self.post_meal_ms = 1250.0;
        self.takeoff_timer_ms = 0.0;
        self.taste_gain = 0.0;
    }

    /// Called once per control window with the taste rate seen at the
    /// proboscis, to enter the engineered meal.
    pub fn maybe_feed(&mut self, taste_rate_hz: f32, mn9: f32, room: &Room, food: V3) {
        if self.mode != Mode::Ground {
            return;
        }
        let d = len(sub(self.head(), food));
        if d < 3.0 && taste_rate_hz > 1.0 && mn9 > 0.05 {
            self.mode = Mode::Feeding;
            self.eats += 1;
            self.feed_timer_ms = 0.0;
        }
        let _ = room;
    }

    pub fn tick_post_meal(&mut self, dt_ms: f32) {
        if self.post_meal_ms > 0.0 {
            self.post_meal_ms = (self.post_meal_ms - dt_ms).max(0.0);
        }
        if self.taste_gain < 1.0 {
            self.taste_gain = (self.taste_gain + dt_ms / 12000.0).min(1.0);
        }
    }

    pub fn speed(&self) -> f32 {
        (self.vel[0] * self.vel[0] + self.vel[1] * self.vel[1]).sqrt()
    }
}
