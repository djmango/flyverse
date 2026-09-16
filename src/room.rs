//! The virtual room, in millimetres, Z up, origin on the floor at room centre.
//!
//! Geometry follows the upstream MaleCNS habitat scale so the numbers are
//! comparable: a 600 x 440 x 220 mm arena with a table whose top is 40 mm
//! above the floor and a sugar cube resting on it.

pub type V3 = [f32; 3];

/// A fruit in the room: a solid sphere with a reflectance spectrum.
///
/// It is a scene object and nothing else. The optics ray-cast against it exactly
/// as they do against a wall, so it drives the photoreceptors of whatever
/// columns are pointed at it -- which is the whole point, because that is how a
/// real fruit reaches a real fly's brain. There is deliberately NO collision
/// with it, no taste, no reward and no signal injected anywhere: whether the
/// connectome does anything with the light from it is the question being asked,
/// and it has to be the connectome's answer.
#[derive(Clone, Copy)]
pub struct Fruit {
    /// Centre, mm, world frame.
    pub c: V3,
    /// Radius, mm.
    pub r: f32,
}

pub const ROOM: Room = Room {
    x: [-300.0, 300.0],
    y: [-220.0, 220.0],
    z: [0.0, 220.0],
    table: Table { x: [40.0, 280.0], y: [-160.0, 120.0], top: 40.0, thickness: 12.0 },
    fruit: Some(FRUIT),
    fruit_grey: false,
    hand: None,
    wind: [1.0, 0.0, 0.0],
    odor_amp: 1.0,
    odor_lambda: 95.0,
};

/// Where the fly starts, mm. Fixed in `World::new`, and the default aim point
/// of the hand's trajectory (§ the hand below): a slap is aimed at where the
/// fly is, and the fly's position when the hand is built is its spawn point.
pub const SPAWN: V3 = [-220.0, -150.0, 0.0];

// ---------------------------------------------------------------- the hand
//
// A hand in the room: a dark, flat, occluding body that the optics ray-cast
// against exactly like a wall, the table or the fruit.
//
// WHY THIS IS BUILT AS GEOMETRY AND NOT AS A SIGNAL
// An approaching hand is what the published whole-brain demos use as their
// headline looming stimulus, and an expanding dark edge is the canonical
// Drosophila escape stimulus. The one thing this model must NOT contain is a
// term that makes the fly flee: no startle gain, no avoidance gradient, no
// looming reflex, no steering coefficient. So the hand is not a stimulus
// injected into the network and not a loom scalar written into a drive. It is
// an ordinary scene object with a geometry, an albedo and a trajectory; the
// fly sees it through its own eyes, through the same optics and the same
// retinotopic loom channel that already exist, and whatever it does with that
// is the connectome's doing.

/// A hand: a flat palm, as an oriented box.
///
/// Geometry: the open palm, which is the part of a hand that reaches a fly, is
/// about 90 x 110 x 24 mm. It is represented by an oriented box with
/// half-extents along its own three axes (see `HAND_HALF` for the order), so
/// the object that approaches the fly is a flat slab whose face turns toward
/// the fly and whose silhouette grows -- an expanding dark edge, not a point
/// source and not a sphere.
///
/// `axes` is orthonormal: `[0]` is the palm's width across, `[1]` its length
/// (roughly vertical down the hand) and `[2]` the palm normal, which points
/// back along the approach direction, i.e. at the fly.
#[derive(Clone, Copy)]
pub struct Hand {
    /// Palm centre, mm, world frame.
    pub c: V3,
    /// Half-extents along `axes`, mm: [width 45, length 55, thickness 12].
    pub half: V3,
    /// Orthonormal palm frame (see the struct doc).
    pub axes: [V3; 3],
}

/// Default palm half-extents, mm: 90 mm across x 110 mm along x 24 mm thick.
pub const HAND_HALF: V3 = [45.0, 55.0, 12.0];

/// Default start distance: palm centre to the aim point, mm.
pub const HAND_DIST_MM: f32 = 300.0;

/// Default launch time of the slap, ms after the run starts.
pub const HAND_START_MS: f32 = 2000.0;

/// Default time from launch to contact, ms.
///
/// A human slap is fast: the hand covers the last few hundred millimetres in
/// on the order of 100-300 ms. 150 ms over 300 mm is 2.0 m/s, inside that
/// window, and the control window is 2 ms, so the expansion is sampled at
/// about 75 points -- the retina sees a genuinely time-varying stimulus rather
/// than a step. Every one of these is a `FLYVERSE_HAND_*` knob and the values
/// actually used are printed on the banner and written into the run's JSON.
pub const HAND_DUR_MS: f32 = 150.0;

/// Default approach direction, world frame: the hand comes down and from the
/// fly's left-rear corner, i.e. obliquely and from ONE side. A slap is not a
/// symmetric looming stimulus, and the asymmetry is what the lateral loom
/// channel could in principle use. Normalised before use.
pub const HAND_DIR: V3 = [0.35, 0.55, 0.76];

impl Hand {
    /// Is `p` inside the palm?
    pub fn contains(&self, p: V3) -> bool {
        let d = sub(p, self.c);
        for a in 0..3 {
            if dot(d, self.axes[a]).abs() > self.half[a] {
                return false;
            }
        }
        true
    }

    /// Distance from `p` to the palm's surface, mm: 0 inside it, otherwise the
    /// distance to the nearest face. Exact for an axis-aligned box in its own
    /// frame, which is what the palm is here.
    pub fn surface_dist(&self, p: V3) -> f32 {
        let d = sub(p, self.c);
        let q = [dot(d, self.axes[0]), dot(d, self.axes[1]), dot(d, self.axes[2])];
        let mut out = 0.0f32;
        for a in 0..3 {
            let over = q[a].abs() - self.half[a];
            if over > 0.0 {
                out += over * over;
            }
        }
        out.sqrt()
    }
}

/// A hand's trajectory: where it starts, where it is aimed, and when it moves.
///
/// The trajectory is a straight line in world space, at constant speed, from
/// `origin` to `target` between `start_s` and `start_s + dur_s`. Before the
/// launch time the palm sits at `origin`; after the contact time it stays at
/// `target`. `approaching == false` parks it at `origin` for the whole run,
/// which is the static-hand control: the same object, in the same place, with
/// the same albedo and the same occlusion, but no expansion. The two
/// configurations are therefore bit-identical up to the launch time, and any
/// difference after it is a response to the motion, not to the object.
#[derive(Clone, Copy)]
pub struct HandTraj {
    pub origin: V3,
    pub target: V3,
    pub start_s: f32,
    pub dur_s: f32,
    pub approaching: bool,
    pub half: V3,
    pub axes: [V3; 3],
    /// Whether the aim point has been re-taken to the fly's own position at the
    /// launch instant. `World::advance` does that once, at `start_s`, unless
    /// `FLYVERSE_HAND_AIM=spawn` asks for the fixed spawn point instead.
    pub aimed_at_fly: bool,
}

impl HandTraj {
    /// The palm's state at time `t` seconds into the run.
    pub fn hand_at(&self, t: f32) -> Hand {
        let c = if !self.approaching {
            self.origin
        } else {
            let u = ((t - self.start_s) / self.dur_s.max(1e-6)).clamp(0.0, 1.0);
            add(self.origin, scale(sub(self.target, self.origin), u))
        };
        Hand { c, half: self.half, axes: self.axes }
    }

    /// Does the palm move at all?
    pub fn motion(&self) -> &'static str {
        if self.approaching {
            "approaching"
        } else {
            "static"
        }
    }

    /// Speed of the palm along its trajectory, mm/s (0 when static).
    pub fn speed_mm_s(&self) -> f32 {
        if self.approaching {
            len(sub(self.target, self.origin)) / self.dur_s.max(1e-6)
        } else {
            0.0
        }
    }

    /// One-line description of the exact parameters, for the banner and the
    /// run's JSON. Everything a reader needs to reconstruct the stimulus.
    pub fn describe(&self) -> String {
        format!(
            "{}: palm {:.0} x {:.0} x {:.0} mm (oriented box, half [{:.0},{:.0},{:.0}]), \
             origin ({:.0},{:.0},{:.0}) -> contact ({:.0},{:.0},{:.0}) mm{}, start {:.0} ms, \
             duration {:.0} ms, speed {:.0} mm/s, approach dir [{:.2},{:.2},{:.2}]",
            self.motion(),
            self.half[0] * 2.0,
            self.half[1] * 2.0,
            self.half[2] * 2.0,
            self.half[0],
            self.half[1],
            self.half[2],
            self.origin[0],
            self.origin[1],
            self.origin[2],
            self.target[0],
            self.target[1],
            self.target[2],
            if self.aimed_at_fly {
                " (aim re-taken to the fly's position at launch)"
            } else {
                ""
            },
            self.start_s * 1000.0,
            self.dur_s * 1000.0,
            self.speed_mm_s(),
            self.axes[2][0],
            self.axes[2][1],
            self.axes[2][2],
        )
    }
}

/// How the hand behaves, read once at startup from `FLYVERSE_HAND`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandMode {
    /// No hand in the room: the *default*, so a run with no flags set is
    /// unchanged. `FLYVERSE_HAND` unset, `0` or `off`.
    Absent,
    /// The hand travels from its origin to the contact point.
    Approach,
    /// The hand is present and does not move: the static-hand control.
    Static,
}

pub fn hand_mode() -> HandMode {
    match std::env::var("FLYVERSE_HAND").ok().as_deref() {
        Some("1") | Some("approach") | Some("yes") | Some("on") => HandMode::Approach,
        Some("static") | Some("still") => HandMode::Static,
        _ => HandMode::Absent,
    }
}

fn env_f32(k: &str, d: f32) -> f32 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(d)
}

fn env_v3(k: &str, d: V3) -> V3 {
    std::env::var(k)
        .ok()
        .and_then(|v| {
            let p: Vec<f32> = v
                .split(|c| c == ',' || c == ' ')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<f32>().ok())
                .collect();
            if p.len() == 3 && p.iter().all(|x| x.is_finite()) {
                Some([p[0], p[1], p[2]])
            } else {
                None
            }
        })
        .unwrap_or(d)
}

/// Default aim point: where the fly is when the world is built.
pub fn hand_target() -> V3 {
    env_v3("FLYVERSE_HAND_TARGET", SPAWN)
}

/// Should the trajectory's aim point be re-taken to the fly's OWN position at
/// the launch instant? Default yes.
///
/// A slap is aimed at the fly, not at the corner of the room the fly happened
/// to start in, and the fly is not where it started: at 2 s the three seeds used
/// here are at the spawn, in mid-air near the far wall and on the far wall
/// respectively. Re-taking the aim once, at the launch instant, is the faithful
/// choice, and it is a single scalar-free stimulus decision -- where the hand
/// goes -- with no feedback from the fly's behaviour afterwards (the hand does
/// not chase). `FLYVERSE_HAND_AIM=spawn` keeps the fixed aim, which is how the
/// difference between "aimed where the fly was" and "aimed at the fly" can be
/// measured instead of assumed.
pub fn hand_aim_at_fly() -> bool {
    !matches!(std::env::var("FLYVERSE_HAND_AIM").ok().as_deref(), Some("spawn"))
}

/// The trajectory, or `None` when no hand is in the room.
///
/// Every parameter is a knob:
///
/// | env | default | meaning |
/// |---|---|---|
/// | `FLYVERSE_HAND` | unset (absent) | `approach` / `static` |
/// | `FLYVERSE_HAND_DIR` | `0.35,0.55,0.76` | approach direction, world frame |
/// | `FLYVERSE_HAND_DIST_MM` | 300 | origin distance from the aim point |
/// | `FLYVERSE_HAND_START_MS` | 2000 | launch time |
/// | `FLYVERSE_HAND_DUR_MS` | 150 | launch -> contact time |
/// | `FLYVERSE_HAND_TARGET` | the fly's spawn point | aim point |
/// | `FLYVERSE_HAND_HALF_MM` | `45,55,12` | palm half-extents |
/// | `FLYVERSE_HAND_AIM` | `fly` | `spawn` to keep the fixed aim point |
pub fn active_hand() -> Option<HandTraj> {
    let mode = hand_mode();
    if mode == HandMode::Absent {
        return None;
    }
    let dir = norm(env_v3("FLYVERSE_HAND_DIR", HAND_DIR));
    let dir = if len(dir) < 0.5 { norm(HAND_DIR) } else { dir };
    let target = hand_target();
    let d0 = env_f32("FLYVERSE_HAND_DIST_MM", HAND_DIST_MM).max(1.0);
    let half = env_v3("FLYVERSE_HAND_HALF_MM", HAND_HALF);
    let half = [
        half[0].abs().max(1.0),
        half[1].abs().max(1.0),
        half[2].abs().max(1.0),
    ];
    Some(HandTraj {
        origin: add(target, scale(dir, d0)),
        target,
        start_s: env_f32("FLYVERSE_HAND_START_MS", HAND_START_MS).max(0.0) / 1000.0,
        dur_s: env_f32("FLYVERSE_HAND_DUR_MS", HAND_DUR_MS).max(1.0) / 1000.0,
        approaching: mode == HandMode::Approach,
        half,
        axes: hand_axes(dir),
        aimed_at_fly: false,
    })
}

/// Orthonormal palm frame from the approach direction: width across the
/// approach, length along it, normal back at the fly.
fn hand_axes(dir: V3) -> [V3; 3] {
    let n = scale(dir, -1.0);
    let mut w = cross([0.0, 0.0, 1.0], n);
    if len(w) < 1e-3 {
        w = cross([1.0, 0.0, 0.0], n);
    }
    let w = norm(w);
    let l = norm(cross(n, w));
    [w, l, n]
}

/// The fruit's resting place: on the table, 60 mm diameter, in the far corner
/// from the sugar cube (`food_home` is (160, 40, 44)) so that the visual object
/// and the odour source are two different places in the room and the two routes
/// can be told apart.
pub const FRUIT: Fruit = Fruit { c: [240.0, -130.0, 40.0 + 25.0], r: 25.0 };

/// Is the fruit in the room? `FLYVERSE_NO_FRUIT=1` removes it, which is the
/// clear-room control: same room, same odour field, same sugar cube, nothing
/// coloured to look at.
pub fn fruit_on() -> bool {
    std::env::var("FLYVERSE_NO_FRUIT").is_err()
}

/// Is the fruit rendered spectrally flat instead of in colour?
/// `FLYVERSE_FRUIT_GREY=1` gives it the reflectance that has the SAME
/// luminance-channel (R1-R6/Rh1) catch as the real fruit, so the two differ
/// only in colour. This is the control that separates "the fly responded to the
/// fruit" from "the fly responded to colour": if the grey fruit produces the
/// same behaviour, the response was luminance, not colour.
pub fn fruit_grey() -> bool {
    std::env::var("FLYVERSE_FRUIT_GREY").is_ok()
}

/// Horizontal scale factor for the arena: `FLYVERSE_ROOM_SCALE`, default 1.0.
///
/// A fly in a small box wall-follows; in a large arena it flies straighter. This
/// knob is how the geometry half of that statement is tested: it multiplies the
/// two horizontal semi-extents and (unless `FLYVERSE_ROOM_HEIGHT` says
/// otherwise) the ceiling. Non-finite or non-positive values fall back to 1.0,
/// so a bad value behaves like the default rather than producing a degenerate
/// box.
pub fn room_scale() -> f32 {
    std::env::var("FLYVERSE_ROOM_SCALE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0)
}

/// Ceiling height in mm: `FLYVERSE_ROOM_HEIGHT`, or `None` for the scaled
/// 220 mm. Setting it independently of the scale is what separates the two
/// geometry questions: a wide floor with the shipped ceiling tests the
/// horizontal crowding alone, and a tall ceiling at the shipped footprint tests
/// whether the fly is banging its head (airborne altitude p95 is 187 mm and max
/// 220 mm, i.e. the ceiling). A large value is a floorless run in effect.
pub fn room_height_mm() -> Option<f32> {
    std::env::var("FLYVERSE_ROOM_HEIGHT")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// The arena the world is actually built in.
///
/// Default: the shipped `ROOM`, returned by identity so a normal run is
/// bit-identical (`ROOM` is `Copy`).
///
/// Only the six walls move. The table, the sugar cube (and hence `food_home`)
/// and the odour field keep their absolute positions, and so does the spawn
/// point (`World::new`): they are fixed landmarks, and scaling them would
/// change the odour/taste question at the same time as the wall question. The
/// consequence is that in a widened arena the fly starts far from every wall
/// (at 4x, 1050 mm from the nearest), which is exactly the condition the loom
/// channel needs in order to carry information.
pub fn active() -> Room {
    let s = room_scale();
    let h = room_height_mm().unwrap_or(ROOM.z[1] * s);
    let fruit = if fruit_on() { Some(FRUIT) } else { None };
    let grey = fruit_grey();
    let hand = active_hand();
    if s == 1.0 && h == ROOM.z[1] && fruit.is_some() && !grey && hand.is_none() {
        return ROOM;
    }
    Room {
        x: [ROOM.x[0] * s, ROOM.x[1] * s],
        y: [ROOM.y[0] * s, ROOM.y[1] * s],
        z: [ROOM.z[0], h],
        table: ROOM.table,
        fruit,
        fruit_grey: grey,
        // The palm's position at t = 0: the origin of its trajectory. `World`
        // moves it every window from the trajectory.
        hand: hand.map(|t| t.hand_at(0.0)),
        wind: ROOM.wind,
        odor_amp: ROOM.odor_amp,
        odor_lambda: ROOM.odor_lambda,
    }
}

#[derive(Clone, Copy)]
pub struct Table {
    pub x: [f32; 2],
    pub y: [f32; 2],
    pub top: f32,
    pub thickness: f32,
}

#[derive(Clone, Copy)]
pub struct Room {
    pub x: [f32; 2],
    pub y: [f32; 2],
    pub z: [f32; 2],
    pub table: Table,
    /// The fruit, if one is in the room. `None` is the clear-room control
    /// (`FLYVERSE_NO_FRUIT=1`).
    pub fruit: Option<Fruit>,
    /// Render the fruit spectrally flat instead of in colour
    /// (`FLYVERSE_FRUIT_GREY=1`): the equi-luminant grey control.
    pub fruit_grey: bool,
    /// The hand, if one is in the room (`FLYVERSE_HAND=approach|static`).
    /// `None` -- the default -- is the no-hand run. This is the palm's position
    /// at the instant the optics sample it: `World::advance` re-places it from
    /// the trajectory every 2 ms window, so it is a moving occluder the retina
    /// ray-casts against, exactly like any other geometry.
    pub hand: Option<Hand>,
    /// Unit direction the room air drifts in.
    pub wind: V3,
    pub odor_amp: f32,
    /// Odour decay length in mm.
    pub odor_lambda: f32,
}

impl Room {
    /// Height of the walkable surface below a point, floor or table top.
    pub fn support_z(&self, x: f32, y: f32) -> f32 {
        let t = &self.table;
        if x >= t.x[0] && x <= t.x[1] && y >= t.y[0] && y <= t.y[1] {
            t.top
        } else {
            self.z[0]
        }
    }

    /// Is the point inside the table volume (for collision)?
    pub fn inside_table(&self, p: V3) -> bool {
        let t = &self.table;
        p[0] >= t.x[0]
            && p[0] <= t.x[1]
            && p[1] >= t.y[0]
            && p[1] <= t.y[1]
            && p[2] >= t.top - t.thickness
            && p[2] <= t.top
    }

    /// Cube centre that marks the food reward.
    pub fn food_home(&self) -> V3 {
        [160.0, 40.0, self.table.top + 4.0]
    }

    /// Isobutylene-equivalent ppm at a point, from a single emitting cube.
    ///
    /// A finite-core, advection-shifted exponential: the source is the cube,
    /// plus a weaker virtual source displaced upwind so that the field forms a
    /// comet-shaped plume with a real left/right gradient at the antennae.
    /// This is a stimulus field, not a fluid solve.
    pub fn odor(&self, p: V3, food: V3) -> f32 {
        let core = 6.0f32;
        let mut c = 0.0f32;
        for (src, w) in [(food, 1.0f32), (shift_upwind(food, self.wind, 55.0), 0.55)] {
            let d = sub(p, src);
            let r = len(d);
            if r <= core {
                c += w;
            } else {
                c += w * (-(r - core) / self.odor_lambda).exp();
            }
        }
        (c * self.odor_amp).min(1.0)
    }

    /// Clamp a point into the room, returning the axes that were clamped.
    pub fn clamp(&self, p: &mut V3) -> [bool; 3] {
        let mut hit = [false; 3];
        for a in 0..3 {
            let (lo, hi) = match a {
                0 => (self.x[0], self.x[1]),
                1 => (self.y[0], self.y[1]),
                _ => (self.z[0], self.z[1]),
            };
            if p[a] < lo {
                p[a] = lo;
                hit[a] = true;
            } else if p[a] > hi {
                p[a] = hi;
                hit[a] = true;
            }
        }
        hit
    }
}

#[inline]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
#[inline]
pub fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
#[inline]
pub fn len(a: V3) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
#[inline]
pub fn norm(a: V3) -> V3 {
    let l = len(a);
    if l <= 1e-9 {
        [0.0, 0.0, 0.0]
    } else {
        scale(a, 1.0 / l)
    }
}
#[inline]
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[inline]
pub fn shift_upwind(a: V3, wind: V3, d: f32) -> V3 {
    let w = norm(wind);
    sub(a, scale(w, d))
}
