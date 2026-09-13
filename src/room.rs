//! The virtual room, in millimetres, Z up, origin on the floor at room centre.
//!
//! Geometry follows the upstream MaleCNS habitat scale so the numbers are
//! comparable: a 600 x 440 x 220 mm arena with a table whose top is 40 mm
//! above the floor and a sugar cube resting on it.

pub type V3 = [f32; 3];

pub const ROOM: Room = Room {
    x: [-300.0, 300.0],
    y: [-220.0, 220.0],
    z: [0.0, 220.0],
    table: Table { x: [40.0, 280.0], y: [-160.0, 120.0], top: 40.0, thickness: 12.0 },
    wind: [1.0, 0.0, 0.0],
    odor_amp: 1.0,
    odor_lambda: 95.0,
};

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
pub fn shift_upwind(a: V3, wind: V3, d: f32) -> V3 {
    let w = norm(wind);
    sub(a, scale(w, d))
}
