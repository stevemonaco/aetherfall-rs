//! Combat effects, puzzle props and boss parts.

#![allow(dead_code)]

use std::f32::consts::{PI, TAU};

use crate::art::props::{LEAF, STONE, WOOD, tf};
use crate::core::canvas::Canvas;
use crate::core::color::{Rgba, c};
use crate::core::gfx::{self, TexId};
use crate::core::noise;
use crate::core::palette;
use crate::gameplay::element::Element;
use crate::world::map::Terrain;
use crate::world::theme::TerrainTheme;

/// A crescent sword trail pointing right, in three frames (forming, full, fading).
pub fn slash_arc(element: Element, heavy: bool) -> TexId {
    gfx::cached(&format!("slash_{}_{}", element.name(), tf(heavy)), || {
        let size = if heavy { 40 } else { 32 };
        let mut cv = Canvas::new(size * 3, size);
        let light = element.light();
        let glow = if element == Element::None { Rgba::rgb(200, 210, 240) } else { element.glow() };
        let half_size = size as f32 / 2.0;
        let r = half_size - 1.0;
        for f in 0..3 {
            let sweep = if f == 0 { 0.55 } else { 1.0 };
            let thickness = if f == 2 {
                3.0
            } else if heavy {
                6.5
            } else {
                5.0
            };
            for y in 0..size {
                for x in 0..size {
                    let (dx, dy) = (x as f32 + 0.5 - half_size, y as f32 + 0.5 - half_size);
                    let d = (dx * dx + dy * dy).sqrt();
                    let a = dy.atan2(dx);
                    let limit = 1.25 * sweep;
                    if a.abs() > limit {
                        continue;
                    }
                    let taper = 1.0 - a.abs() / limit;
                    let inner = r - thickness * taper;
                    if d > r || d < inner {
                        continue;
                    }
                    let edge = (d - inner) / (r - inner).max(0.5);
                    let color = if edge > 0.65 {
                        Rgba::WHITE
                    } else if edge > 0.3 {
                        light
                    } else {
                        glow
                    };
                    cv.set(f * size + x, y, color.mul(if f == 2 { 0.6 } else { 1.0 }));
                }
            }
        }
        cv
    })
}

/// The thrown aether bolt: a comet pointing right, tinted by element.
pub fn bolt(element: Element, charged: bool) -> TexId {
    gfx::cached(&format!("bolt_{}_{}", element.name(), tf(charged)), || {
        let (w, h) = if charged { (16, 9) } else { (11, 7) };
        let mut cv = Canvas::new(w, h);
        let glow = if element == Element::None { Rgba::rgb(190, 210, 255) } else { element.glow() };
        let cy = h as f32 / 2.0;
        for x in 0..w {
            let t = x as f32 / (w - 1) as f32;
            let half = (h as f32 / 2.0 - 0.5) * t.powf(0.7);
            for y in 0..h {
                let dy = (y as f32 + 0.5 - cy).abs();
                if dy > half + 0.3 {
                    continue;
                }
                let col = if t > 0.75 && dy < half * 0.6 {
                    Rgba::WHITE
                } else if t > 0.45 {
                    element.light()
                } else {
                    glow.mul(0.4 + t)
                };
                cv.set(x, y, col);
            }
        }
        cv.circle(w as f32 - 3.5, cy, if charged { 3.2 } else { 2.4 }, Rgba::WHITE);
        cv
    })
}

pub fn ring() -> TexId {
    gfx::cached("ring", || {
        let mut cv = Canvas::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                let (fx, fy) = (x as f32 - 15.5, y as f32 - 15.5);
                let d = (fx * fx + fy * fy).sqrt();
                if d <= 15.5 && d > 13.5 {
                    cv.set(x, y, Rgba::WHITE);
                }
            }
        }
        cv
    })
}

/// A soft radial falloff used to paint pools of light.
pub fn light_blob() -> TexId {
    gfx::cached("light_blob", || {
        let mut cv = Canvas::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                let (fx, fy) = (x as f32 - 31.5, y as f32 - 31.5);
                let d = (fx * fx + fy * fy).sqrt() / 32.0;
                if d >= 1.0 {
                    continue;
                }
                let f = 1.0 - d;
                let f = f * f * (3.0 - 2.0 * f);
                // White with the falloff only in alpha (the straight-alpha form of premultiplied (v, v, v, v)).
                cv.set(x, y, Rgba::new(255, 255, 255, (f * 255.0) as u8));
            }
        }
        cv
    })
}

/// A rift: a swirling violet tear with a black core, four frames of 32x40.
pub fn rift_portal() -> TexId {
    portal("rift", [c(0x2A0A40), c(0x5A1E8A), c(0x9A4AE0), c(0xE0B0FF)], c(0x08020E))
}

/// The golden portal of a Trial Rift.
pub fn trial_portal() -> TexId {
    portal("trial_rift", [c(0x4A2A06), c(0x9A6A12), c(0xE8B440), c(0xFFF0B0)], c(0x120A02))
}

/// Four 32x40 frames of a swirling portal from a dark-to-light colour ramp around a dark core.
fn portal(key: &str, ramp: [Rgba; 4], core: Rgba) -> TexId {
    gfx::cached(key, || {
        let mut cv = Canvas::new(128, 40);
        for f in 0..4 {
            for y in 0..40 {
                for x in 0..32 {
                    let (dx, dy) = ((x as f32 + 0.5 - 16.0) / 12.0, (y as f32 + 0.5 - 20.0) / 18.0);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > 1.0 {
                        continue;
                    }
                    let a = dy.atan2(dx) + f as f32 * PI / 8.0 + d * 4.0;
                    let swirl = ((a * 3.0).sin() + 1.0) / 2.0;
                    let color = if d < 0.35 {
                        core
                    } else if d < 0.5 {
                        ramp[0]
                    } else if swirl > 0.6 {
                        if d > 0.85 { ramp[3] } else { ramp[2] }
                    } else {
                        ramp[1]
                    };
                    cv.set(f * 32 + x, y, color);
                }
            }
        }
        cv
    })
}

/// A small glowing spirit in two frames of 12x14, shaped for each soul.
pub fn spirit(id: &str, glow: Rgba) -> TexId {
    gfx::cached(&format!("spirit_{id}"), || {
        let mut cv = Canvas::new(24, 14);
        let light = glow.lerp(Rgba::WHITE, 0.6);
        let dark = glow.lerp(palette::OUTLINE, 0.5);
        for f in 0..2 {
            let ox = f * 12;
            match id {
                "lantern" => {
                    cv.rect(ox + 4, 2, 4, 1, dark).rect(ox + 3, 3, 6, 7, glow).rect(ox + 4, 4, 4, 5, light).rect(
                        ox + 4,
                        10,
                        4,
                        1,
                        dark,
                    );
                    cv.set(ox + 5, 1 - f, dark);
                }
                "ember" => {
                    for y in 1..13 {
                        let half = ((y - 1) / 3 - if y > 9 { y - 9 } else { 0 }).max(0);
                        cv.rect(
                            ox + 6 - half + if y < 4 { f } else { 0 },
                            y,
                            half * 2 + 1,
                            1,
                            if y < 6 { light } else { glow },
                        );
                    }
                }
                "guardian" => {
                    cv.ellipse((ox + 6) as f32, 7.0, 5.0, 5.5, glow)
                        .ellipse((ox + 6) as f32, 7.0, 3.0, 3.5, light)
                        .rect(ox + 5, 4, 2, 6, Rgba::WHITE);
                }
                "stormling" => {
                    cv.circle((ox + 6) as f32, 7.0, 4.5, glow).circle((ox + 6) as f32, 7.0, 2.5, light);
                    for (dx, dy) in [(-5, -1), (5, 1), (0, -6), (-1, 6)] {
                        cv.set(ox + 6 + dx + f, 7 + dy, light);
                    }
                }
                _ => {
                    cv.ellipse((ox + 6) as f32, 6.0, 4.5, 5.0, glow).ellipse((ox + 6) as f32, 5.0, 3.0, 3.5, light);
                    cv.rect(ox + 3 + f, 10, 2, 3, glow).rect(ox + 7 - f, 10, 2, 2, glow);
                }
            }
            cv.set(ox + 7, 6, palette::OUTLINE);
            cv.set(ox + 5, 6, palette::OUTLINE);
        }
        cv.outline_with(glow.lerp(palette::OUTLINE, 0.75));
        cv
    })
}

/// A rock tile matching a theme's cliffs, for hidden walls.
pub fn secret_rock(theme: &str) -> TexId {
    gfx::cached(&format!("secret_{theme}"), || {
        let ramp = TerrainTheme::get(theme).ramp(Terrain::Rock).to_vec();
        let mut cv = Canvas::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let w = noise::worley(x as f32, y as f32, 7.0, 67);
                let lit = (x as f32 - w.center.x) + (y as f32 - w.center.y);
                let col = if w.f2 - w.f1 < 1.1 {
                    ramp[0]
                } else if lit < -2.5 {
                    ramp[3]
                } else if lit > 2.5 {
                    ramp[1]
                } else {
                    ramp[2]
                };
                cv.set(x, y, col);
            }
        }
        cv
    })
}

/// A tile of bridge planks laid over water; boards run across the direction of travel.
pub fn planks(horizontal: bool) -> TexId {
    gfx::cached(&format!("planks_{}", tf(horizontal)), || {
        let mut cv = Canvas::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let (along, across) = if horizontal { (x, y) } else { (y, x) };
                let col = if across == 0 || across == 15 {
                    WOOD[0]
                } else if across == 1 || across == 14 {
                    WOOD[1]
                } else if along % 4 == 0 {
                    WOOD[0]
                } else if along % 4 == 1 {
                    WOOD[3]
                } else if noise::hash01(x, y, 251) < 0.06 {
                    WOOD[1]
                } else {
                    WOOD[2]
                };
                cv.set(x, y, col);
            }
        }
        cv
    })
}

/// A faceted gem set into a boss, coloured by its element.
pub fn boss_gem(color: Rgba) -> TexId {
    gfx::cached(&format!("gem_{}", color.packed()), || {
        let mut cv = Canvas::new(10, 10);
        let dark = color.lerp(palette::OUTLINE, 0.5);
        let light = color.lerp(Rgba::WHITE, 0.6);
        cv.pattern(
            0,
            0,
            "...##...|..#ll#..|.#lmmd#.|#lmmmdd#|#mmmddd#|.#mddd#.|..#dd#..|...##...",
            &[('#', palette::OUTLINE), ('l', light), ('m', color), ('d', dark)],
        );
        cv
    })
}

/// A serpent fin, 10x12, flared to one side.
pub fn serpent_fin(right: bool) -> TexId {
    gfx::cached(&format!("fin_{}", tf(right)), || {
        let mut cv = Canvas::new(10, 12);
        let fin = [c(0x1E5A6A), c(0x2E8A9A), c(0x5AC0C0), c(0xC8F0E8)];
        for y in 0..12i32 {
            let span = (9 - (y - 4).abs() * 2 + if y < 4 { 0 } else { 1 }).max(1);
            for i in 0..span {
                let x = if right { i } else { 9 - i };
                let col = if i == span - 1 {
                    fin[3]
                } else if (i + y) % 3 == 0 {
                    fin[0]
                } else if i > span / 2 {
                    fin[2]
                } else {
                    fin[1]
                };
                cv.set(x, y, col);
            }
        }
        cv.outline_with(c(0x0A2A30));
        cv
    })
}

/// A patched canvas tent with its flap open.
pub(crate) fn tent_canvas(w: i32, h: i32) -> Canvas {
    let mut cv = Canvas::new(w, h);
    let cloth = [c(0x5A4A3A), c(0x8A7458), c(0xB09A78), c(0xD0BC98)];
    for y in 2..h {
        let t = (y - 2) as f32 / (h - 3) as f32;
        let half = (2.0 + t * (w / 2 - 2) as f32) as i32;
        for x in w / 2 - half..=w / 2 + half {
            let col = if x < w / 2 - half / 3 {
                cloth[3]
            } else if x > w / 2 + half / 2 {
                cloth[1]
            } else {
                cloth[2]
            };
            cv.set(x, y, col);
        }
    }
    for y in h / 2..h {
        let half = (y - h / 2) / 2 + 1;
        for x in w / 2 - half..=w / 2 + half {
            cv.set(x, y, c(0x1E1814));
        }
    }
    cv.rect(w / 2 - 1, 0, 2, 4, WOOD[1]).rect(w / 2 - 6, h / 3, 5, 3, c(0x8A5A3A));
    cv.outline();
    cv
}

/// A striped lighthouse tower on a stone base, taller than its footprint.
pub(crate) fn lighthouse_canvas(w: i32, h: i32) -> Canvas {
    let mut cv = Canvas::new(w, h);
    let (white, shade, red, red_dark) = (c(0xE8E4F0), c(0xA8A4B8), c(0xC84A3A), c(0x8A2A22));
    cv.rect(2, h - 12, w - 4, 12, STONE[2]).rect(2, h - 12, w - 4, 2, STONE[3]).rect(2, h - 2, w - 4, 2, STONE[1]);
    let (top, bottom) = (18, h - 12);
    for y in top..bottom {
        let t = (y - top) as f32 / (bottom - top) as f32;
        let half = (7.0 + t * 6.0) as i32;
        let band = (y - top) / 8 % 2 == 1;
        for x in w / 2 - half..=w / 2 + half {
            let lit = x < w / 2 - half / 3;
            let col = match (band, lit) {
                (true, true) => red,
                (true, false) => red_dark,
                (false, true) => white,
                (false, false) => shade,
            };
            cv.set(x, y, col);
        }
    }
    cv.rect(w / 2 - 3, bottom - 10, 6, 10, WOOD[1]).rect(w / 2 - 2, bottom - 9, 4, 9, WOOD[2]);
    cv.rect(w / 2 - 10, top - 2, 20, 3, c(0x3A3A4A));
    cv.rect(w / 2 - 6, 6, 12, 10, c(0xFFE890)).rect(w / 2 - 4, 8, 8, 6, Rgba::WHITE);
    cv.rect(w / 2 - 7, 5, 14, 1, c(0x3A3A4A)).rect(w / 2 - 1, 6, 2, 10, c(0x3A3A4A));
    for y in 0..6 {
        cv.rect(w / 2 - (6 - y), y, (6 - y) * 2, 1, red);
    }
    cv.outline();
    cv
}

/// Broken masonry filling what used to be a hole.
pub fn rubble() -> TexId {
    gfx::cached("rubble", || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(0, 0, 16, 16, STONE[1]);
        for (x, y, r) in [(4.0, 4.0, 3.2), (11.0, 5.0, 3.6), (6.0, 11.0, 3.8), (12.0, 12.0, 2.8)] {
            cv.circle(x, y, r, STONE[2]);
        }
        cv.shade(|x, y, col| if col == STONE[2] && noise::hash01(x, y, 261) < 0.25 { STONE[3] } else { col });
        cv.line(2, 8, 7, 7, STONE[0]).line(9, 9, 14, 8, STONE[0]);
        cv
    })
}

/// A pushable block of Concord masonry.
pub fn stone_block() -> TexId {
    gfx::cached("stone_block", || {
        let mut cv = Canvas::new(16, 18);
        cv.rect(1, 1, 14, 6, STONE[3])
            .rect(1, 7, 14, 10, STONE[2])
            .rect(1, 7, 14, 1, STONE[1])
            .rect(13, 7, 2, 10, STONE[1]);
        cv.line(3, 3, 12, 3, STONE[2]).line(5, 10, 10, 13, STONE[1]);
        cv.rect(6, 9, 4, 4, c(0xC8A0F0)).rect(7, 10, 2, 2, Rgba::WHITE);
        cv.outline();
        cv
    })
}

/// A floating memory shard, 12x22.
pub fn memory_shard() -> TexId {
    gfx::cached("memory_shard", || {
        let mut cv = Canvas::new(12, 22);
        let glass = [c(0x6A4A9A), c(0xA88AE0), c(0xE0D0FF), Rgba::WHITE];
        for y in 0..22 {
            let half = if y < 9 { y * 5 / 9 } else { (21 - y) * 5 / 12 };
            for x in 6 - half..6 + half {
                let col = if x < 6 {
                    if x < 6 - half / 2 { glass[3] } else { glass[2] }
                } else if x > 6 + half / 2 {
                    glass[0]
                } else {
                    glass[1]
                };
                cv.set(x, y, col);
            }
        }
        cv.outline_with(c(0x2A1A40));
        cv
    })
}

/// An aether seal, two frames of 16x16.
pub fn aether_seal_art() -> TexId {
    gfx::cached("aether_seal", || {
        let mut cv = Canvas::new(32, 16);
        for f in 0..2 {
            let ox = f * 16;
            cv.rect(ox, 0, 16, 16, c(0x2A1A40)).rect(ox + 1, 1, 14, 14, c(0x4A2E70));
            cv.circle((ox + 8) as f32, 8.0, 5.5, if f == 0 { c(0xC890F0) } else { c(0xF0C8FF) })
                .circle((ox + 8) as f32, 8.0, 3.5, c(0x4A2E70))
                .circle((ox + 8) as f32, 8.0, 1.5, Rgba::WHITE);
            for (dx, dy) in [(0, -7), (0, 6), (-7, 0), (6, 0)] {
                cv.set(ox + 8 + dx, 8 + dy, c(0xF0C8FF));
            }
        }
        cv.outline_with(c(0x14081E));
        cv
    })
}

/// The Concord Warden: a hulking automaton with a glowing core, two frames of 40x50 (idle, bracing).
pub fn warden() -> TexId {
    gfx::cached("warden", || {
        let plate = [c(0x2A2A3A), c(0x4E5266), c(0x7A8098), c(0xB4BACC)];
        let gold = [c(0x6A4A1A), c(0xA8802E), c(0xE0BC50), c(0xFFE8A0)];
        let mut cv = Canvas::new(80, 50);
        for f in 0..2 {
            let mut frame = Canvas::new(40, 50);
            let lift = f;
            frame
                .rect(11, 38, 7, 11, plate[1])
                .rect(22, 38, 7, 11, plate[1])
                .rect(10, 46, 9, 3, plate[0])
                .rect(21, 46, 9, 3, plate[0]);
            frame.ellipse(20.0, (26 - lift) as f32, 13.0, 13.0, plate[2]).rect(8, 24 - lift, 24, 15, plate[2]);
            frame.shade(|x, _, col| {
                if col != plate[2] {
                    col
                } else if x < 14 {
                    plate[3]
                } else if x > 27 {
                    plate[1]
                } else {
                    plate[2]
                }
            });
            frame.rect(10, 34 - lift, 20, 2, gold[1]).rect(10, 34 - lift, 20, 1, gold[2]);
            frame
                .ellipse(20.0, (26 - lift) as f32, 5.0, 5.0, c(0x0E1A2A))
                .ellipse(20.0, (26 - lift) as f32, 3.2, 3.2, c(0x8AE0FF))
                .rect(19, 24 - lift, 2, 2, Rgba::WHITE);
            frame.ellipse(20.0, (10 - lift) as f32, 7.0, 6.0, plate[2]).rect(15, 9 - lift, 10, 2, c(0x0E1A2A)).rect(
                16,
                9 - lift,
                8,
                1,
                c(0xFF6A4A),
            );
            frame.rect(13, 4 - lift, 14, 2, gold[1]);
            let arm_y = if f == 1 { 14 } else { 20 };
            frame.ellipse(4.0, (arm_y + 4) as f32, 4.0, 8.0, plate[1]).ellipse(
                36.0,
                (arm_y + 4) as f32,
                4.0,
                8.0,
                plate[1],
            );
            frame.rect(1, arm_y + 10, 7, 5, gold[1]).rect(32, arm_y + 10, 7, 5, gold[1]);
            frame.outline_with(c(0x0A0A12));
            cv.blit(&frame, f * 40, 0);
        }
        cv
    })
}

pub fn disc() -> TexId {
    gfx::cached("disc", || Canvas::new(32, 32).circle(16.0, 16.0, 16.0, Rgba::WHITE).take())
}

pub fn scorch() -> TexId {
    gfx::cached("scorch", || {
        let mut cv = Canvas::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let n = noise::value(x as f32 / 4.0, y as f32 / 4.0, 211);
                if n < 0.75 {
                    let base = if n < 0.3 { Rgba::rgb(30, 22, 18) } else { Rgba::rgb(58, 46, 36) };
                    cv.set(x, y, base.mul(0.75));
                }
                if noise::hash01(x, y, 213) < 0.05 {
                    cv.set(x, y, Rgba::rgb(120, 110, 100).mul(0.8));
                }
            }
        }
        cv
    })
}

/// Three variants of tall grass tufts, 16x20 each, drawn standing up out of their tile.
pub fn tall_grass() -> TexId {
    gfx::cached("tall_grass", || {
        let mut cv = Canvas::new(48, 20);
        for v in 0..3 {
            for blade in 0..9 {
                let bx = v * 16 + 1 + (noise::hash(blade, v, 221) % 14) as i32;
                let height = 7 + (noise::hash(blade, v, 223) % 7) as i32;
                let lean = (noise::hash(blade, v, 225) % 3) as i32 - 1;
                for i in 0..height {
                    let x = bx + if i > height / 2 { lean } else { 0 };
                    let y = 19 - i;
                    let color = if i > height - 3 {
                        LEAF[4]
                    } else if i > height / 2 {
                        LEAF[3]
                    } else {
                        LEAF[2]
                    };
                    cv.set(x, y, color);
                    if i < height / 2 {
                        cv.set(x + 1, y, LEAF[1]);
                    }
                }
            }
        }
        cv.outline_with(LEAF[0]);
        cv
    })
}

pub fn flames() -> TexId {
    gfx::cached("flames", || {
        let fire = [c(0x8A1808), c(0xE04818), c(0xFF9A30), c(0xFFE890)];
        let mut cv = Canvas::new(64, 22);
        for f in 0..4 {
            for tongue in 0..4 {
                let cx = f * 16 + 2 + tongue * 4;
                let h = 8 + (noise::hash(tongue, f, 231) % 12) as i32;
                for y in 0..h {
                    let t = y as f32 / h as f32;
                    let half = ((1.0 - t) * 2.5) as i32;
                    let sway = ((t * 4.0 + f as f32 + tongue as f32).sin() * 1.2) as i32;
                    let color = if t < 0.3 {
                        fire[1]
                    } else if t < 0.6 {
                        fire[2]
                    } else {
                        fire[3]
                    };
                    for x in -half..=half {
                        cv.set(cx + x + sway, 21 - y, if x.abs() == half && half > 0 { fire[1] } else { color });
                    }
                }
            }
        }
        cv.outline_with(fire[0].mul(0.8));
        cv
    })
}

pub fn spark_tile() -> TexId {
    gfx::cached("spark_tile", || {
        let mut cv = Canvas::new(16, 16);
        for i in 0..6 {
            let x = (noise::hash(i, 0, 241) % 14) as i32 + 1;
            let y = (noise::hash(i, 1, 241) % 14) as i32 + 1;
            cv.set(x, y, Rgba::WHITE);
            for (sx, sy) in [(x, y + 1), (x, y - 1), (x - 1, y), (x + 1, y)] {
                cv.set(sx, sy, c(0xFFF0A0));
            }
        }
        cv
    })
}

pub fn chain_link() -> TexId {
    gfx::cached("chain_link", || Canvas::new(3, 3).pattern(0, 0, ".#.|#.#|.#.", &[('#', Rgba::WHITE)]).take())
}

pub fn hook() -> TexId {
    gfx::cached("hook", || {
        Canvas::new(7, 7)
            .pattern(
                0,
                0,
                "..##...|.#ww#..|#wwwwl#|#wwwwl#|.#ww#..|..##...|.......",
                &[('#', c(0x3A3A5A)), ('w', c(0xE0E8FF)), ('l', c(0x9AF0FF))],
            )
            .take()
    })
}

/// A pushable block of ice, 16x18 with its top face visible.
pub fn ice_block() -> TexId {
    gfx::cached("ice_block", || {
        let ice = [c(0x2A5A90), c(0x6AA8E0), c(0xA8DCF8), c(0xF0FCFF)];
        let mut cv = Canvas::new(16, 18);
        cv.rect(1, 1, 14, 6, ice[3]).rect(1, 7, 14, 10, ice[2]);
        cv.rect(1, 7, 14, 1, ice[1]).rect(13, 7, 2, 10, ice[1]).rect(1, 1, 1, 16, Rgba::WHITE);
        cv.line(4, 9, 7, 14, ice[3]).line(9, 3, 12, 5, ice[2]);
        cv.outline_with(ice[0]);
        cv
    })
}

/// An enemy's shield; its emblem shows the element that shatters it in one blow.
pub fn enemy_shield(bane: Element) -> TexId {
    gfx::cached(&format!("enemy_shield_{}", bane.name()), || {
        let mut cv = Canvas::new(8, 10);
        let emblem = if bane == Element::None { c(0xC84A3A) } else { bane.glow() };
        cv.rect(0, 0, 8, 6, STONE[2]).ellipse(4.0, 6.0, 4.0, 4.0, STONE[2]).rect(3, 1, 2, 7, emblem);
        cv.rect(0, 0, 8, 1, STONE[3]);
        cv.outline();
        cv
    })
}

pub fn guard_shield() -> TexId {
    gfx::cached("guard_shield", || {
        let mut cv = Canvas::new(10, 12);
        cv.rect(1, 1, 8, 6, c(0x5A7ECC)).ellipse(5.0, 7.0, 4.0, 4.5, c(0x5A7ECC));
        cv.rect(4, 2, 2, 8, c(0xE8C050)).rect(2, 4, 6, 2, c(0xE8C050));
        cv.rect(1, 1, 8, 1, c(0x8AAEEC));
        cv.outline();
        cv
    })
}

pub fn turbine_base() -> TexId {
    gfx::cached("turbine_base", || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(6, 0, 4, 13, STONE[2])
            .rect(6, 0, 1, 13, STONE[3])
            .rect(3, 12, 10, 4, STONE[1])
            .rect(3, 12, 10, 1, STONE[3]);
        cv.rect(7, 4, 2, 2, c(0x6AE0C8));
        cv.outline();
        cv
    })
}

pub fn turbine_blades() -> TexId {
    gfx::cached("turbine_blades", || {
        let mut cv = Canvas::new(48, 16);
        for f in 0..3 {
            for b in 0..3 {
                let a = f as f32 * TAU / 9.0 + b as f32 * TAU / 3.0;
                for r in 1..7 {
                    // MathF.Round rounds half to even.
                    let x = f * 16 + 8 + (a.cos() * r as f32).round_ties_even() as i32;
                    let y = 8 + (a.sin() * r as f32).round_ties_even() as i32;
                    cv.set(x, y, if r > 4 { c(0xE8E8F0) } else { c(0xB0B0C0) });
                }
            }
            cv.circle((f * 16 + 8) as f32, 8.0, 1.5, c(0x6AE0C8));
        }
        cv.outline();
        cv
    })
}

pub fn brazier() -> TexId {
    gfx::cached("brazier", || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(2, 2, 12, 5, STONE[2]).rect(2, 2, 12, 1, STONE[3]).rect(3, 3, 10, 2, c(0x2A2018));
        cv.rect(6, 7, 4, 6, STONE[1]).rect(4, 13, 8, 3, STONE[2]).rect(4, 13, 8, 1, STONE[3]);
        cv.outline();
        cv
    })
}

pub fn crate_box() -> TexId {
    gfx::cached("crate", || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(1, 1, 14, 14, WOOD[2]).rect(1, 1, 14, 2, WOOD[3]).rect(1, 13, 14, 2, WOOD[1]);
        cv.rect(1, 1, 2, 14, WOOD[1]).rect(13, 1, 2, 14, WOOD[1]);
        cv.line(3, 3, 12, 12, WOOD[1]).line(3, 12, 12, 3, WOOD[1]);
        cv.outline();
        cv
    })
}

pub fn cloud() -> TexId {
    gfx::cached("cloud", || {
        let mut cv = Canvas::new(24, 20);
        cv.circle(8.0, 12.0, 6.0, Rgba::WHITE)
            .circle(15.0, 11.0, 7.0, Rgba::WHITE)
            .circle(12.0, 7.0, 5.5, Rgba::WHITE)
            .circle(18.0, 15.0, 4.0, Rgba::WHITE)
            .circle(6.0, 15.0, 4.0, Rgba::WHITE);
        cv.shade(|_, y, _| {
            Rgba::WHITE.mul(if y < 9 {
                0.85
            } else if y > 15 {
                0.55
            } else {
                0.7
            })
        });
        cv
    })
}

pub fn vent() -> TexId {
    gfx::cached("vent", || {
        let mut cv = Canvas::new(16, 10);
        cv.ellipse(8.0, 5.0, 7.0, 4.0, c(0x3A3424)).ellipse(8.0, 5.0, 4.5, 2.5, c(0x1A160E));
        cv.set(10, 5, c(0x9AE07A));
        cv.set(6, 4, c(0x9AE07A));
        cv
    })
}

pub fn tether_post() -> TexId {
    gfx::cached("tether_post", || {
        let mut cv = Canvas::new(12, 26);
        cv.rect(4, 8, 4, 16, c(0x6C7890)).rect(4, 8, 1, 16, c(0x9AA6BC)).rect(2, 22, 8, 4, c(0x48526A));
        cv.circle(6.0, 5.0, 5.0, c(0x9AA6BC)).circle(6.0, 5.0, 2.5, Rgba::TRANSPARENT);
        cv.shade(
            |x, y, col| if (((x - 6) * (x - 6) + (y - 5) * (y - 5)) as f32) < 6.25 { Rgba::TRANSPARENT } else { col },
        );
        cv.set(6, 1, c(0x9AF0FF));
        cv.set(3, 3, c(0x9AF0FF));
        cv.outline();
        cv
    })
}

pub fn lever() -> TexId {
    gfx::cached("lever", || {
        let mut cv = Canvas::new(32, 16);
        for f in 0..2 {
            let ox = f * 16;
            cv.rect(ox + 3, 11, 10, 4, STONE[1]).rect(ox + 3, 11, 10, 1, STONE[3]);
            if f == 0 {
                cv.line(ox + 8, 11, ox + 4, 3, WOOD[2]).circle((ox + 4) as f32, 3.0, 2.0, c(0xE84040));
            } else {
                cv.line(ox + 8, 11, ox + 12, 3, WOOD[2]).circle((ox + 12) as f32, 3.0, 2.0, c(0x5AE070));
            }
        }
        cv.outline();
        cv
    })
}

/// A sword blade pointing right, with the hilt at the left edge, coloured for the current mode.
pub fn blade(mode: Element) -> TexId {
    gfx::cached(&format!("blade_{}", mode.name()), || {
        let blade = if mode == Element::None { c(0xE0E4F0) } else { mode.light() };
        let dark = if mode == Element::None { c(0x8890A8) } else { mode.dark() };
        let mut cv = Canvas::new(17, 7);
        cv.rect(0, 3, 3, 1, WOOD[2]).rect(3, 1, 1, 5, c(0xE8C050)).rect(4, 1, 1, 5, c(0xA88030));
        cv.rect(5, 2, 10, 3, blade).rect(5, 2, 10, 1, Rgba::WHITE).rect(5, 4, 10, 1, dark);
        cv.set(15, 3, blade);
        cv.set(16, 3, dark);
        cv.outline();
        cv
    })
}

/// A round emblem for an element mode, used in the HUD.
pub fn emblem(mode: Element) -> TexId {
    gfx::cached(&format!("emblem_{}", mode.name()), || {
        let mut cv = Canvas::new(18, 18);
        let glow = mode.glow();
        cv.circle(9.0, 9.0, 8.5, mode.dark())
            .circle(9.0, 9.0, 7.0, mode.dark().lerp(glow, 0.5))
            .circle(8.0, 8.0, 5.0, glow);
        let symbol = match mode {
            Element::Wind => "..##....|.#..#...|....#...|.####.#.|......#.|..####..",
            Element::Fire => "...#....|..##....|..###...|.####.#.|.######.|..####..",
            Element::Water => "...##...|..####..|.######.|.######.|..####..|........",
            Element::Thunder => "....##..|...##...|..####..|....##..|...##...|..#.....",
            Element::Aether => "...##...|.#.##.#.|..####..|.######.|..####..|.#.##.#.",
            Element::None => "...##...|..#..#..|.#....#.|.#....#.|..#..#..|...##...",
        };
        cv.pattern(5, 6, symbol, &[('#', Rgba::WHITE)]);
        cv.outline();
        cv
    })
}
