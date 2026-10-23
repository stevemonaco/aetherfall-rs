//! Paints a whole map's terrain pixel by pixel. Working per pixel instead of from a tile atlas gives rounded, slightly
//! ragged borders between any pair of terrains without authoring transition tiles. Water is left transparent so the
//! animated water layer underneath shows through. A direct port of TerrainBaker.cs.

use crate::color::Rgba;
use crate::map::{TILE, Terrain, TileMap};
use crate::noise::{self, worley};
use crate::theme::TerrainTheme;

const CORNER_RADIUS: f32 = 6.0;
const NEIGHBOURS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

pub struct Baked {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<Rgba>,
}

pub fn bake(map: &TileMap, theme: &TerrainTheme, lava: bool) -> Baked {
    let (w, h) = (map.pixel_width(), map.pixel_height());
    let shape = build_shape(map, w, h);
    let (down, above) = measure_vertical_runs(&shape, w, h);
    let ctx = Shader { map, theme, shape: &shape, down: &down, above: &above, w, h, lava };
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            pixels.push(ctx.shade(x, y));
        }
    }
    Baked { width: w, height: h, pixels }
}

fn build_shape(map: &TileMap, w: i32, h: i32) -> Vec<Terrain> {
    let mut shape = vec![Terrain::Grass; (w * h) as usize];
    for cy in 0..map.height {
        for cx in 0..map.width {
            let a = map.at(cx, cy);
            for ly in 0..TILE {
                for lx in 0..TILE {
                    shape[((cy * TILE + ly) * w + cx * TILE + lx) as usize] = a;
                }
            }
            if !a.is_organic() {
                continue;
            }
            for (sx, sy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let hn = map.at(cx + sx, cy);
                let vn = map.at(cx, cy + sy);
                let dn = map.at(cx + sx, cy + sy);
                let hc = hn.edge_class();
                if hc != vn.edge_class() || hc != dn.edge_class() || hc == a.edge_class() || !hn.is_organic() {
                    continue;
                }
                let corner_x = if sx < 0 { 0.0 } else { TILE as f32 };
                let corner_y = if sy < 0 { 0.0 } else { TILE as f32 };
                let center_x = corner_x - sx as f32 * CORNER_RADIUS;
                let center_y = corner_y - sy as f32 * CORNER_RADIUS;
                for ly in 0..TILE {
                    for lx in 0..TILE {
                        let (px, py) = (lx as f32 + 0.5, ly as f32 + 0.5);
                        if (px - corner_x).abs() > CORNER_RADIUS || (py - corner_y).abs() > CORNER_RADIUS {
                            continue;
                        }
                        let (dx, dy) = (px - center_x, py - center_y);
                        if dx * sx as f32 > 0.0
                            && dy * sy as f32 > 0.0
                            && dx * dx + dy * dy > CORNER_RADIUS * CORNER_RADIUS
                        {
                            shape[((cy * TILE + ly) * w + cx * TILE + lx) as usize] = dn;
                        }
                    }
                }
            }
        }
    }
    roughen(&shape, w, h)
}

// Higher-priority terrain bulges into its neighbour by a noise-driven 0-2 pixels, so borders aren't ruler-straight.
fn roughen(shape: &[Terrain], w: i32, h: i32) -> Vec<Terrain> {
    let mut result = shape.to_vec();
    for y in 0..h {
        for x in 0..w {
            let a = shape[(y * w + x) as usize];
            if !a.is_organic() {
                continue;
            }
            let reach = (noise::value(x as f32 / 5.0, y as f32 / 5.0, 91) * 3.2) as i32;
            let mut best = a;
            for d in 1..=reach {
                for (ox, oy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
                    let (nx, ny) = (x + ox, y + oy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let b = shape[(ny * w + nx) as usize];
                    if b.is_organic() && b.priority() > best.priority() {
                        best = b;
                    }
                }
            }
            result[(y * w + x) as usize] = best;
        }
    }
    result
}

fn measure_vertical_runs(shape: &[Terrain], w: i32, h: i32) -> (Vec<i16>, Vec<u8>) {
    let mut down = vec![0i16; (w * h) as usize];
    let mut above = vec![0u8; (w * h) as usize];
    for x in 0..w {
        for y in (0..h).rev() {
            let i = (y * w + x) as usize;
            let c = shape[i].edge_class();
            down[i] = if y + 1 < h && shape[((y + 1) * w + x) as usize].edge_class() == c {
                down[((y + 1) * w + x) as usize] + 1
            } else if y + 1 < h {
                0
            } else {
                99
            };
        }
        let mut since_raised = 255u8;
        for y in 0..h {
            let i = (y * w + x) as usize;
            if shape[i].is_raised() {
                since_raised = 0;
            } else {
                since_raised = since_raised.saturating_add(1);
            }
            above[i] = since_raised;
        }
    }
    (down, above)
}

struct Shader<'a> {
    map: &'a TileMap,
    theme: &'a TerrainTheme,
    shape: &'a [Terrain],
    down: &'a [i16],
    above: &'a [u8],
    w: i32,
    h: i32,
    lava: bool,
}

impl Shader<'_> {
    fn shade(&self, x: i32, y: i32) -> Rgba {
        let t = self.shape[(y * self.w + x) as usize];
        let r = self.theme.ramp(t);
        if t == Terrain::Water {
            return self.water(x, y);
        }
        if self.owns_outline(x, y, t) {
            return self.theme.outline;
        }
        let tiled = matches!(self.theme.name, "temple" | "tower" | "citadel" | "ruin");
        let color = match t {
            Terrain::Grass => self.grass(x, y, r),
            Terrain::Flowers => self.flowers(x, y, r),
            Terrain::Path => dirt(x, y, r),
            Terrain::Sand => sand(x, y, r),
            Terrain::Snow => snow(x, y, r),
            Terrain::Rock => self.rock(x, y, r),
            Terrain::Forest => self.forest(x, y, r),
            Terrain::Bridge => self.bridge(x, y, r),
            Terrain::Cobble => cobble(x, y, r),
            Terrain::Floor if tiled => tiled_floor(x, y, r),
            Terrain::Floor => floor(x, y, r),
            Terrain::Pit => self.pit(x, y, r),
            Terrain::Blight if tiled => tiled_floor(x, y, self.theme.ramp(Terrain::Floor)),
            Terrain::Blight => dirt(x, y, self.theme.ramp(Terrain::Path)),
            Terrain::WoodFloor => wood_floor(x, y, r),
            Terrain::InteriorWall => self.interior_wall(x, y, r),
            Terrain::Ice => ice(x, y, r),
            Terrain::Bog => bog(x, y, r),
            Terrain::TallGrass => self.grass(x, y, self.theme.ramp(Terrain::Grass)),
            Terrain::Shallows => self.shallows(x, y),
            Terrain::Metal => metal(x, y),
            Terrain::Water => r[2],
        };
        // Raised terrain casts a short shadow onto the ground below it.
        let above = self.above[(y * self.w + x) as usize];
        if !t.is_raised() && (1..=3).contains(&above) && t != Terrain::Pit {
            return color.lerp(self.theme.outline, 0.35);
        }
        color
    }

    fn at(&self, x: i32, y: i32) -> Terrain {
        self.shape[(y.clamp(0, self.h - 1) * self.w + x.clamp(0, self.w - 1)) as usize]
    }

    fn owns_outline(&self, x: i32, y: i32, t: Terrain) -> bool {
        if t == Terrain::Pit {
            return false;
        }
        let (c, p) = (t.edge_class(), t.priority());
        NEIGHBOURS.iter().any(|&(ox, oy)| {
            let n = self.at(x + ox, y + oy);
            n.edge_class() != c && n.priority() < p && contrasts(t, n)
        })
    }

    fn fringe(&self, x: i32, y: i32, t: Terrain) -> bool {
        let c = t.edge_class();
        NEIGHBOURS.iter().any(|&(ox, oy)| {
            let n = self.at(x + ox, y + oy);
            n.edge_class() != c && n.priority() < t.priority()
        })
    }

    fn water(&self, x: i32, y: i32) -> Rgba {
        for (ox, oy) in NEIGHBOURS {
            let n = self.at(x + ox, y + oy);
            if !matches!(n, Terrain::Water | Terrain::Pit | Terrain::Bridge) {
                return self.theme.foam;
            }
        }
        if self.above[(y * self.w + x) as usize] <= 4 {
            return self.theme.outline.with_alpha(0.45);
        }
        if self.at(x, y - 1) != Terrain::Water && self.at(x, y - 2) == Terrain::Water {
            return self.theme.foam.with_alpha(0.5);
        }
        Rgba::TRANSPARENT
    }

    fn grass(&self, x: i32, y: i32, r: &[Rgba]) -> Rgba {
        if self.fringe(x, y, Terrain::Grass) {
            return r[1];
        }
        if let Some(light) = tuft(x, y) {
            return if light { r[3] } else { r[1] };
        }
        let patch = noise::value(x as f32 / 7.0, y as f32 / 7.0, 13);
        let speck = noise::hash01(x, y, 5);
        if speck < 0.018 {
            return r[3];
        }
        if speck < 0.04 {
            return r[1];
        }
        if patch < 0.3 { r[2].lerp(r[1], 0.45) } else { r[2] }
    }

    fn flowers(&self, x: i32, y: i32, r: &[Rgba]) -> Rgba {
        let (bx, by) = (x / 6, y / 6);
        let hash = noise::hash(bx, by, 33);
        let cx = bx * 6 + 1 + ((hash >> 3) % 4) as i32;
        let cy = by * 6 + 1 + ((hash >> 7) % 4) as i32;
        let (dx, dy) = (x - cx, y - cy);
        let colors = &self.theme.flower_colors;
        let petal = colors[(noise::hash(x / 24, y / 24, 7) % colors.len() as u32) as usize];
        if dx == 0 && dy == 0 {
            return if petal == Rgba::hex(0xF8D850) { Rgba::hex(0xE07030) } else { Rgba::hex(0xF8D850) };
        }
        if dx.abs() + dy.abs() == 1 {
            return if dy == 1 { petal.lerp(r[1], 0.35) } else { petal };
        }
        if dy == 2 && dx == 0 {
            return r[1];
        }
        self.grass(x, y, r)
    }

    fn rock(&self, x: i32, y: i32, r: &[Rgba]) -> Rgba {
        const FACE_HEIGHT: i16 = 10;
        let d = self.down[(y * self.w + x) as usize];
        if d < FACE_HEIGHT {
            if d == FACE_HEIGHT - 1 {
                return r[3];
            }
            let jitter = (noise::value(x as f32 / 3.0, y as f32 / 6.0, 61) * 3.0) as i32;
            return match (x + jitter) % 5 {
                0 => r[0],
                1 => r[2],
                _ if noise::hash01(x, y, 63) < 0.08 => r[0],
                _ => r[1],
            };
        }
        let cell = worley(x as f32, y as f32, 7.0, 67);
        if cell.f2 - cell.f1 < 1.1 {
            return r[0];
        }
        let lit = (x as f32 - cell.center.x) + (y as f32 - cell.center.y);
        if lit < -2.5 {
            r[3]
        } else if lit > 2.5 {
            r[1]
        } else {
            r[2]
        }
    }

    fn forest(&self, x: i32, y: i32, r: &[Rgba]) -> Rgba {
        let d = self.down[(y * self.w + x) as usize];
        if d < 4 {
            let trunk_phase = (x + (noise::hash(x / 9, 0, 71) % 3) as i32) % 9;
            return if trunk_phase == 4 || trunk_phase == 5 {
                if d == 3 { self.theme.trunk.lerp(r[0], 0.5) } else { self.theme.trunk }
            } else {
                r[0]
            };
        }
        let cell = worley(x as f32, y as f32, 7.0, 73);
        if cell.f2 - cell.f1 < 1.0 {
            return r[0];
        }
        let lit = ((x as f32 - cell.center.x) + (y as f32 - cell.center.y)) / 7.0;
        if lit < -0.4 {
            return if cell.id & 7 == 0 { r[3] } else { r[3].lerp(r[2], 0.3) };
        }
        if lit < 0.2 {
            return if noise::hash01(x, y, 75) < 0.07 { r[3] } else { r[2] };
        }
        r[1]
    }

    fn bridge(&self, x: i32, y: i32, r: &[Rgba]) -> Rgba {
        let (cx, cy) = (x / TILE, y / TILE);
        let m = self.map;
        let horizontal = m.at(cx, cy - 1) == Terrain::Water || m.at(cx, cy + 1) == Terrain::Water;
        let (along, across) = if horizontal { (x, y % TILE) } else { (y, x % TILE) };
        let rail_start =
            if horizontal { m.at(cx, cy - 1) != Terrain::Bridge } else { m.at(cx - 1, cy) != Terrain::Bridge };
        let rail_end =
            if horizontal { m.at(cx, cy + 1) != Terrain::Bridge } else { m.at(cx + 1, cy) != Terrain::Bridge };
        if (rail_start && across <= 1) || (rail_end && across >= TILE - 2) {
            return if across == 0 || across == 15 { self.theme.outline } else { r[1] };
        }
        match along % 4 {
            0 => r[0],
            1 => r[3],
            _ if noise::hash01(x, y, 81) < 0.06 => r[1],
            _ => r[2],
        }
    }

    fn pit(&self, x: i32, y: i32, r: &[Rgba]) -> Rgba {
        let mut depth = 0;
        while depth < 6 && self.at(x, y - depth - 1) == Terrain::Pit {
            depth += 1;
        }
        if self.lava {
            return lava(x, y, depth);
        }
        match depth {
            0..2 => r[3],
            2..4 => r[2],
            4..6 => r[1],
            _ => r[0],
        }
    }

    // Ankle-deep water over a sandy bed: lighter than open water and baked, with sparse ripple lines.
    fn shallows(&self, x: i32, y: i32) -> Rgba {
        let water = self.theme.ramp(Terrain::Water);
        let bed = sand(x, y, self.theme.ramp(Terrain::Sand));
        let color = bed.lerp(water[2], 0.55);
        let ripple =
            (x as f32 * 0.5 + noise::value(x as f32 / 12.0, y as f32 / 12.0, 151) * 7.0 + y as f32 * 0.15).sin();
        if ripple > 0.94 && noise::value(x as f32 / 5.0, y as f32 / 3.0, 153) > 0.5 {
            return water[3].lerp(Rgba::WHITE, 0.3);
        }
        if noise::hash01(x, y, 155) < 0.02 { water[3] } else { color }
    }

    fn interior_wall(&self, x: i32, y: i32, r: &[Rgba]) -> Rgba {
        const FACE_HEIGHT: i16 = 13;
        let d = self.down[(y * self.w + x) as usize];
        if d < FACE_HEIGHT {
            if d <= 1 {
                return r[1];
            }
            if d == FACE_HEIGHT - 1 {
                return r[3];
            }
            return if x % 6 == 0 { r[2].lerp(r[1], 0.5) } else { r[2] };
        }
        if (x + y) % 4 == 0 { r[1] } else { r[1].lerp(r[0], 0.4) }
    }
}

// Soft ground transitions (grass to path, etc.) only get a darker fringe, not a black outline.
fn contrasts(t: Terrain, n: Terrain) -> bool {
    t.is_raised() || matches!(n, Terrain::Water | Terrain::Pit) || matches!(t, Terrain::Bridge | Terrain::Cobble)
}

/// A small "v" of grass blades at a hashed spot in each 8x8 block; returns whether the pixel is the light tip.
fn tuft(x: i32, y: i32) -> Option<bool> {
    let (bx, by) = (x >> 3, y >> 3);
    let hash = noise::hash(bx, by, 21);
    if !hash.is_multiple_of(3) {
        return None;
    }
    let ox = (bx << 3) + ((hash >> 4) % 5) as i32;
    let oy = (by << 3) + ((hash >> 9) % 5) as i32;
    let (dx, dy) = (x - ox, y - oy);
    if dy == 0 && (dx == 0 || dx == 2 || dx == 4) {
        return Some(dx == 2);
    }
    if dy == 1 && (dx == 1 || dx == 3) {
        return Some(false);
    }
    None
}

fn dirt(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let speck = noise::hash01(x, y, 41);
    let pebble = noise::hash(x / 5, y / 5, 43);
    if pebble.is_multiple_of(5) {
        let px = x / 5 * 5 + ((pebble >> 5) % 4) as i32;
        let py = y / 5 * 5 + ((pebble >> 9) % 3) as i32;
        if x == px && y == py {
            return r[3];
        }
        if x == px && y == py + 1 {
            return r[0];
        }
    }
    if speck < 0.06 {
        return r[1];
    }
    if speck > 0.97 {
        return r[3];
    }
    if noise::value(x as f32 / 9.0, y as f32 / 9.0, 47) < 0.32 { r[2].lerp(r[1], 0.35) } else { r[2] }
}

fn sand(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let speck = noise::hash01(x, y, 51);
    if speck < 0.03 {
        return r[1];
    }
    if speck > 0.95 {
        return r[3];
    }
    let ripple = (x as f32 * 0.45 + noise::value(x as f32 / 10.0, y as f32 / 10.0, 53) * 6.0 + y as f32 * 0.9).sin();
    if ripple > 0.93 { r[2].lerp(r[1], 0.4) } else { r[2] }
}

fn cobble(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let cell = worley(x as f32, y as f32, 6.0, 85);
    if cell.f2 - cell.f1 < 1.0 {
        return r[0];
    }
    let lit = (x as f32 - cell.center.x) + (y as f32 - cell.center.y);
    if lit < -2.5 {
        r[3]
    } else if lit > 2.5 {
        r[1]
    } else if cell.id & 3 == 0 {
        r[2].lerp(r[1], 0.25)
    } else {
        r[2]
    }
}

fn floor(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let cell = worley(x as f32, y as f32, 10.0, 91);
    if cell.f2 - cell.f1 < 0.8 {
        return r[0];
    }
    let speck = noise::hash01(x, y, 93);
    if speck < 0.04 {
        return r[0];
    }
    if speck > 0.97 {
        return r[3];
    }
    if noise::value(x as f32 / 8.0, y as f32 / 8.0, 95) < 0.35 { r[1] } else { r[2] }
}

fn lava(x: i32, y: i32, depth: i32) -> Rgba {
    if depth < 2 {
        return Rgba::rgb(70, 30, 20);
    }
    let n = noise::hash(x / 3, y / 2, 151) % 8;
    let swirl = (x as f32 * 0.21 + y as f32 * 0.13 + n as f32).sin();
    if swirl > 0.6 {
        Rgba::rgb(255, 210, 90)
    } else if swirl > -0.2 {
        Rgba::rgb(240, 110, 30)
    } else {
        Rgba::rgb(180, 50, 20)
    }
}

fn snow(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let speck = noise::hash01(x, y, 131);
    if speck < 0.012 {
        return r[3];
    }
    if speck > 0.985 {
        return r[1];
    }
    let drift = noise::value(x as f32 / 14.0, y as f32 / 9.0, 133);
    if drift < 0.3 {
        return r[2].lerp(r[1], 0.35);
    }
    if drift > 0.72 { r[2].lerp(r[3], 0.5) } else { r[2] }
}

// Dressed stone tiles for built interiors: grout lines, a bevelled top-left edge, per-tile variation.
fn tiled_floor(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let (lx, ly) = (x & 15, y & 15);
    if lx == 15 || ly == 15 {
        return r[0];
    }
    if lx == 0 || ly == 0 {
        return r[3];
    }
    let tile = noise::hash(x >> 4, y >> 4, 141);
    if tile & 7 == 0 && (lx - ly - ((tile >> 8) % 5) as i32).abs() < 1 {
        return r[1];
    }
    if noise::hash01(x, y, 143) < 0.03 {
        return r[1];
    }
    match tile & 3 {
        1 => r[2].lerp(r[1], 0.3),
        2 => r[2].lerp(r[3], 0.2),
        _ => r[2],
    }
}

fn ice(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let diagonal = (x + y * 2) % 23;
    if (diagonal == 0 || diagonal == 1) && noise::value(x as f32 / 9.0, y as f32 / 9.0, 111) > 0.45 {
        return r[3];
    }
    let cell = worley(x as f32, y as f32, 12.0, 113);
    if cell.f2 - cell.f1 < 0.6 {
        return r[1];
    }
    if noise::value(x as f32 / 10.0, y as f32 / 10.0, 115) < 0.4 { r[2].lerp(r[3], 0.3) } else { r[2] }
}

// Riveted sky-metal plates; grooves every half tile and a bolt at each plate corner.
fn metal(x: i32, y: i32) -> Rgba {
    let r = [Rgba::hex(0x22283A), Rgba::hex(0x48526A), Rgba::hex(0x6C7890), Rgba::hex(0x9AA6BC)];
    let (lx, ly) = (x & 15, y & 15);
    if lx == 15 || ly == 15 {
        return r[0];
    }
    if lx == 0 || ly == 0 {
        return r[3];
    }
    if (lx == 2 || lx == 13) && (ly == 2 || ly == 13) {
        return r[3];
    }
    if (lx == 3 || lx == 14) && (ly == 3 || ly == 14) {
        return r[0];
    }
    if ly == 7 {
        return r[1];
    }
    if noise::hash01(x, y, 157) < 0.04 {
        r[1]
    } else if (x + y) % 9 == 0 {
        r[2].lerp(r[3], 0.3)
    } else {
        r[2]
    }
}

fn bog(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let cell = worley(x as f32, y as f32, 7.0, 121);
    if cell.id & 7 == 0 && cell.f1 < 2.2 {
        return if cell.f1 < 1.0 {
            r[3]
        } else if (y as f32) < cell.center.y {
            r[0]
        } else {
            r[1]
        };
    }
    if noise::hash01(x, y, 123) < 0.05 {
        return r[0];
    }
    if noise::value(x as f32 / 6.0, y as f32 / 6.0, 125) < 0.45 { r[1] } else { r[2] }
}

fn wood_floor(x: i32, y: i32, r: &[Rgba]) -> Rgba {
    let row = y / 4;
    if y % 4 == 3 || (x + row * 7) % 16 == 0 {
        return r[0];
    }
    if y % 4 == 0 {
        return r[3];
    }
    if noise::hash01(x, y, 97) < 0.08 { r[1] } else { r[2] }
}

/// The animated water tile frames (four 16px frames side by side), drawn under the baked terrain.
pub fn water_frames(ramp: &[Rgba]) -> (i32, i32, Vec<Rgba>) {
    const FRAMES: i32 = 4;
    let width = TILE * FRAMES;
    let mut pixels = vec![Rgba::TRANSPARENT; (width * TILE) as usize];
    for f in 0..FRAMES {
        for y in 0..TILE {
            for x in 0..TILE {
                let phase = f as f32 / FRAMES as f32 * std::f32::consts::TAU;
                let mut color = ramp[2];
                for row in [3.0f32, 11.0] {
                    let shift = if row == 3.0 { 0.0 } else { 8.0 };
                    let wave = ((x as f32 + shift) / TILE as f32 * std::f32::consts::TAU + phase).sin() * 1.5;
                    let crest = row + wave;
                    let lit = (x + shift as i32 + f * 2) % TILE < 7;
                    if (y as f32 - crest).abs() < 0.6 && lit {
                        color = ramp[3];
                    } else if (y as f32 - crest - 1.0).abs() < 0.6 && lit {
                        color = ramp[1];
                    }
                }
                if noise::hash01(x, y, 101 + f) < 0.015 {
                    color = ramp[3];
                }
                pixels[(y * width + f * TILE + x) as usize] = color;
            }
        }
    }
    (width, TILE, pixels)
}
