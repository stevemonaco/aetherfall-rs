//! Procedurally drawn dungeon entrances: the Mossy Grotto's mouth, the Sunken Temple, the Storm Spire, the Fallen
//! Bastion's gatehouse and the Sky Altar. Each is drawn larger than its footprint, anchored at its doorway.

use std::f32::consts::PI;

use crate::art::props::LEAF;
use crate::core::canvas::Canvas;
use crate::core::color::{Rgba, c};
use crate::core::gfx::{self, TexId};
use crate::core::noise;
use crate::core::palette;

/// Where an entrance sprite's anchor tile (its doorway, bottom-centre) sits inside the texture, in pixels.
pub fn anchor(look: &str) -> (f32, f32) {
    match look {
        "grotto" => (38.0, 60.0),
        "temple" => (56.0, 100.0),
        "spire" => (28.0, 96.0),
        "bastion" => (56.0, 64.0),
        _ => (56.0, 80.0),
    }
}

pub fn texture(look: &str) -> TexId {
    match look {
        "grotto" => grotto(),
        "temple" => temple(),
        "spire" => spire(),
        "bastion" => bastion(),
        _ => sky_altar(),
    }
}

/// Staggered ashlar blocks: each a slightly varied mid tone with a lit top edge and dark mortar.
fn masonry(cv: &mut Canvas, area: (i32, i32, i32, i32), ramp: &[Rgba; 4], block: (i32, i32), seed: i32) {
    let (x0, y0, w, h) = area;
    let (bw, bh) = block;
    for y in 0..h {
        let row = y / bh;
        let shift = if row % 2 == 1 { bw / 2 } else { 0 };
        for x in 0..w {
            let (bx, lx, ly) = ((x + shift) / bw, (x + shift) % bw, y % bh);
            let n = noise::hash01(bx, row, seed);
            let col = if ly == bh - 1 || lx == 0 {
                ramp[1].lerp(ramp[0], 0.55)
            } else if ly == 0 {
                ramp[3].lerp(ramp[2], 0.25 + n * 0.5)
            } else {
                ramp[2].lerp(ramp[1], n * 0.55)
            };
            cv.set(x0 + x, y0 + y, col);
        }
    }
}

/// Brightens the left of a face and darkens its right, so it reads as lit from the top-left.
fn side_light(cv: &mut Canvas, area: (i32, i32, i32, i32), ramp: &[Rgba; 4]) {
    let (x0, y0, w, h) = area;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let col = cv.get(x, y);
            if col.a == 0 {
                continue;
            }
            let t = (x - x0) as f32 / (w - 1).max(1) as f32;
            let lit = if t > 0.7 {
                col.lerp(ramp[0], 0.3 + (t - 0.7) * 1.2)
            } else if t < 0.16 {
                col.lerp(ramp[3], 0.3)
            } else {
                col
            };
            cv.set(x, y, lit);
        }
    }
}

/// Blends opaque pixels toward `col` with a radial falloff, for glows that stay inside the silhouette.
fn glow(cv: &mut Canvas, center: (f32, f32), radius: f32, col: Rgba, strength: f32) {
    let (cx, cy) = center;
    for y in (cy - radius) as i32..=(cy + radius) as i32 {
        for x in (cx - radius) as i32..=(cx + radius) as i32 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let here = cv.get(x, y);
            if d < radius && here.a > 0 {
                cv.set(x, y, here.lerp(col, (1.0 - d / radius) * strength));
            }
        }
    }
}

/// A wavy hanging (or, with negative `len`, rising) strand of weed, vine or root.
fn strand(cv: &mut Canvas, x: i32, y: i32, len: i32, cols: (Rgba, Rgba), seed: i32) {
    let dir = len.signum();
    for i in 0..len.abs() {
        let dx = ((i as f32 * 0.55 + seed as f32 * 1.7).sin() * 1.1).round() as i32;
        let col = if i == len.abs() - 1 || (i + seed) % 4 == 0 { cols.1 } else { cols.0 };
        cv.set(x + dx, y + i * dir, col);
    }
}

fn boulder_shade(x: i32, y: i32, cell: f32, seed: i32, ramp: &[Rgba; 4]) -> Rgba {
    let w = noise::worley(x as f32, y as f32, cell, seed);
    if w.f2 - w.f1 < 1.1 {
        return ramp[0].lerp(ramp[1], 0.35);
    }
    let lit = ((x as f32 - w.center.x) + (y as f32 - w.center.y)) / cell;
    if lit < -0.55 {
        ramp[3]
    } else if lit < 0.05 {
        ramp[2]
    } else if lit < 0.5 {
        ramp[1]
    } else {
        ramp[1].lerp(ramp[0], 0.45)
    }
}

/// The Mossy Grotto: a heaped, moss-grown rock arch with roots trailing over a mouth that glows with embers.
pub fn grotto() -> TexId {
    gfx::cached("entrance_grotto", || {
        let rock = [c(0x221C1A), c(0x4A4038), c(0x726656), c(0xA4967E)];
        let (w, h) = (76, 60);
        let mut cv = Canvas::new(w, h);
        for (x, y, r) in [
            (38.0, 37.0, 27.0),
            (15.0, 42.0, 12.5),
            (61.0, 42.0, 12.5),
            (24.0, 21.0, 12.0),
            (52.0, 20.0, 13.0),
            (37.0, 12.0, 10.5),
            (9.0, 52.0, 7.5),
            (67.0, 52.0, 7.5),
        ] {
            cv.circle(x, y, r, rock[2]);
        }
        cv.rect(3, 48, 70, 12, rock[2]);
        cv.shade(|x, y, _| boulder_shade(x, y, 10.0, 41, &rock));
        cv.shade(|x, y, col| {
            let value = noise::value(x as f32 / 5.0, y as f32 / 3.5, 77);
            if value > 0.55 && (col == rock[3] || col == rock[2]) && y < 50 {
                if col == rock[3] { LEAF[4] } else { LEAF[3] }
            } else if value > 0.5 && col == rock[1] && y < 50 {
                LEAF[2]
            } else {
                col
            }
        });
        for x in 0..w {
            let Some(top) = (0..h).find(|&y| cv.is_set(x, y)) else { continue };
            let depth = 2 + (noise::hash(x, 0, 5) % 3) as i32;
            for k in 0..depth {
                cv.set(x, top + k, [LEAF[4], LEAF[3], LEAF[2], LEAF[1]][k as usize]);
            }
            if noise::hash01(x, 1, 6) < 0.22 && top < 36 {
                strand(&mut cv, x, top + depth, 2 + (noise::hash(x, 2, 7) % 5) as i32, (LEAF[2], LEAF[1]), x);
            }
        }

        let (mx, my) = (38.0, 42.0);
        let arch = |x: i32, y: i32, rx: f32, ry: f32| {
            let dx = (x as f32 + 0.5 - mx) / rx;
            let dy = ((y as f32 + 0.5 - my) / ry).min(0.0);
            (dx * dx + dy * dy).sqrt()
        };
        let (outer, inner) = ((20.5, 24.0), (14.0, 17.0));
        for y in 12..h {
            for x in 10..66 {
                let (o, i) = (arch(x, y, outer.0, outer.1), arch(x, y, inner.0, inner.1));
                if o >= 1.0 || i < 1.0 {
                    continue;
                }
                let (fx, fy) = (x as f32 + 0.5 - mx, (y as f32 + 0.5 - my).min(0.0));
                let seg = if fy < 0.0 { (-fy).atan2(fx) / PI * 7.0 } else { 7.0 + y as f32 / 6.0 };
                let id = seg.floor() as i32 + if fx < 0.0 && fy >= 0.0 { 40 } else { 0 };
                let col = if seg.fract() < 0.16 || o > 0.92 {
                    rock[0]
                } else if i < 1.1 {
                    rock[1].lerp(rock[0], 0.4)
                } else if fx < 0.0 && fy < -4.0 {
                    rock[3].lerp(rock[2], noise::hash01(id, 3, 9) * 0.6)
                } else {
                    rock[2].lerp(rock[1], noise::hash01(id, 3, 9) * 0.7)
                };
                cv.set(x, y, col);
            }
        }
        for (sx, sy) in [(28, 22), (35, 19), (44, 20), (49, 24), (25, 27), (52, 30)] {
            cv.set(sx, sy, LEAF[4]);
            cv.set(sx + 1, sy, LEAF[3]);
            cv.set(sx, sy + 1, LEAF[3]);
            cv.set(sx + 1, sy + 1, LEAF[2]);
        }

        let ember = [c(0x0C080C), c(0x2A0E0A), c(0x6A2410), c(0xB4481A), c(0xF08A30)];
        for y in 12..h {
            for x in 10..66 {
                let i = arch(x, y, inner.0, inner.1);
                if i >= 1.0 {
                    continue;
                }
                let d = ((x as f32 + 0.5 - mx).powi(2) + ((y as f32 + 0.5 - 61.0) * 1.5).powi(2)).sqrt() / 18.0;
                let heat = (1.0 - d).max(0.0);
                let col = if i > 0.88 && y < 50 {
                    palette::OUTLINE
                } else if heat > 0.74 {
                    ember[3].lerp(ember[4], (heat - 0.74) * 2.5)
                } else if heat > 0.48 {
                    ember[2]
                } else if heat > 0.22 {
                    ember[1]
                } else {
                    ember[0]
                };
                cv.set(x, y, col);
            }
        }
        for (x, len, root) in [
            (25, 11, true),
            (28, 16, false),
            (31, 7, true),
            (34, 10, false),
            (42, 13, true),
            (45, 8, false),
            (48, 14, false),
            (51, 9, true),
        ] {
            let top = (12..h).find(|&y| arch(x, y, outer.0, outer.1) < 1.0).unwrap_or(18);
            let cols = if root { (c(0x6A4426), c(0x2E1C10)) } else { (LEAF[2], LEAF[4]) };
            strand(&mut cv, x, top, len, cols, x);
        }
        for (x, y, r) in [(15.0, 55.5, 3.0), (60.0, 55.0, 3.4), (22.0, 57.5, 2.0), (54.0, 58.0, 1.8)] {
            cv.circle(x, y, r, rock[2]);
            cv.set(x as i32 - 1, (y - r) as i32 + 1, rock[3]);
            cv.rect((x - r) as i32 + 1, (y + r) as i32 - 1, (r * 2.0) as i32 - 1, 1, rock[0]);
        }
        cv.shade(|_, y, col| {
            if y > 52 && col != ember[3] && col != ember[4] { col.lerp(rock[0], 0.07 * (y - 52) as f32) } else { col }
        });
        for (x, y) in [(6, 58), (69, 57), (18, 51), (57, 51), (11, 47)] {
            cv.line(x, y, x - 1, y - 2, LEAF[3]).line(x + 1, y, x + 1, y - 3, LEAF[4]).line(
                x + 2,
                y,
                x + 3,
                y - 2,
                LEAF[3],
            );
        }
        cv.outline();
        cv
    })
}

/// The Sunken Temple: a water-worn Concord facade with a pediment, columns (one broken), weed and barnacles, and
/// steps sinking into a dark doorway.
pub fn temple() -> TexId {
    gfx::cached("entrance_temple", || {
        let stone = [c(0x24303A), c(0x4A626A), c(0x7C9696), c(0xB8CCC2)];
        let recess = [c(0x141C24), c(0x24323A), c(0x3A4E56), c(0x5A7276)];
        let sea = [c(0x0E2C28), c(0x1C5040), c(0x2E7A5A), c(0x62B486)];
        let tide = [c(0x1E6A8A), c(0x48B8D0), c(0x9AF0F0), Rgba::WHITE];
        let (w, h) = (112, 100);
        let mut cv = Canvas::new(w, h);

        masonry(&mut cv, (4, 38, 104, 46), &recess, (10, 6), 3);
        for y in 2..29 {
            let half = ((y - 2) as f32 / 26.0 * 54.0) as i32;
            for x in 56 - half..=56 + half {
                let edge = x <= 56 - half + 2 || x >= 56 + half - 2 || y >= 26;
                let col = if !edge {
                    stone[1].lerp(recess[2], 0.4)
                } else if x < 56 {
                    stone[3]
                } else {
                    stone[2]
                };
                cv.set(x, y, col);
            }
        }
        cv.circle(56.0, 17.0, 6.5, stone[2]).circle(56.0, 17.0, 5.0, tide[0]).circle(56.0, 17.0, 3.5, tide[1]);
        cv.circle(55.0, 16.0, 1.6, tide[2]);
        cv.set(54, 15, tide[3]);
        for side in [-1, 1] {
            for i in 0..4 {
                let (x0, y0) = (56 + side * (10 + i * 7), 20 - i % 2);
                cv.line(x0, y0, x0 + side * 3, y0 - 2, stone[3]).line(
                    x0 + side * 3,
                    y0 - 2,
                    x0 + side * 6,
                    y0,
                    stone[3],
                );
                cv.line(x0, y0 + 1, x0 + side * 6, y0 + 1, stone[0].lerp(stone[1], 0.5));
            }
        }

        cv.rect(0, 28, w, 3, stone[3]).rect(0, 30, w, 1, stone[1]);
        cv.rect(2, 31, w - 4, 5, stone[2]);
        for x in 2..w - 2 {
            let wy = 33 + ((x as f32 * 0.55).sin() * 1.4).round() as i32;
            cv.set(x, wy, stone[3]);
            cv.set(x, wy + 1, stone[1]);
        }
        cv.rect(2, 36, w - 4, 3, stone[2]).rect(2, 36, w - 4, 1, stone[3]).rect(2, 38, w - 4, 1, stone[0]);

        cv.rect(43, 47, 26, 4, stone[2]).rect(43, 47, 26, 1, stone[3]).rect(43, 50, 26, 1, stone[0]);
        cv.rect(55, 48, 2, 2, tide[2]);
        for y in 51..86 {
            for x in 46..66 {
                let depth = (85 - y) as f32 / 34.0;
                cv.set(x, y, recess[1].lerp(palette::OUTLINE, 0.4 + depth * 0.6));
            }
        }
        for (i, sy) in [72, 76, 80].iter().enumerate() {
            let shade = recess[3].lerp(palette::OUTLINE, 0.65 - i as f32 * 0.25);
            cv.rect(46, *sy, 20, 3, shade).rect(46, sy + 3, 20, 1, palette::OUTLINE);
        }
        cv.rect(45, 51, 1, 35, stone[1]).rect(66, 51, 1, 35, stone[0]);

        for (i, cx) in [10, 26, 42, 70, 86, 102].into_iter().enumerate() {
            let broken = i == 5;
            let top = if broken { 58 } else { 41 };
            for y in top..82 {
                for x in cx - 4..cx + 4 {
                    let t = x - (cx - 4);
                    let col = match t {
                        0 | 1 => stone[3],
                        6 | 7 => stone[1],
                        _ if t % 2 == 1 => stone[1].lerp(stone[2], 0.5),
                        _ => stone[2],
                    };
                    cv.set(x, y, col);
                }
            }
            if broken {
                for x in cx - 4..cx + 4 {
                    let notch = (noise::hash(x, 1, 21) % 4) as i32;
                    for y in top..top + notch {
                        cv.set(x, y, Rgba::TRANSPARENT);
                    }
                    cv.set(x, top + notch, stone[3]);
                }
                masonry(&mut cv, (cx - 5, 41, 10, 17), &recess, (10, 6), 3);
            } else {
                cv.rect(cx - 6, 39, 12, 3, stone[3]).rect(cx - 6, 41, 12, 1, stone[1]);
                cv.rect(cx - 5, 39, 1, 1, stone[2]).rect(cx + 4, 39, 1, 1, stone[2]);
            }
            cv.rect(cx - 6, 81, 12, 3, stone[2]).rect(cx - 6, 81, 12, 1, stone[3]);
        }
        for (x, y) in [(108, 80), (104, 82), (111, 83)] {
            cv.rect(x - 2, y, 4, 3, stone[2]).rect(x - 2, y, 4, 1, stone[3]);
        }

        cv.rect(0, 84, w, 5, stone[3].lerp(stone[2], 0.5)).rect(0, 84, w, 1, stone[3]).rect(0, 88, w, 1, stone[1]);
        cv.rect(0, 89, w, 6, stone[2]).rect(0, 89, w, 1, stone[3]).rect(0, 94, w, 1, stone[1]);
        cv.rect(0, 95, w, 5, stone[1]).rect(0, 95, w, 1, stone[2]);
        for x in (7..w).step_by(13) {
            cv.rect(x, 84, 1, 4, stone[1]).rect(x + 5, 89, 1, 5, stone[1]).rect(x + 2, 95, 1, 5, stone[0]);
        }

        cv.shade(|x, y, col| {
            if y < 73 || col == palette::OUTLINE {
                return col;
            }
            if y == 73 || (y == 74 && noise::hash01(x, y, 60) < 0.5) {
                return col.lerp(sea[0], 0.55);
            }
            let streak = noise::value(x as f32 / 1.5, y as f32 / 6.0, 61) > 0.6;
            col.lerp(if streak { sea[1] } else { sea[2] }, if streak { 0.45 } else { 0.12 })
        });
        cv.line(40, 15, 44, 20, stone[0]).line(44, 20, 42, 25, stone[0]).line(71, 12, 68, 19, stone[0]);
        cv.line(24, 55, 26, 64, stone[0].lerp(stone[1], 0.4)).line(85, 46, 88, 58, stone[0].lerp(stone[1], 0.4));
        for x in (98..112).rev() {
            for y in 25..30 + (111 - x) / 3 {
                if (x - 98) * 2 > (y - 25) * 3 + 4 {
                    cv.set(x, y, Rgba::TRANSPARENT);
                }
            }
        }

        for (x, len) in [(5, 7), (19, 11), (33, 5), (47, 9), (63, 6), (77, 12), (93, 8)] {
            strand(&mut cv, x, 31, len, (sea[2], sea[3]), x);
        }
        for (x, len) in
            [(3, 10), (8, 7), (17, 12), (31, 8), (36, 6), (50, 9), (62, 11), (75, 7), (82, 13), (95, 9), (109, 10)]
        {
            strand(&mut cv, x, 99, -len, (sea[2], sea[3]), x * 3);
            strand(&mut cv, x + 1, 99, -len / 2, (sea[1], sea[2]), x * 5);
        }
        for (x, y) in
            [(8, 78), (12, 74), (27, 80), (41, 76), (69, 79), (88, 75), (100, 79), (20, 92), (58, 97), (96, 91)]
        {
            for (bx, by, big) in [(x, y, true), (x + 3, y + 1, false)] {
                cv.set(bx, by, c(0xE8E4D4));
                cv.set(bx, by + 1, sea[0]);
                if big {
                    cv.set(bx + 1, by, c(0xC0BCAC));
                    cv.set(bx + 1, by + 1, sea[0]);
                    cv.set(bx, by - 1, c(0xC0BCAC));
                }
            }
        }
        glow(&mut cv, (56.0, 17.0), 10.0, tide[2], 0.35);
        cv.outline();
        cv
    })
}

/// The Storm Spire: a tall, narrow tower of slate bound with copper bands, a storm-lit lantern room and a lightning
/// rod, rising well above its three-tile base.
pub fn spire() -> TexId {
    gfx::cached("entrance_spire", || {
        let slate = [c(0x1C1C2A), c(0x3A3A52), c(0x60607C), c(0x9292AE)];
        let copper = [c(0x4A2414), c(0x8A4A24), c(0xC87A3A), c(0xF0B070)];
        let verdigris = c(0x4AA890);
        let storm = [c(0x1E2A78), c(0x4A7AE8), c(0xA8D0FF), c(0xF4FAFF)];
        let (w, h) = (56, 96);
        let mut cv = Canvas::new(w, h);

        for (area, block, seed) in [
            ((5, 91, 46, 5), (9, 5), 11),
            ((9, 86, 38, 5), (8, 5), 12),
            ((13, 54, 30, 32), (6, 4), 13),
            ((17, 32, 22, 19), (5, 4), 14),
            ((20, 16, 16, 12), (4, 4), 15),
        ] {
            masonry(&mut cv, area, &slate, block, seed);
            side_light(&mut cv, area, &slate);
        }

        let band = |cv: &mut Canvas, x: i32, y: i32, bw: i32, bh: i32| {
            cv.rect(x, y, bw, bh, copper[2]).rect(x, y, bw, 1, copper[3]).rect(x, y + bh - 1, bw, 1, copper[0]);
            side_light(cv, (x, y, bw, bh), &copper);
            for rx in (x + 3..x + bw - 2).step_by(5) {
                cv.set(rx, y + bh / 2, copper[3]);
            }
            for vx in (x + 2..x + bw - 2).step_by(4) {
                if noise::hash01(vx, y, 31) < 0.5 {
                    let len = 2 + (noise::hash(vx, y, 32) % 4) as i32;
                    for k in 0..len {
                        let here = cv.get(vx, y + bh + k);
                        cv.set(vx, y + bh + k, here.lerp(verdigris, 0.6 - k as f32 * 0.12));
                    }
                }
            }
        };
        band(&mut cv, 10, 51, 36, 4);
        band(&mut cv, 14, 29, 28, 3);
        band(&mut cv, 18, 14, 20, 2);

        for y in 3..14 {
            let half = 1 + ((y - 3) as f32 / 11.0 * 10.0) as i32;
            for x in 28 - half..28 + half {
                let seam = (y - 3) % 3 == 2 && (x + (y - 3) / 3 * 2) % 4 == 0;
                let t = (x - (28 - half)) as f32 / (half * 2) as f32;
                let col = if (y - 3) % 3 == 2 && t > 0.1 {
                    slate[1]
                } else if seam {
                    slate[0]
                } else if t < 0.35 {
                    slate[3]
                } else if t < 0.7 {
                    slate[2]
                } else {
                    slate[1]
                };
                cv.set(x, y, col);
            }
        }
        cv.rect(27, 0, 2, 4, copper[2]).rect(27, 0, 1, 4, copper[3]).rect(24, 2, 8, 1, copper[1]);
        cv.set(24, 1, copper[2]);
        cv.set(31, 1, copper[2]);

        for y in 17..28 {
            for x in 21..35 {
                let d = ((x as f32 + 0.5 - 28.0).powi(2) + (y as f32 + 0.5 - 22.5).powi(2)).sqrt();
                let col = if d < 2.5 {
                    storm[3]
                } else if d < 4.5 {
                    storm[2]
                } else if d < 7.0 {
                    storm[1]
                } else {
                    storm[0]
                };
                cv.set(x, y, col);
            }
        }
        for mx in [23, 32] {
            cv.rect(mx, 17, 1, 11, slate[1]);
        }
        cv.rect(21, 17, 14, 1, slate[0]).rect(20, 27, 16, 1, slate[3]);

        for y in 36..47 {
            for x in 26..30 {
                let edge = x == 26 || x == 29 || y == 36 || y == 46;
                cv.set(
                    x,
                    y,
                    if edge {
                        storm[1]
                    } else if y < 41 {
                        storm[3]
                    } else {
                        storm[2]
                    },
                );
            }
        }
        cv.set(26, 36, slate[1]);
        cv.set(29, 36, slate[1]);
        for sx in [17, 38] {
            cv.rect(sx, 59, 2, 9, palette::OUTLINE).rect(sx, 62, 2, 3, storm[0]);
        }

        let (dx, dy, r) = (28.0, 74.0, 6.0);
        for y in 66..86 {
            for x in 20..36 {
                let fx = x as f32 + 0.5 - dx;
                let fy = (y as f32 + 0.5 - dy).min(0.0);
                let d = (fx * fx + fy * fy).sqrt();
                if d < r - 1.0 {
                    let lit = ((y - 66) as f32 / 20.0).powf(1.5);
                    cv.set(x, y, palette::OUTLINE.lerp(storm[0], lit * 0.9));
                } else if d < r + 1.0 {
                    cv.set(x, y, if fx < 0.0 { copper[3] } else { copper[1] });
                }
            }
        }
        cv.rect(24, 84, 8, 2, storm[1].lerp(slate[2], 0.4));
        cv.line(15, 70, 17, 77, slate[0]).line(17, 77, 16, 82, slate[0]).line(37, 38, 35, 44, slate[0]);
        cv.outline();
        cv
    })
}

/// The Fallen Bastion: a gatehouse between two broken towers, its breached wall cracked to the keystone, a bent
/// portcullis jammed half-raised and tattered banners hanging from the towers.
pub fn bastion() -> TexId {
    gfx::cached("entrance_bastion", || {
        let fort = [c(0x2A2226), c(0x5C4E4C), c(0x8C7C74), c(0xBCAA9A)];
        let iron = [c(0x16161E), c(0x34343F), c(0x585868), c(0x8A8A9C)];
        let banner = [c(0x3A0C14), c(0x6E1A22), c(0x9E2C32), c(0xC84A44)];
        let gold = [c(0x7A5A20), c(0xC8A040)];
        let soot = c(0x16121A);
        let (w, h) = (128, 64);
        let mut cv = Canvas::new(w, h);

        masonry(&mut cv, (40, 14, 48, 50), &fort, (8, 5), 51);
        cv.rect(40, 14, 48, 1, fort[3]);
        for x in (44..84).step_by(8) {
            if !(54..74).contains(&x) {
                masonry(&mut cv, (x, 9, 5, 5), &fort, (5, 5), 52);
                cv.rect(x, 9, 5, 1, fort[3]);
            }
        }
        for x in 54..74i32 {
            let breach = 5 - (x - 64).abs() / 3 + (noise::hash(x, 0, 53) % 2) as i32;
            for y in 9..15 + breach.max(0) {
                cv.set(x, y, Rgba::TRANSPARENT);
            }
            cv.set(x, 15 + breach.max(0), fort[3]);
        }
        for (x0, flip) in [(4, false), (86, true)] {
            masonry(&mut cv, (x0, 13, 38, 51), &fort, (8, 5), 54 + x0);
            side_light(&mut cv, (x0, 13, 38, 51), &fort);
            cv.rect(x0 - 1, 12, 40, 2, fort[3]).rect(x0 - 1, 14, 40, 1, fort[0]);
            for i in 0..4 {
                let mx = x0 + i * 10;
                let broken = if flip { i == 3 } else { i == 1 };
                if broken {
                    for x in mx..mx + 6 {
                        let stub = (noise::hash(x, i, 55) % 3) as i32;
                        cv.rect(x, 11 - stub, 1, stub + 1, fort[2]);
                    }
                    continue;
                }
                let chip = if flip && i == 0 { 2 } else { 0 };
                masonry(&mut cv, (mx, 5 + chip, 7, 7 - chip), &fort, (7, 4), 56);
                cv.rect(mx, 5 + chip, 7, 1, fort[3]).rect(mx + 6, 5 + chip, 1, 7 - chip, fort[1]);
            }
            for (sx, sy) in [(x0 + if flip { 6 } else { 29 }, 24), (x0 + 18, 50)] {
                cv.rect(sx, sy, 3, 9, palette::OUTLINE).rect(sx, sy, 1, 9, fort[0]);
            }
        }

        let (ax, ay, r) = (64.0, 41.0, 15.0);
        for y in 20..h {
            for x in 44..84 {
                let fx = x as f32 + 0.5 - ax;
                let fy = (y as f32 + 0.5 - ay).min(0.0);
                let d = (fx * fx + fy * fy).sqrt();
                if d < r {
                    let depth = ((y - 26) as f32 / 38.0).clamp(0.0, 1.0);
                    cv.set(x, y, c(0x0A080C).lerp(c(0x2E2428), depth * depth));
                } else if d < r + 5.0 {
                    let seg = if fy < 0.0 { (-fy).atan2(fx) / PI * 9.0 } else { 9.0 + y as f32 / 6.0 };
                    let col = if seg.fract() < 0.15 || d > r + 4.0 {
                        fort[0]
                    } else if fx < 0.0 {
                        fort[3].lerp(fort[2], noise::hash01(seg as i32, 1, 57) * 0.6)
                    } else {
                        fort[2].lerp(fort[1], noise::hash01(seg as i32, 1, 57) * 0.6)
                    };
                    cv.set(x, y, col);
                }
            }
        }
        cv.rect(61, 21, 6, 6, fort[3]).rect(66, 21, 1, 6, fort[1]).rect(61, 26, 6, 1, fort[0]);
        let crack = [(66, 17), (63, 20), (65, 24), (64, 27), (66, 31)];
        for pair in crack.windows(2) {
            cv.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, palette::OUTLINE);
        }
        cv.line(10, 20, 14, 28, fort[0]).line(14, 28, 12, 36, fort[0]).line(116, 40, 112, 50, fort[0]);

        for y in 26..48 {
            for x in 49..80 {
                let fx = x as f32 + 0.5 - ax;
                let fy = (y as f32 + 0.5 - ay).min(0.0);
                if (fx * fx + fy * fy).sqrt() >= r - 0.5 {
                    continue;
                }
                let bar = (x - 51) % 5 == 0;
                let bent = x == 66 && y > 38;
                let snapped = x == 61 && y > 40;
                let rail = matches!(y, 31 | 38 | 44);
                if (bar && !snapped && !bent) || (x == 67 && y > 38 && y < 47) {
                    cv.set(x, y, if y >= 45 { iron[3] } else { iron[2] });
                    cv.set(x + 1, y, iron[1]);
                } else if rail {
                    cv.set(x, y, iron[2]);
                    cv.set(x, y + 1, iron[0]);
                }
            }
        }
        for x in [51, 56, 71, 76] {
            cv.set(x, 48, iron[3]);
            cv.set(x, 49, iron[2]);
        }

        for (bx, len, torn) in [(15, 34, 0), (101, 26, 1)] {
            cv.rect(bx - 2, 16, 16, 2, c(0x5A3A20)).rect(bx - 2, 16, 16, 1, c(0x8A6038));
            for x in bx..bx + 12 {
                let end =
                    18 + len - (noise::hash(x, torn, 58) % 7) as i32 - if torn == 1 && x > bx + 7 { 8 } else { 0 };
                for y in 18..end {
                    let t = x - bx;
                    let col = if t == 0 || t == 1 {
                        banner[3]
                    } else if t >= 10 || (y + t) % 9 == 0 {
                        banner[1]
                    } else {
                        banner[2]
                    };
                    cv.set(x, y, col);
                }
                cv.set(x, end, banner[0]);
            }
            cv.rect(bx + 1, 19, 10, 1, gold[1]);
            cv.circle(bx as f32 + 6.0, 29.0, 3.2, gold[1]).circle(bx as f32 + 6.0, 29.0, 1.6, banner[1]);
            for i in 0..4 {
                let a = i as f32 * PI / 2.0 + PI / 4.0;
                cv.set(bx + 6 + (a.cos() * 4.6).round() as i32, 29 + (a.sin() * 4.6).round() as i32, gold[0]);
            }
        }

        cv.shade(|x, y, col| {
            if col == palette::OUTLINE || y > 50 {
                return col;
            }
            let burn = noise::value(x as f32 / 6.0, y as f32 / 5.0, 63);
            if burn > 0.6 && noise::hash01(x, y, 64) < (burn - 0.6) * 4.0 { col.lerp(soot, 0.5) } else { col }
        });
        cv.shade(|_, y, col| if y > 58 { col.lerp(fort[0], (y - 58) as f32 * 0.07) } else { col });
        for (x, y, r) in [
            (3.0, 61.0, 3.0),
            (44.0, 60.0, 2.5),
            (84.0, 61.0, 2.8),
            (124.0, 60.5, 3.2),
            (36.0, 62.0, 1.8),
            (93.0, 62.0, 2.0),
        ] {
            cv.circle(x, y, r, fort[2]);
            cv.set(x as i32 - 1, (y - r) as i32 + 1, fort[3]);
        }
        cv.outline();
        cv
    })
}

/// The Sky Altar: a round marble dais trimmed in gold and wreathed in cloud, an altar stone before a tall socket pillar,
/// and two flanking pillars. The three crystals and the light column are drawn by the entity, since they change with
/// the story.
pub fn sky_altar() -> TexId {
    gfx::cached("entrance_altar", || {
        let marble = [c(0x464A66), c(0x7C84A4), c(0xB8C0DA), c(0xF0F2FF)];
        let gold = [c(0x8A6420), c(0xC89A38), c(0xF0D070)];
        let cloud = [c(0x9AA8CC), c(0xD0DAF0), c(0xF6F8FF)];
        let (w, h) = (112, 80);
        let mut cv = Canvas::new(w, h);

        cv.ellipse(56.0, 68.0, 53.0, 11.0, marble[1]);
        cv.rect(3, 63, 106, 5, marble[1]);
        cv.shade(|x, y, col| {
            if y < 63 {
                col
            } else if x % 8 == 0 {
                marble[0]
            } else if x < 26 {
                marble[2]
            } else if y > 74 {
                marble[0].lerp(marble[1], 0.5)
            } else {
                col
            }
        });
        for y in 50..76 {
            for x in 2..110 {
                let (dx, dy) = ((x as f32 + 0.5 - 56.0) / 53.0, (y as f32 + 0.5 - 62.5) / 12.0);
                let d = (dx * dx + dy * dy).sqrt();
                if d >= 1.0 {
                    continue;
                }
                let col = if d > 0.92 && dy > 0.0 {
                    if dx < -0.3 { gold[2] } else { gold[1] }
                } else if (d - 0.64).abs() < 0.04 {
                    gold[0]
                } else if d > 0.92 {
                    marble[3]
                } else if (dy.atan2(dx) / PI * 8.0).fract() < 0.05 && d > 0.64 {
                    marble[1]
                } else if dx < -0.25 && dy < 0.3 {
                    marble[3].lerp(marble[2], 0.5)
                } else {
                    marble[2]
                };
                cv.set(x, y, col);
            }
        }
        cv.rect(42, 72, 28, 4, marble[2]).rect(42, 72, 28, 1, marble[3]).rect(42, 75, 28, 1, marble[0]);
        cv.rect(38, 76, 36, 4, marble[2]).rect(38, 76, 36, 1, marble[3]).rect(38, 79, 36, 1, marble[0]);
        side_light(&mut cv, (38, 72, 36, 8), &marble);

        let pillar = |cv: &mut Canvas, x: i32, top: i32, bottom: i32| {
            let (pw, cx) = (9, x + 4);
            cv.rect(x, top, pw, bottom - top, marble[2]);
            let flute = bottom - top - 8;
            cv.rect(x + 3, top + 4, 1, flute, marble[1]).rect(x + 6, top + 4, 1, flute, marble[1]);
            cv.rect(x + 2, top + 4, 1, flute, marble[3]);
            side_light(cv, (x, top, pw, bottom - top), &marble);
            cv.rect(x - 1, bottom - 4, pw + 2, 4, marble[2]).rect(x - 1, bottom - 4, pw + 2, 1, marble[3]);
            cv.rect(x - 1, bottom - 1, pw + 2, 1, marble[0]);
            cv.rect(x - 1, top, pw + 2, 3, gold[1]).rect(x - 1, top, pw + 2, 1, gold[2]).rect(
                x - 1,
                top + 2,
                pw + 2,
                1,
                gold[0],
            );
            cv.rect(x + 1, top - 2, pw - 2, 2, gold[1]).rect(x + 1, top - 2, pw - 2, 1, gold[2]);
            cv.line(cx - 4, top - 2, cx - 5, top - 8, gold[1]).line(cx + 4, top - 2, cx + 5, top - 8, gold[0]);
            cv.set(cx - 5, top - 8, gold[2]);
            cv.set(cx + 5, top - 8, gold[2]);
            cv.rect(cx, top + 6, 1, 3, c(0x9AD8FF)).rect(cx - 1, top + 7, 3, 1, c(0x9AD8FF));
        };
        pillar(&mut cv, 52, 16, 56);
        pillar(&mut cv, 12, 36, 70);
        pillar(&mut cv, 91, 36, 70);

        cv.rect(39, 42, 34, 4, marble[3]).rect(39, 45, 34, 1, marble[1]).rect(39, 42, 34, 1, gold[2]);
        cv.rect(42, 46, 28, 18, marble[2]);
        side_light(&mut cv, (42, 46, 28, 18), &marble);
        cv.rect(42, 63, 28, 1, marble[0]);
        cv.circle(56.0, 54.5, 5.5, gold[1]).circle(56.0, 54.5, 4.2, marble[1]).circle(56.0, 54.5, 2.0, c(0xBFE8FF));
        cv.set(55, 53, Rgba::WHITE);
        for (x, col) in [(46, c(0xFF8A30)), (56, c(0x4A9AF0)), (66, c(0xF8E060))] {
            cv.rect(x - 1, 48, 3, 2, col.lerp(marble[1], 0.35));
        }

        for (x, y, r) in [
            (6.0, 76.0, 4.0),
            (12.0, 73.5, 5.5),
            (20.0, 76.5, 4.5),
            (27.0, 78.0, 3.0),
            (106.0, 76.0, 4.0),
            (100.0, 73.5, 5.5),
            (92.0, 76.5, 4.5),
            (85.0, 78.0, 3.0),
        ] {
            cv.circle(x, y, r, cloud[2]);
        }
        cv.shade(|x, y, col| {
            if col != cloud[2] {
                col
            } else if y > 77 || (y > 75 && noise::hash01(x, y, 81) < 0.5) {
                cloud[0]
            } else if y > 74 {
                cloud[1]
            } else {
                col
            }
        });
        cv.outline();
        cv
    })
}

/// A great crystal for the altar sockets, white so it can be tinted to its element.
pub fn altar_crystal() -> TexId {
    gfx::cached("entrance_altar_crystal", || {
        let mut cv = Canvas::new(7, 12);
        for y in 0..12 {
            let half = if y < 4 { y as f32 * 0.85 + 0.6 } else { (11 - y) as f32 * 0.45 + 0.6 };
            for x in 0..7 {
                let dx = x as f32 + 0.5 - 3.5;
                if dx.abs() <= half {
                    let col = if dx < -0.5 {
                        Rgba::WHITE
                    } else if dx < 0.8 {
                        c(0xD8D8E4)
                    } else {
                        c(0x9A9AB0)
                    };
                    cv.set(x, y, col);
                }
            }
        }
        cv.outline_with(c(0x30304A));
        cv
    })
}

/// A soft vertical shaft of light, faded at both ends and edges.
pub fn light_column() -> TexId {
    gfx::cached("entrance_light_column", || {
        let (w, h) = (14, 72);
        let mut cv = Canvas::new(w, h);
        for y in 0..h {
            let along = (y as f32 / h as f32).min((h - y) as f32 / 10.0).min(1.0);
            for x in 0..w {
                let across = 1.0 - ((x as f32 + 0.5 - w as f32 / 2.0).abs() / (w as f32 / 2.0)).powi(2);
                let core = if (5..9).contains(&x) { 0.35 } else { 0.0 };
                cv.set(x, y, c(0xE8F4FF).with_alpha((across * 0.55 + core) * along));
            }
        }
        cv
    })
}

/// A faint puff of smoke, for wisps drifting from the Grotto.
pub fn smoke_wisp() -> TexId {
    gfx::cached("entrance_smoke", || {
        let mut cv = Canvas::new(10, 8);
        for y in 0..8 {
            for x in 0..10 {
                let d = ((x as f32 + 0.5 - 5.0) / 5.0).powi(2) + ((y as f32 + 0.5 - 4.0) / 4.0).powi(2);
                if d < 1.0 {
                    cv.set(x, y, c(0xB8B2BC).with_alpha((1.0 - d) * 0.8));
                }
            }
        }
        cv
    })
}
