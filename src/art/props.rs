//! Procedurally drawn sprites for everything not covered by the 7Soul pack.

#![allow(dead_code)]

use std::f32::consts::{PI, TAU};

use crate::art::combat::{lighthouse_canvas, tent_canvas};
use crate::core::canvas::Canvas;
use crate::core::color::{Rgba, c};
use crate::core::gfx::{self, TexId};
use crate::core::noise;
use crate::core::palette;
use crate::gameplay::items;
use crate::gameplay::supports::{self, Group};

pub(crate) const LEAF: [Rgba; 5] = [c(0x0F2A1E), c(0x1E5130), c(0x2F7A3C), c(0x57A84A), c(0x9AD66A)];
pub(crate) const STONE: [Rgba; 4] = [c(0x2A2630), c(0x56505E), c(0x837C8A), c(0xB4AEB8)];
pub(crate) const WOOD: [Rgba; 4] = [c(0x3A2414), c(0x6A4226), c(0x965E34), c(0xC0844C)];

/// Formats a bool as "True"/"False" for texture cache keys.
pub(crate) fn tf(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

pub fn tree() -> TexId {
    gfx::cached("tree", || {
        let mut cv = Canvas::new(32, 38);
        cv.rect(13, 24, 6, 12, WOOD[2]).rect(17, 24, 2, 12, WOOD[1]).rect(13, 24, 1, 12, WOOD[3]);
        cv.rect(11, 34, 10, 2, WOOD[1]);
        for (x, y, r) in [
            (16.0, 11.0, 10.0),
            (8.5, 17.0, 7.5),
            (23.5, 17.0, 7.5),
            (16.0, 20.0, 9.0),
            (11.0, 9.0, 6.0),
            (21.0, 9.0, 6.0),
        ] {
            cv.circle(x, y, r, LEAF[2]);
        }
        cv.shade(|x, y, col| if col != LEAF[2] { col } else { canopy_shade(x, y, 5.0, 0) });
        cv.outline();
        cv
    })
}

pub fn bush() -> TexId {
    gfx::cached("bush", || {
        let mut cv = Canvas::new(16, 16);
        cv.circle(5.0, 9.0, 4.5, LEAF[2])
            .circle(11.0, 9.0, 4.5, LEAF[2])
            .circle(8.0, 6.5, 5.0, LEAF[2])
            .rect(2, 9, 12, 5, LEAF[2]);
        cv.shade(|x, y, _| canopy_shade(x, y, 3.5, 9));
        cv.outline();
        cv
    })
}

fn canopy_shade(x: i32, y: i32, cell: f32, seed: i32) -> Rgba {
    let w = noise::worley(x as f32, y as f32, cell, 300 + seed);
    if w.f2 - w.f1 < 0.9 {
        return LEAF[1];
    }
    let lit = ((x as f32 - w.center.x) + (y as f32 - w.center.y)) / cell + (y - 14) as f32 * 0.03;
    if lit < -0.45 {
        return if (w.id & 3) == 0 { LEAF[4] } else { LEAF[3] };
    }
    if lit < 0.25 { LEAF[2] } else { LEAF[1] }
}

pub fn boulder() -> TexId {
    gfx::cached("boulder", || {
        let mut cv = Canvas::new(16, 16);
        cv.ellipse(8.0, 9.0, 7.0, 6.0, STONE[2]).rect(1, 9, 14, 5, STONE[2]);
        cv.shade(|x, y, _| {
            let lit = (x - 6) as f32 * 0.5 + (y - 6) as f32;
            if lit < -1.5 {
                STONE[3]
            } else if lit < 4.0 {
                STONE[2]
            } else {
                STONE[1]
            }
        });
        cv.line(6, 5, 8, 9, STONE[1]).line(8, 9, 7, 12, STONE[1]).line(8, 9, 11, 10, STONE[1]);
        cv.outline();
        cv
    })
}

pub fn sign() -> TexId {
    gfx::cached("sign", || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(7, 9, 2, 6, WOOD[1]);
        cv.rect(2, 2, 12, 8, WOOD[2]).rect(2, 2, 12, 1, WOOD[3]).rect(2, 9, 12, 1, WOOD[1]);
        cv.rect(4, 4, 8, 1, WOOD[0]).rect(4, 6, 6, 1, WOOD[0]);
        cv.outline();
        cv
    })
}

pub fn pot() -> TexId {
    gfx::cached("pot", || {
        let clay = [c(0x5A2A1A), c(0x9A4A2A), c(0xC8703A), c(0xE8A060)];
        let mut cv = Canvas::new(16, 16);
        cv.ellipse(8.0, 10.0, 6.0, 5.0, clay[2]).rect(5, 3, 6, 3, clay[2]).rect(4, 2, 8, 2, clay[1]);
        cv.shade(|x, y, col| {
            if y < 4 {
                col
            } else if x < 6 {
                clay[3]
            } else if x > 10 {
                clay[1]
            } else {
                clay[2]
            }
        });
        cv.rect(4, 9, 8, 1, clay[1]);
        cv.outline();
        cv
    })
}

pub fn well() -> TexId {
    gfx::cached("well", || {
        let mut cv = Canvas::new(32, 32);
        cv.rect(4, 2, 2, 20, WOOD[1]).rect(26, 2, 2, 20, WOOD[1]);
        cv.rect(2, 0, 28, 6, WOOD[2]).rect(2, 0, 28, 1, WOOD[3]).rect(2, 5, 28, 1, WOOD[0]);
        for x in (4..30).step_by(4) {
            cv.rect(x, 1, 1, 4, WOOD[1]);
        }
        cv.ellipse(16.0, 22.0, 13.0, 8.0, STONE[2]);
        cv.shade(|x, y, col| {
            if col != STONE[2] {
                col
            } else if (x / 4 + y / 3) % 2 == 0 {
                STONE[2]
            } else {
                STONE[3]
            }
        });
        cv.ellipse(16.0, 20.0, 9.0, 4.0, palette::OUTLINE).ellipse(16.0, 21.0, 8.0, 3.0, c(0x2D5BB0));
        cv.rect(15, 6, 2, 10, c(0x8A7060));
        cv.outline();
        cv
    })
}

pub fn pod() -> TexId {
    gfx::cached("pod", || {
        let glass = [c(0x2A5A8A), c(0x5AA0D8), c(0xA8E0FF), c(0xE8FAFF)];
        let mut cv = Canvas::new(16, 26);
        cv.rect(1, 18, 14, 8, STONE[1]).rect(1, 18, 14, 2, STONE[3]).rect(3, 21, 10, 2, c(0x5AC0A0));
        cv.ellipse(8.0, 11.0, 6.5, 9.0, glass[1]);
        cv.shade(|x, y, col| {
            if col != glass[1] {
                col
            } else if x < 6 {
                if y < 9 { glass[3] } else { glass[2] }
            } else if x > 11 {
                glass[0]
            } else {
                glass[1]
            }
        });
        cv.ellipse(8.0, 13.0, 4.0, 6.0, c(0x1C3048));
        cv.line(4, 4, 6, 2, glass[3]);
        cv.outline();
        cv
    })
}

pub fn bed() -> TexId {
    gfx::cached("bed", || {
        let mut cv = Canvas::new(16, 26);
        cv.rect(1, 0, 14, 4, WOOD[1]).rect(1, 0, 14, 1, WOOD[3]);
        cv.rect(2, 4, 12, 20, c(0xE8E4F0)).rect(3, 5, 10, 4, Rgba::WHITE);
        cv.rect(2, 11, 12, 13, c(0xC04A5A)).rect(2, 11, 12, 2, c(0xE87A8A)).rect(2, 22, 12, 2, c(0x8A2A3A));
        cv.rect(1, 24, 2, 2, WOOD[1]).rect(13, 24, 2, 2, WOOD[1]);
        cv.outline();
        cv
    })
}

pub fn table() -> TexId {
    gfx::cached("table", || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(1, 3, 14, 8, WOOD[2]).rect(1, 3, 14, 2, WOOD[3]).rect(1, 10, 14, 1, WOOD[1]);
        cv.rect(2, 11, 2, 4, WOOD[1]).rect(12, 11, 2, 4, WOOD[1]);
        cv.rect(9, 1, 3, 3, c(0xE8E4F0)).rect(10, 2, 1, 1, c(0x8A5A3A));
        cv.outline();
        cv
    })
}

pub fn shelf() -> TexId {
    gfx::cached("shelf", || {
        let mut cv = Canvas::new(16, 26);
        cv.rect(0, 0, 16, 26, WOOD[1]).rect(0, 0, 16, 2, WOOD[2]);
        let books = [c(0xC04A3A), c(0x3A6AC0), c(0x5AA845), c(0xE8C050), c(0x8A4AA0)];
        for shelf in 0..3 {
            let y = 3 + shelf * 8;
            cv.rect(1, y, 14, 6, WOOD[0]);
            for x in (1..15).step_by(2) {
                let lean = (noise::hash(x, shelf, 4) % 2) as i32;
                cv.rect(x, y + lean, 2, 6 - lean, books[(noise::hash(x, shelf, 9) % 5) as usize]);
            }
            cv.rect(0, y + 6, 16, 2, WOOD[2]);
        }
        cv.outline();
        cv
    })
}

pub fn counter() -> TexId {
    gfx::cached("counter", || {
        let mut cv = Canvas::new(16, 18);
        cv.rect(0, 0, 16, 6, WOOD[3]).rect(0, 5, 16, 1, WOOD[1]);
        cv.rect(0, 6, 16, 12, WOOD[2]);
        for x in (3..16).step_by(5) {
            cv.rect(x, 7, 1, 11, WOOD[1]);
        }
        cv.outline();
        cv
    })
}

pub fn plant() -> TexId {
    gfx::cached("plant", || {
        let mut cv = Canvas::new(16, 18);
        cv.rect(4, 11, 8, 6, c(0xC8703A)).rect(3, 10, 10, 2, c(0xE8A060));
        cv.ellipse(5.0, 7.0, 3.5, 4.0, LEAF[2])
            .ellipse(11.0, 7.0, 3.5, 4.0, LEAF[3])
            .ellipse(8.0, 4.0, 3.0, 4.0, LEAF[3]);
        cv.line(8, 10, 8, 4, LEAF[1]);
        cv.outline();
        cv
    })
}

pub fn rug() -> TexId {
    gfx::cached("rug", || {
        let mut cv = Canvas::new(48, 32);
        cv.rect(0, 0, 48, 32, c(0x8A2A3A)).rect(3, 3, 42, 26, c(0xC04A5A)).rect(6, 6, 36, 20, c(0x8A2A3A)).rect(
            8,
            8,
            32,
            16,
            c(0xE8C050),
        );
        cv.rect(10, 10, 28, 12, c(0xC04A5A));
        cv
    })
}

/// The Ember Warlock boss: two 32x40 frames of a hooded fire mage with a staff.
pub fn warlock() -> TexId {
    gfx::cached("warlock", || {
        let robe = [c(0x2A0A10), c(0x5A1420), c(0x8A2228), c(0xB83A30)];
        let flame = [c(0xC83A20), c(0xFF8A30), c(0xFFD060), c(0xFFF6C0)];
        let mut cv = Canvas::new(64, 40);
        for f in 0..2 {
            let ox = f * 32;
            let mut frame = Canvas::new(32, 40);

            for y in 14..39 {
                let t = (y - 14) as f32 / 25.0;
                let half = (6.0 + t * 8.0) as i32;
                let sway = if f == 1 && y > 30 { 1 } else { 0 };
                for x in 16 - half + sway..=15 + half + sway {
                    let fold = (x - sway + 32) % 5;
                    let mut col = if x < 13 {
                        robe[3]
                    } else if x > 19 {
                        robe[1]
                    } else {
                        robe[2]
                    };
                    if fold == 0 && y > 18 {
                        col = robe[1];
                    }
                    frame.set(x, y, col);
                }
            }
            for x in 3..29 {
                if frame.is_set(x, 37) {
                    frame.set(x, 37, if (x + f) % 3 == 0 { flame[2] } else { flame[1] });
                }
            }

            frame.ellipse(8.0, 22.0, 3.0, 6.0, robe[2]).ellipse(24.0, 21.0, 3.0, 6.0, robe[1]);
            frame.ellipse(16.0, 11.0, 7.0, 8.0, robe[1]);
            frame.shade(|x, y, col| if col == robe[1] && y < 17 && x < 14 { robe[2] } else { col });
            frame.ellipse(16.0, 13.0, 4.0, 4.5, c(0x0A0408));
            frame.set(14, 13, flame[3]);
            frame.set(18, 13, flame[3]);
            frame.set(14, 14, flame[1]);
            frame.set(18, 14, flame[1]);

            for i in 0..5 {
                let fx = 11 + i * 2;
                let height = (if i % 2 == f { 5 } else { 3 }) + if i == 2 { 2 } else { 0 };
                for y in 0..height {
                    frame.set(fx, 4 - y, if y < height / 2 { flame[1] } else { flame[2] });
                }
                frame.set(fx, 5, flame[0]);
            }

            frame.line(27, 8, 27, 38, WOOD[1]).line(28, 9, 28, 38, WOOD[0]);
            frame.circle(27.5, 6.5, 3.2, flame[1]).circle(27.0, 6.0, 1.8, if f == 0 { flame[2] } else { flame[3] });
            frame.circle(6.0, 27.0, 2.6, flame[1]).circle(6.0, 26.5, 1.4, flame[3]);
            frame.ellipse(24.0, 27.0, 2.0, 1.5, c(0xE8B090));

            frame.outline_with(c(0x14040A));
            cv.blit(&frame, ox, 0);
        }
        cv
    })
}

/// A straw training dummy on a post, 18x26, feet at the bottom centre.
pub fn training_dummy() -> TexId {
    gfx::cached("training_dummy", || {
        let mut cv = Canvas::new(18, 26);
        let wood = [c(0x5A3A1E), c(0x8A5A2E)];
        let straw = [c(0x9A7A3A), c(0xD6BA70), c(0xF0DCA0)];
        cv.rect(8, 8, 2, 18, wood[0]).rect(8, 8, 1, 18, wood[1]);
        cv.rect(1, 11, 16, 2, wood[0]).rect(1, 11, 16, 1, wood[1]);
        cv.ellipse(9.0, 15.5, 4.5, 5.5, straw[0]).ellipse(8.5, 15.0, 3.5, 4.5, straw[1]);
        cv.line(6, 13, 11, 18, straw[0]).line(11, 13, 6, 18, straw[0]);
        cv.circle(9.0, 5.0, 4.0, straw[1]).circle(8.5, 4.5, 2.5, straw[2]);
        cv.set(7, 4, wood[0]);
        cv.set(10, 4, wood[0]);
        cv.line(7, 7, 10, 7, wood[0]);
        cv.outline();
        cv
    })
}

pub fn save_crystal() -> TexId {
    gfx::cached("save_crystal", || {
        let mut cv = Canvas::new(12, 20);
        let ice = [c(0x1C3678), c(0x3F86D8), c(0x8FD8FF), c(0xF0FCFF)];
        for y in 0..20 {
            let half = if y < 8 { y * 6 / 8 } else { (19 - y) * 6 / 11 };
            for x in 6 - half..6 + half {
                let col = if x < 6 {
                    if x < 6 - half / 2 { ice[3] } else { ice[2] }
                } else if x > 6 + half / 2 {
                    ice[0]
                } else {
                    ice[1]
                };
                cv.set(x, y, col);
            }
        }
        cv.outline_with(c(0x10183A));
        cv
    })
}

/// A house of the given size in tiles; the door sits on the bottom row at `door_tile`.
pub fn house(tiles_w: i32, tiles_h: i32, door_tile: i32, roof: &str) -> TexId {
    gfx::cached(&format!("house_{tiles_w}_{tiles_h}_{door_tile}_{roof}"), || {
        if roof == "lighthouse" {
            return lighthouse_canvas(tiles_w * 16, tiles_h * 16 + 40);
        }
        if roof == "tent" {
            return tent_canvas(tiles_w * 16, tiles_h * 16);
        }
        let (w, h) = (tiles_w * 16, tiles_h * 16);
        let mut cv = Canvas::new(w, h);
        let roof_ramp = match roof {
            "blue" => [c(0x1A2448), c(0x2E4A8A), c(0x4A72C0), c(0x7AA4E8)],
            "green" => [c(0x163420), c(0x2A6038), c(0x44904E), c(0x76C070)],
            "slate" => [c(0x22202C), c(0x46424E), c(0x6A6676), c(0x9894A4)],
            _ => [c(0x4A1A1A), c(0x8A3028), c(0xC04A34), c(0xE87A50)],
        };
        let plaster = [c(0x6A5440), c(0xB8A080), c(0xE0CCA4), c(0xF6EAC8)];

        let wall_top = h - 22;
        cv.rect(2, wall_top, w - 4, h - wall_top, plaster[2]);
        cv.rect(2, h - 3, w - 4, 3, STONE[2]).rect(2, h - 3, w - 4, 1, STONE[3]);
        cv.rect(2, wall_top, 2, h - wall_top - 3, WOOD[1]).rect(w - 4, wall_top, 2, h - wall_top - 3, WOOD[1]);
        cv.rect(2, wall_top, w - 4, 2, WOOD[1]);

        for t in 0..tiles_w {
            if t == door_tile {
                continue;
            }
            let wx = t * 16 + 4;
            cv.rect(wx, wall_top + 5, 8, 8, WOOD[0]).rect(wx + 1, wall_top + 6, 6, 6, c(0xF8D878));
            cv.rect(wx + 1, wall_top + 6, 6, 2, c(0xFFF0B0)).rect(wx + 4, wall_top + 6, 1, 6, WOOD[1]).rect(
                wx + 1,
                wall_top + 9,
                6,
                1,
                WOOD[1],
            );
        }
        let dx = door_tile * 16 + 3;
        cv.rect(dx - 1, h - 18, 12, 16, WOOD[0]).rect(dx, h - 17, 10, 15, WOOD[2]);
        cv.rect(dx + 4, h - 17, 2, 15, WOOD[1]).rect(dx + 7, h - 10, 1, 2, c(0xF8D850));

        for y in 0..wall_top + 2 {
            for x in 0..w {
                let row = y / 4;
                let seam = y % 4 == 3 || (x + row * 3) % 8 == 0;
                let col = if seam {
                    roof_ramp[1]
                } else if y % 4 == 0 {
                    roof_ramp[3]
                } else {
                    roof_ramp[2]
                };
                cv.set(x, y, col);
            }
        }
        cv.rect(0, 0, w, 3, roof_ramp[1]).rect(0, wall_top + 1, w, 2, roof_ramp[0]);
        cv.rect(w - 14, 0, 6, 3, STONE[1]).rect(w - 13, -1, 4, 1, STONE[2]);
        cv.outline();
        cv
    })
}

pub fn cave_mouth() -> TexId {
    gfx::cached("cave_mouth", || {
        let mut cv = Canvas::new(32, 24);
        cv.ellipse(16.0, 16.0, 15.0, 14.0, STONE[2]).rect(1, 16, 30, 8, STONE[2]);
        cv.shade(|x, y, _| {
            if (x * 7 + y * 3) % 11 == 0 {
                STONE[1]
            } else if y < 8 {
                STONE[3]
            } else {
                STONE[2]
            }
        });
        cv.ellipse(16.0, 18.0, 10.0, 11.0, palette::OUTLINE).rect(6, 18, 20, 6, palette::OUTLINE);
        cv.ellipse(16.0, 19.0, 8.0, 9.0, c(0x08060C)).rect(8, 19, 16, 5, c(0x08060C));
        cv.outline();
        cv
    })
}

pub fn stairs_down() -> TexId {
    gfx::cached("stairs_down", || {
        let mut cv = Canvas::new(16, 16);
        for i in 0..4 {
            let shade = STONE[3].lerp(palette::OUTLINE, i as f32 / 4.0);
            cv.rect(1 + i, 1 + i * 4, 14 - i * 2, 3, shade).rect(
                1 + i,
                4 + i * 4,
                14 - i * 2,
                1,
                shade.lerp(palette::OUTLINE, 0.6),
            );
        }
        cv.rect(0, 0, 1, 16, STONE[1]).rect(15, 0, 1, 16, STONE[1]);
        cv
    })
}

pub fn stairs_up() -> TexId {
    gfx::cached("stairs_up", || {
        let mut cv = Canvas::new(16, 16);
        for i in 0..4 {
            let shade = STONE[1].lerp(STONE[3], i as f32 / 3.0);
            cv.rect(1, 1 + i * 4, 14, 3, shade).rect(1, 4 + i * 4, 14, 1, STONE[0]);
        }
        cv.rect(0, 0, 1, 16, STONE[0]).rect(15, 0, 1, 16, STONE[0]);
        cv
    })
}

pub fn cracked_rock() -> TexId {
    gfx::cached("cracked_rock", || {
        let rock = [c(0x3A2A24), c(0x6A4E3A), c(0x9A7656), c(0xC8A27A)];
        let mut cv = Canvas::new(16, 16);
        cv.rect(0, 1, 16, 15, rock[2]);
        cv.shade(|_, y, _| {
            if y < 4 {
                rock[3]
            } else if y > 12 {
                rock[1]
            } else {
                rock[2]
            }
        });
        cv.line(3, 2, 6, 7, rock[0])
            .line(6, 7, 4, 12, rock[0])
            .line(6, 7, 11, 6, rock[0])
            .line(11, 6, 13, 11, rock[0])
            .line(11, 6, 12, 2, rock[0]);
        cv.pattern(6, 9, "#.#|.#.|#.#", &[('#', c(0x9FE8D8))]);
        cv.outline();
        cv
    })
}

pub fn thorns() -> TexId {
    gfx::cached("thorns", || {
        let vine = [c(0x1A1028), c(0x4A2A5A), c(0x7A4A8A), c(0xB07AC0)];
        let mut cv = Canvas::new(16, 16);
        for i in 0..4 {
            let ox = (2 + i * 4) as f32;
            for y in 1..16 {
                let x = (ox + (y as f32 * 0.7 + (i * 2) as f32).sin() * 1.6) as i32;
                cv.set(x, y, vine[2]);
                cv.set(x + 1, y, vine[1]);
                if y % 4 == i % 4 {
                    cv.set(x - 1, y - 1, vine[3]);
                    cv.set(x + 2, y - 1, vine[3]);
                }
            }
        }
        cv.outline_with(vine[0]);
        cv
    })
}

/// A frozen-over water tile laid by the Sword of Water.
pub fn ice_tile() -> TexId {
    gfx::cached("ice_tile", || {
        let ice = [c(0x4A7AB0), c(0x8AC0E8), c(0xBCE4F8), c(0xF0FCFF)];
        let mut cv = Canvas::new(16, 16);
        cv.rect(0, 0, 16, 16, ice[2]);
        cv.shade(|x, y, _| {
            if (x + y * 2) % 13 == 0 {
                ice[3]
            } else if noise::hash01(x, y, 131) < 0.06 {
                ice[1]
            } else {
                ice[2]
            }
        });
        cv.line(2, 11, 6, 8, ice[1]).line(6, 8, 9, 9, ice[1]).line(9, 3, 12, 6, ice[1]);
        cv.rect(0, 15, 16, 1, ice[1]).rect(0, 0, 16, 1, ice[3]);
        cv
    })
}

pub fn ice_wall() -> TexId {
    gfx::cached("ice_wall", || {
        let ice = [c(0x2A5A90), c(0x6AA8E0), c(0xA8DCF8), c(0xF0FCFF)];
        let mut cv = Canvas::new(16, 16);
        cv.rect(0, 1, 16, 15, ice[1]);
        for (x, w, h) in [(0, 5, 15), (4, 5, 12), (8, 4, 15), (11, 5, 13)] {
            cv.rect(x, 16 - h, w, h, ice[2]).rect(x, 16 - h, 1, h, ice[3]).rect(x + w - 1, 16 - h, 1, h, ice[1]);
        }
        cv.line(3, 5, 6, 10, ice[3]).line(10, 4, 12, 8, ice[3]);
        cv.outline_with(ice[0]);
        cv
    })
}

pub fn flame_wall() -> TexId {
    gfx::cached("flame_wall", || {
        let fire = [c(0x6A1008), c(0xD84020), c(0xFF9A30), c(0xFFE890)];
        let mut cv = Canvas::new(32, 16);
        for f in 0..2 {
            for i in 0..4 {
                let cx = f * 16 + 2 + i * 4;
                let h = 9 + (noise::hash(i, f, 141) % 6) as i32;
                for y in 0..h {
                    let half = ((h - y) / 3).max(0);
                    let col = if y < h / 3 {
                        fire[1]
                    } else if y < 2 * h / 3 {
                        fire[2]
                    } else {
                        fire[3]
                    };
                    cv.rect(cx - half, 15 - y, half * 2 + 1, 1, col);
                }
            }
        }
        cv.outline_with(fire[0]);
        cv
    })
}

pub fn iron_wall() -> TexId {
    gfx::cached("iron_wall", || {
        let iron = [c(0x1A1A24), c(0x3E3E52), c(0x6A6A80), c(0x9E9EB4)];
        let mut cv = Canvas::new(16, 16);
        cv.rect(0, 0, 16, 16, iron[2])
            .rect(0, 0, 16, 2, iron[3])
            .rect(0, 7, 16, 2, iron[1])
            .rect(0, 14, 16, 2, iron[1]);
        for (x, y) in [(2, 3), (13, 3), (2, 10), (13, 10)] {
            cv.rect(x, y, 2, 2, iron[3]).rect(x + 1, y + 1, 1, 1, iron[1]);
        }
        cv.pattern(6, 3, "..#..|.###.|#####|..#..|.#...", &[('#', c(0xF8E060))]);
        cv.outline_with(iron[0]);
        cv
    })
}

pub fn ice_shard() -> TexId {
    gfx::cached("ice_shard", || {
        Canvas::new(12, 7)
            .pattern(
                0,
                0,
                "....####....|..##wwwl##..|##wwwwlllll#|..##lllb##..|....####....",
                &[('#', c(0x1C3678)), ('w', Rgba::WHITE), ('l', c(0x8FD8FF)), ('b', c(0x3F86D8))],
            )
            .outline_with(c(0x10183A))
            .take()
    })
}

pub fn thunder_bolt() -> TexId {
    gfx::cached("bolt", || {
        let mut cv = Canvas::new(36, 9);
        for f in 0..2 {
            let mut y = 4;
            for x in 0..18 {
                if x % 3 == 0 {
                    y = (y + (noise::hash(x, f, 151) % 5) as i32 - 2).clamp(1, 7);
                }
                cv.set(f * 18 + x, y, Rgba::WHITE);
                cv.set(f * 18 + x, y - 1, c(0xFFF0A0));
                cv.set(f * 18 + x, y + 1, c(0xF8D040));
            }
        }
        cv.outline_with(c(0x6A5010));
        cv
    })
}

pub fn barrier_bubble() -> TexId {
    gfx::cached("barrier", || {
        let mut cv = Canvas::new(28, 28);
        for y in 0..28 {
            for x in 0..28 {
                let (fx, fy) = (x as f32 - 13.5, y as f32 - 13.5);
                let d = (fx * fx + fy * fy).sqrt();
                if d > 13.5 {
                    continue;
                }
                if d > 12.3 {
                    cv.set(x, y, c(0xC8F0FF).mul(0.9));
                } else if x + y < 14 && d > 9.0 {
                    cv.set(x, y, Rgba::WHITE.mul(0.5));
                } else {
                    cv.set(x, y, c(0x6AB8FF).mul(0.18));
                }
            }
        }
        cv
    })
}

pub fn gate() -> TexId {
    gfx::cached("gate", || {
        let iron = [c(0x1A1A24), c(0x3A3A4A), c(0x6A6A7E), c(0xA0A0B4)];
        let mut cv = Canvas::new(16, 16);
        cv.rect(0, 0, 16, 3, iron[1]).rect(0, 0, 16, 1, iron[2]).rect(0, 13, 16, 3, iron[1]);
        for x in (1..16).step_by(4) {
            cv.rect(x, 3, 2, 10, iron[2]).rect(x, 3, 1, 10, iron[3]);
        }
        cv.outline_with(iron[0]);
        cv
    })
}

pub fn locked_door() -> TexId {
    gfx::cached("locked_door", || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(1, 0, 14, 16, WOOD[2]).rect(1, 0, 14, 2, WOOD[3]);
        for x in (4..15).step_by(4) {
            cv.rect(x, 2, 1, 14, WOOD[1]);
        }
        cv.rect(1, 5, 14, 2, c(0x5A5A6A)).rect(1, 11, 14, 2, c(0x5A5A6A));
        cv.rect(6, 7, 4, 4, c(0xE8C050)).rect(7, 8, 2, 2, palette::OUTLINE);
        cv.outline();
        cv
    })
}

pub fn floor_switch(pressed: bool) -> TexId {
    gfx::cached(&format!("switch_{}", tf(pressed)), || {
        let mut cv = Canvas::new(16, 16);
        cv.rect(2, 3, 12, 11, STONE[1]).rect(3, 4, 10, 9, if pressed { c(0x3A7A6A) } else { c(0x5AC0A0) });
        if !pressed {
            cv.rect(3, 4, 10, 2, c(0x9AF0D0)).rect(3, 12, 10, 1, c(0x2A6A5A));
        }
        cv.outline();
        cv
    })
}

pub fn torch() -> TexId {
    gfx::cached("torch", || {
        let mut cv = Canvas::new(8, 16);
        cv.rect(3, 8, 2, 8, WOOD[1]).rect(2, 7, 4, 2, STONE[2]);
        cv.outline();
        cv
    })
}

pub fn spark() -> TexId {
    gfx::cached("spark", || Canvas::new(3, 3).pattern(0, 0, ".#.|###|.#.", &[('#', Rgba::WHITE)]).take())
}

pub fn orb() -> TexId {
    gfx::cached("orb", || {
        let mut cv = Canvas::new(8, 8);
        cv.circle(4.0, 4.0, 3.5, Rgba::WHITE);
        cv.shade(|x, y, _| if x + y < 6 { Rgba::WHITE } else { Rgba::rgb(200, 200, 210) });
        cv
    })
}

pub fn wind_slash() -> TexId {
    gfx::cached("wind_slash", || {
        let mut cv = Canvas::new(14, 14);
        let light = c(0xE8FFF8);
        let mid = c(0x8AEAD0);
        let mut a = -1.5f32;
        while a <= 1.5 {
            let r = 6.0;
            cv.set((7.0 + a.cos() * r) as i32, (7.0 + a.sin() * r) as i32, light);
            cv.set(
                (7.0 + a.cos() * (r - 1.0)) as i32,
                (7.0 + a.sin() * (r - 1.0)) as i32,
                if a.abs() < 1.2 { mid } else { light },
            );
            if a.abs() < 0.9 {
                cv.set((7.0 + a.cos() * (r - 2.0)) as i32, (7.0 + a.sin() * (r - 2.0)) as i32, mid);
            }
            a += 0.05;
        }
        cv.outline_with(c(0x1A5A50));
        cv
    })
}

pub fn tornado() -> TexId {
    gfx::cached("tornado", || {
        let mut cv = Canvas::new(64, 22);
        for f in 0..4 {
            for ring in 0..6 {
                let (cy, rx) = (19.0 - ring as f32 * 3.2, 2.5 + ring as f32 * 1.1);
                let cx = (f * 16 + 8) as f32 + (ring as f32 * 0.9 + f as f32 * PI / 2.0).sin() * 1.5;
                cv.ellipse(cx, cy, rx, 1.6, if ring % 2 == 0 { c(0xE8FFF8) } else { c(0x8AEAD0) });
                cv.set((cx + (f as f32 * 1.6 + ring as f32).cos() * rx) as i32, cy as i32, c(0x3A9A88));
            }
        }
        cv.outline_with(c(0x1A5A50));
        cv
    })
}

pub fn fireball() -> TexId {
    gfx::cached("fireball", || {
        let mut cv = Canvas::new(24, 12);
        for f in 0..2 {
            let ox = f * 12;
            cv.circle((ox + 6) as f32, 6.0, 5.0, c(0xE04018)).circle((ox + 6) as f32, 6.0, 3.6, c(0xFF9A30)).circle(
                (ox + 5) as f32,
                5.0,
                2.0,
                c(0xFFF0A0),
            );
            for i in 0..5 {
                cv.set(
                    ox + 1 + (noise::hash(i, f, 5) % 10) as i32,
                    1 + (noise::hash(i, f, 6) % 10) as i32,
                    c(0xFFD060),
                );
            }
        }
        cv.outline_with(c(0x5A1008));
        cv
    })
}

pub fn coin() -> TexId {
    gfx::cached("coin", || {
        let gold = [c(0x8A5A10), c(0xD8A020), c(0xF8D850), c(0xFFF8C0)];
        let mut cv = Canvas::new(32, 8);
        let widths = [3, 2, 1, 2];
        for f in 0..4 {
            let rx = widths[f as usize] as f32 + 0.5;
            cv.ellipse((f * 8 + 4) as f32, 4.0, rx, 3.5, gold[2]);
            cv.shade(|x, _, col| {
                if x / 8 != f || col != gold[2] {
                    col
                } else if x % 8 < 4 {
                    gold[3]
                } else {
                    gold[1]
                }
            });
        }
        cv.outline_with(gold[0]);
        cv
    })
}

pub fn heart_pickup() -> TexId {
    gfx::cached("heart", || {
        Canvas::new(9, 8)
            .pattern(
                0,
                0,
                ".##.##...|#wr#rrr#.|#rrrrrr#.|#rrrrrd#.|.#rrrd#..|..#rd#...|...##....",
                &[('#', palette::OUTLINE), ('r', c(0xE84048)), ('w', Rgba::WHITE), ('d', c(0x9A1828))],
            )
            .take()
    })
}

pub fn shadow(width: i32) -> TexId {
    gfx::cached(&format!("shadow_{width}"), || {
        let h = (width / 3).max(3);
        let mut cv = Canvas::new(width, h);
        cv.ellipse(width as f32 / 2.0, h as f32 / 2.0, width as f32 / 2.0, h as f32 / 2.0, Rgba::BLACK.mul(0.35));
        cv
    })
}

pub fn puff() -> TexId {
    gfx::cached("puff", || {
        let mut cv = Canvas::new(80, 16);
        for f in 0..5 {
            let r = 2.0 + f as f32 * 1.6;
            for i in 0..8 {
                let a = i as f32 * TAU / 8.0 + f as f32 * 0.2;
                cv.circle(
                    (f * 16 + 8) as f32 + a.cos() * r,
                    8.0 + a.sin() * r,
                    (2.4 - f as f32 * 0.45).max(0.8),
                    if f < 3 { Rgba::WHITE } else { c(0xC8C8D8) },
                );
            }
        }
        cv
    })
}

pub fn icon(id: &str) -> TexId {
    gfx::cached(&format!("icon_{id}"), || {
        let mut cv = Canvas::new(12, 12);
        match id {
            "herb" => {
                cv.ellipse(4.0, 5.0, 3.0, 4.0, LEAF[3]).ellipse(8.0, 5.0, 3.0, 4.0, LEAF[2]).line(6, 6, 6, 11, LEAF[1]);
            }
            "ether" => {
                cv.rect(4, 1, 4, 2, WOOD[2]).ellipse(6.0, 7.5, 4.5, 4.0, c(0x4A9AF0)).rect(4, 5, 2, 2, c(0xC8E8FF));
            }
            "tanned_hide" | "chain_mail" => {
                let body = if id == "chain_mail" { STONE[2] } else { WOOD[3] };
                cv.rect(2, 2, 8, 9, body).rect(0, 2, 2, 4, body).rect(10, 2, 2, 4, body).rect(
                    4,
                    2,
                    4,
                    2,
                    palette::OUTLINE,
                );
            }
            "wood_shield" | "iron_shield" => {
                let face = if id == "iron_shield" { STONE[3] } else { WOOD[2] };
                cv.rect(2, 1, 8, 6, face).ellipse(6.0, 7.0, 4.0, 4.0, face).rect(5, 2, 2, 8, c(0xE8C050));
            }
            "grotto_key" => {
                key_icon(&mut cv, c(0xE8C050));
            }
            "ember_crystal" => {
                cv.pattern(
                    2,
                    0,
                    "..##....|.#oo#...|#yooo#..|#yoorr#.|.#orr#..|..#r#...|...#....",
                    &[('#', c(0x6A1A10)), ('o', c(0xFF8A30)), ('y', c(0xFFF0A0)), ('r', c(0xC83A20))],
                );
                cv.rect(4, 9, 6, 2, c(0xFF8A30).mul(0.5));
            }
            "gale_orb" | "flame_orb" | "tide_orb" | "storm_orb" => {
                let gem = relic_color(id);
                cv.circle(6.0, 6.5, 4.8, gem.lerp(palette::OUTLINE, 0.55)).circle(6.0, 6.5, 3.8, gem);
                cv.circle(4.8, 5.2, 1.4, gem.lerp(Rgba::WHITE, 0.6)).set(4, 4, Rgba::WHITE);
            }
            "gale_bracer" | "flame_bracer" | "tide_bracer" | "storm_bracer" | "prism_band" => {
                let gem = relic_color(id);
                cv.ellipse(6.0, 7.0, 4.5, 4.0, c(0xE8C050)).ellipse(6.0, 7.0, 2.5, 2.0, Rgba::TRANSPARENT);
                cv.shade(|x, y, col| {
                    if ((x - 6) * (x - 6)) as f32 / 6.25 + ((y - 7) * (y - 7)) as f32 / 4.0 < 1.0 {
                        Rgba::TRANSPARENT
                    } else {
                        col
                    }
                });
                cv.circle(6.0, 3.0, 2.4, gem).set(5, 2, Rgba::WHITE);
            }
            "elixir" => {
                cv.rect(4, 1, 4, 2, WOOD[2]).ellipse(6.0, 7.5, 4.5, 4.0, c(0xF04A8A)).rect(4, 5, 2, 2, c(0xFFD0E8));
            }
            "weave_knot" => {
                let thread = c(0xC8B8F0);
                cv.line(1, 3, 10, 9, thread).line(1, 9, 10, 3, thread).line(6, 0, 6, 11, thread.mul(0.8));
                cv.circle(6.0, 6.0, 2.2, c(0xF0E8FF));
            }
            "insight_tome" => {
                cv.rect(2, 2, 8, 9, c(0x5A3A8A)).rect(3, 3, 6, 7, c(0x7A5AB0)).rect(9, 3, 1, 8, c(0xE8E0D0));
                cv.rect(5, 5, 2, 3, c(0xF8D850));
            }
            "dormant_blade" => {
                cv.line(2, 10, 9, 3, STONE[2]).line(3, 10, 10, 3, STONE[3]).line(1, 8, 4, 11, c(0x8A6A40));
            }
            "aether_mote" => {
                cv.circle(6.0, 6.0, 4.5, c(0x6E2890)).circle(6.0, 6.0, 3.2, c(0xF496FF)).circle(
                    5.0,
                    5.0,
                    1.4,
                    Rgba::WHITE,
                );
            }
            "rabbit_boots" => {
                cv.rect(3, 2, 4, 7, c(0xE8E0D0)).rect(3, 8, 8, 3, c(0xE8E0D0)).rect(3, 2, 4, 1, Rgba::WHITE).rect(
                    1,
                    0,
                    2,
                    4,
                    c(0xF0B0C0),
                );
            }
            "temple_key" | "spire_key" | "sky_key" => {
                let metal = match id {
                    "temple_key" => c(0xF08A9A),
                    "spire_key" => c(0x9AA0F0),
                    _ => c(0xF0F0FF),
                };
                key_icon(&mut cv, metal);
            }
            "tide_crystal" | "storm_crystal" => {
                let (hi, mid, lo) = if id == "tide_crystal" {
                    (c(0xC8F0FF), c(0x4A9AF0), c(0x1C3678))
                } else {
                    (c(0xFFFBD0), c(0xF8E060), c(0x8A7010))
                };
                cv.pattern(
                    2,
                    0,
                    "..##....|.#mm#...|#hmmm#..|#hmmll#.|.#mll#..|..#l#...|...#....",
                    &[('#', lo), ('m', mid), ('h', hi), ('l', lo)],
                );
            }
            "scale_armor" | "frost_plate" | "aether_mail" => {
                let plate = match id {
                    "scale_armor" => c(0x2E8A8A),
                    "frost_plate" => c(0xA8D8F4),
                    _ => c(0xE8E0FF),
                };
                cv.rect(2, 2, 8, 9, plate)
                    .rect(0, 2, 2, 4, plate)
                    .rect(10, 2, 2, 4, plate)
                    .rect(4, 2, 4, 2, palette::OUTLINE)
                    .rect(5, 5, 2, 5, plate.lerp(Rgba::WHITE, 0.4));
            }
            "coral_shield" | "storm_shield" | "sacred_shield" => {
                let face = match id {
                    "coral_shield" => c(0xF07A6A),
                    "storm_shield" => c(0x6A70C0),
                    _ => c(0xF0F0FF),
                };
                cv.rect(2, 1, 8, 6, face).ellipse(6.0, 7.0, 4.0, 4.0, face).rect(5, 2, 2, 8, c(0xE8C050));
            }
            "power_charm" | "guard_charm" | "swift_anklet" | "focus_band" | "fortune_coin" | "scholar_lens"
            | "sage_pendant" | "vampire_fang" | "feather_charm" | "salamander_scale" | "ember_crown"
            | "tide_locket" | "night_eye" | "storm_greaves" | "corsair_coin" | "concord_core" => {
                let accent = match id {
                    "power_charm" => c(0xE84040),
                    "guard_charm" => c(0x4A80E8),
                    "swift_anklet" => c(0x6AE0A0),
                    "focus_band" => c(0xF0A0F0),
                    "fortune_coin" => c(0xF8D850),
                    "scholar_lens" => c(0xC8E8FF),
                    "sage_pendant" => c(0x8A6AE8),
                    "vampire_fang" => c(0xA01830),
                    "feather_charm" => Rgba::WHITE,
                    "ember_crown" => c(0xFF5020),
                    "tide_locket" => c(0x30C0D0),
                    "night_eye" => c(0x7040C0),
                    "storm_greaves" => c(0xF8E060),
                    "corsair_coin" => c(0xB08A30),
                    "concord_core" => c(0xFFC8FF),
                    _ => c(0xF07030),
                };
                cv.line(2, 1, 6, 5, c(0xC8A050)).line(10, 1, 6, 5, c(0xC8A050));
                cv.pattern(
                    3,
                    5,
                    "..##..|.#aa#.|#aaaa#|#aaaa#|.#aa#.|..##..",
                    &[('#', accent.lerp(palette::OUTLINE, 0.5)), ('a', accent)],
                );
                cv.set(5, 7, Rgba::WHITE);
            }
            _ => {
                if let Some(gem) = supports::get(id) {
                    let face = match (gem.prism, gem.fits) {
                        (true, _) => c(0xF496FF),
                        (_, Some(Group::Attack)) => c(0xE85A46),
                        (_, Some(Group::Ranged)) => c(0x6ED278),
                        (_, Some(Group::Special)) => c(0x6E96FF),
                        (_, None) => c(0xE6DCAA),
                    };
                    cv.pattern(
                        2,
                        1,
                        "..####..|.#aaaa#.|#abbbba#|#aabbaa#|.#aaaa#.|..#aa#..|...##...",
                        &[('#', face.lerp(palette::OUTLINE, 0.6)), ('a', face), ('b', face.lerp(Rgba::WHITE, 0.55))],
                    );
                } else if let Some(sigil) = items::sigil(id) {
                    let (glow, dark) = (sigil.element.glow(), sigil.element.dark());
                    cv.circle(6.0, 6.0, 5.0, dark).circle(6.0, 6.0, 3.5, glow);
                    cv.line(6, 3, 6, 9, glow.lerp(Rgba::WHITE, 0.6)).line(3, 6, 9, 6, glow.lerp(Rgba::WHITE, 0.6));
                    cv.set(6, 6, Rgba::WHITE);
                } else {
                    cv.circle(6.0, 6.0, 4.0, if id == "blight_core" { c(0x8030A0) } else { Rgba::GRAY });
                }
            }
        }
        cv.outline();
        cv
    })
}

/// The gem color of an element's orb or bracer, or the Prism Band.
fn relic_color(id: &str) -> Rgba {
    match id.split('_').next() {
        Some("flame") => c(0xFF8A30),
        Some("tide") => c(0x4A9AF0),
        Some("storm") => c(0xF8E060),
        Some("prism") => c(0xF0A0F0),
        _ => c(0x8AEAD0),
    }
}

fn key_icon(cv: &mut Canvas, metal: Rgba) {
    cv.circle(3.5, 4.0, 2.5, metal).rect(5, 3, 6, 2, metal).rect(8, 5, 1, 2, metal).rect(10, 5, 1, 3, metal);
    cv.set(3, 4, palette::OUTLINE);
}
