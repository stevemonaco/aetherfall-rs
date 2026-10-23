//! Deterministic hash-based noise used by the procedural art. Matches the C# game bit for bit.

use bevy::math::Vec2;

pub fn hash(x: i32, y: i32, seed: i32) -> u32 {
    let h =
        x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263)).wrapping_add(seed.wrapping_mul(1442695041))
            as u32;
    let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

pub fn hash01(x: i32, y: i32, seed: i32) -> f32 {
    (hash(x, y, seed) & 0xFFFFFF) as f32 / 0x1000000 as f32
}

pub fn value(x: f32, y: f32, seed: i32) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let fx = fx * fx * (3.0 - 2.0 * fx);
    let fy = fy * fy * (3.0 - 2.0 * fy);
    let a = hash01(ix, iy, seed);
    let b = hash01(ix + 1, iy, seed);
    let c = hash01(ix, iy + 1, seed);
    let d = hash01(ix + 1, iy + 1, seed);
    lerp(lerp(a, b, fx), lerp(c, d, fx), fy)
}

/// Cellular noise: distances to the nearest and second-nearest feature points, the nearest cell's id and its centre.
pub struct Worley {
    pub f1: f32,
    pub f2: f32,
    pub id: u32,
    pub center: Vec2,
}

pub fn worley(x: f32, y: f32, cell_size: f32, seed: i32) -> Worley {
    let (sx, sy) = (x / cell_size, y / cell_size);
    let (cx, cy) = (sx.floor() as i32, sy.floor() as i32);
    let (mut f1, mut f2, mut id, mut center) = (f32::MAX, f32::MAX, 0, Vec2::ZERO);
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (gx, gy) = (cx + ox, cy + oy);
            let px = gx as f32 + hash01(gx, gy, seed) * 0.8 + 0.1;
            let py = gy as f32 + hash01(gx, gy, seed + 7) * 0.8 + 0.1;
            let d = ((px - sx) * (px - sx) + (py - sy) * (py - sy)).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = hash(gx, gy, seed + 3);
                center = Vec2::new(px * cell_size, py * cell_size);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    Worley { f1: f1 * cell_size, f2: f2 * cell_size, id, center }
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
