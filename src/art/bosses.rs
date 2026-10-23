//! Boss sprites and the props of their arenas.

#![allow(dead_code)]

use std::f32::consts::{PI, TAU};

use crate::art::props::STONE;
use crate::core::canvas::Canvas;
use crate::core::color::{Rgba, c};
use crate::core::gfx::{self, TexId};
use crate::core::noise;
use crate::core::palette;

/// The Tidal Serpent rising from a pool: two 32x44 frames (mouth closed, mouth open).
pub fn serpent() -> TexId {
    gfx::cached("serpent", || {
        serpent_canvas(
            [c(0x0A2A3A), c(0x14506A), c(0x1E7A92), c(0x4AB4C0)],
            c(0xE8E0B0),
            [c(0x8A1A2A), c(0xD84A4A), c(0xF89A70)],
            c(0x04121A),
        )
    })
}

/// The Magma Wyrm: the serpent's build in basalt and glowing seams.
pub fn wyrm() -> TexId {
    gfx::cached("wyrm", || {
        serpent_canvas(
            [c(0x1A1010), c(0x3A2420), c(0x5A3A30), c(0xFF8A30)],
            c(0xFFC860),
            [c(0x6A1A0A), c(0xE85A1A), c(0xFFD070)],
            c(0x0A0404),
        )
    })
}

pub fn wyrm_plate() -> TexId {
    gfx::cached("wyrmplate", || {
        let mut cv = Canvas::new(14, 10);
        cv.ellipse(7.0, 5.0, 6.0, 4.0, c(0x3A3030));
        cv.line(3, 3, 10, 6, c(0xFF8A30)).line(5, 7, 9, 2, c(0xFFB050));
        cv.rect(4, 2, 3, 1, c(0x6A5A50));
        cv.outline();
        cv
    })
}

fn serpent_canvas(scale: [Rgba; 4], belly: Rgba, fin: [Rgba; 3], outline: Rgba) -> Canvas {
    let mut cv = Canvas::new(64, 44);
    for f in 0..2 {
        let mut s = Canvas::new(32, 44);
        for y in 14..42 {
            let t = (y - 14) as f32 / 28.0;
            let cx = 16.0 + (t * 3.2 + 0.4).sin() * 4.0;
            let half = 4.5 + t * 2.5;
            for x in (cx - half) as i32..=(cx + half) as i32 {
                let u = (x as f32 - (cx - half)) / (2.0 * half);
                let col = if u > 0.62 && u < 0.9 {
                    belly
                } else if u < 0.25 {
                    scale[3]
                } else if (y + x / 3) % 4 == 0 {
                    scale[1]
                } else {
                    scale[2]
                };
                s.set(x, y, col);
            }
        }
        s.ellipse(16.0, 11.0, 9.0, 7.0, scale[2]);
        s.shade(|x, y, col| if col == scale[2] && y < 9 && x < 15 { scale[3] } else { col });
        s.ellipse(16.0, (15 + f) as f32, 7.0, (3 + f * 2) as f32, if f == 1 { c(0x5A0A14) } else { scale[1] });
        if f == 1 {
            s.set(12, 15, Rgba::WHITE);
            s.set(20, 15, Rgba::WHITE);
        }
        s.rect(11, 8, 3, 2, c(0xF8E060)).rect(19, 8, 3, 2, c(0xF8E060));
        s.set(12, 8, c(0x1A0A04));
        s.set(20, 8, c(0x1A0A04));
        for i in 0..4 {
            let col = fin[if i % 2 == 0 { 1 } else { 2 }];
            s.line(8 - i, 6 + i * 2, 3 - i, 2 + i * 3, col).line(24 + i, 6 + i * 2, 29 + i, 2 + i * 3, col);
        }
        s.line(16, 2, 16, 5, fin[2]).line(14, 3, 13, 0, fin[1]).line(18, 3, 19, 0, fin[1]);
        s.outline_with(outline);
        cv.blit(&s, f * 32, 0);
    }
    cv
}

/// The Storm Golem: two 40x44 frames (idle and arms raised).
pub fn golem() -> TexId {
    gfx::cached("golem", || {
        let rock = [c(0x24203A), c(0x46405E), c(0x6A6488), c(0x9A94B4)];
        let core = [c(0x2A6AE0), c(0x8AD0FF), Rgba::WHITE];
        let mut cv = Canvas::new(80, 44);
        for f in 0..2 {
            let mut g = Canvas::new(40, 44);
            g.rect(12, 34, 6, 9, rock[1])
                .rect(22, 34, 6, 9, rock[1])
                .rect(11, 41, 8, 3, rock[0])
                .rect(21, 41, 8, 3, rock[0]);
            g.ellipse(20.0, 24.0, 12.0, 12.0, rock[2]);
            g.ellipse(20.0, 9.0, 7.0, 6.0, rock[2]);
            let arm_y = if f == 0 { 20 } else { 6 };
            g.ellipse(5.0, (arm_y + 6) as f32, 5.0, 9.0, rock[2]).ellipse(35.0, (arm_y + 6) as f32, 5.0, 9.0, rock[2]);
            g.ellipse(5.0, (arm_y + 15) as f32, 5.0, 4.0, rock[1]).ellipse(
                35.0,
                (arm_y + 15) as f32,
                5.0,
                4.0,
                rock[1],
            );
            g.shade(|x, y, col| {
                if col != rock[2] {
                    return col;
                }
                let w = noise::worley(x as f32, y as f32, 6.0, 501);
                if w.f2 - w.f1 < 0.8 {
                    return rock[0];
                }
                if (x as f32 - w.center.x) + (y as f32 - w.center.y) < -1.5 { rock[3] } else { rock[2] }
            });
            g.circle(20.0, 24.0, 4.5, core[0]).circle(20.0, 24.0, 2.8, core[1]).circle(19.5, 23.5, 1.2, core[2]);
            g.line(20, 19, 20, 14, core[1]).line(15, 26, 11, 30, core[1]).line(25, 26, 29, 30, core[1]);
            g.rect(16, 8, 3, 2, core[1]).rect(22, 8, 3, 2, core[1]);
            g.outline_with(c(0x0A0814));
            cv.blit(&g, f * 40, 0);
        }
        cv
    })
}

/// Sovereign Vael's first form: two 32x40 frames of a crowned figure in regal robes.
pub fn sovereign() -> TexId {
    gfx::cached("sovereign", || {
        let robe = [c(0x1E1440), c(0x3A2A70), c(0x5A44A0), c(0x8A74D0)];
        let gold = [c(0x8A6010), c(0xE0B040), c(0xFFF0A0)];
        let mut cv = Canvas::new(64, 40);
        for f in 0..2 {
            let mut s = Canvas::new(32, 40);
            for y in 14..39 {
                let half = (5.0 + (y - 14) as f32 / 25.0 * 10.0) as i32;
                for x in 16 - half..=15 + half {
                    let col = if x < 12 {
                        robe[3]
                    } else if x > 20 || (x + f) % 6 == 0 {
                        robe[1]
                    } else {
                        robe[2]
                    };
                    s.set(x, y, col);
                }
            }
            s.rect(15, 14, 2, 25, gold[1]).rect(4, 37, 24, 2, gold[1]);
            s.ellipse(16.0, 15.0, 9.0, 3.0, robe[3]);
            s.ellipse(16.0, 10.0, 5.0, 6.0, c(0xE8D8C8)).rect(11, 13, 10, 3, c(0xE8D8C8));
            s.rect(10, 6, 2, 9, c(0xD8D8F0)).rect(21, 6, 2, 9, c(0xD8D8F0)).rect(11, 5, 11, 2, c(0xD8D8F0));
            s.rect(13, 9, 2, 2, c(0x3A1A5A)).rect(18, 9, 2, 2, c(0x3A1A5A)).rect(15, 13, 3, 1, c(0xA08080));
            s.set(13, 9, c(0xC080FF));
            s.set(18, 9, c(0xC080FF));
            s.rect(11, 3, 11, 3, gold[1])
                .rect(11, 1, 2, 2, gold[1])
                .rect(15, 0, 2, 3, gold[1])
                .rect(20, 1, 2, 2, gold[1]);
            s.set(16, 4, c(0x8AD0FF));
            s.rect(11, 3, 11, 1, gold[2]);
            let hand = if f == 0 { 22.0 } else { 19.0 };
            s.ellipse(5.0, hand, 2.5, 2.5, c(0xE8D8C8)).ellipse(27.0, hand, 2.5, 2.5, c(0xE8D8C8));
            s.outline_with(c(0x0A0618));
            cv.blit(&s, f * 32, 0);
        }
        cv
    })
}

/// Vael's final winged form: two 48x48 frames with beating wings.
pub fn sovereign_ascended() -> TexId {
    gfx::cached("sovereign2", || {
        let armor = [c(0x14102A), c(0x2A2250), c(0x463A80), c(0x7A6AC0)];
        let wing = [c(0x8A90C8), c(0xC8D0F0), Rgba::WHITE];
        let mut cv = Canvas::new(96, 48);
        for f in 0..2 {
            let mut s = Canvas::new(48, 48);
            for side in [-1, 1] {
                for i in 0..5 {
                    let angle = (if f == 0 { -0.5 } else { -0.2 }) + i as f32 * 0.32;
                    let (x0, y0) = (24 + side * 6, 18);
                    let x1 = 24 + side * (10.0 + angle.cos() * 18.0) as i32;
                    let y1 = (y0 as f32 + angle.sin() * 18.0) as i32;
                    s.line(x0, y0, x1, y1, wing[1]).line(x0, y0 + 1, x1, y1 + 1, wing[0]).line(
                        x0,
                        y0 - 1,
                        x1,
                        y1 - 1,
                        wing[2],
                    );
                }
            }
            s.ellipse(24.0, 30.0, 9.0, 14.0, armor[2]);
            s.shade(|x, _, col| {
                if col != armor[2] {
                    col
                } else if x < 20 {
                    armor[3]
                } else if x > 28 {
                    armor[1]
                } else {
                    armor[2]
                }
            });
            s.ellipse(24.0, 14.0, 6.0, 7.0, armor[1]).rect(20, 13, 9, 2, c(0xFF4A8A));
            s.circle(24.0, 27.0, 3.5, c(0xF0E8FF)).circle(24.0, 27.0, 2.0, c(0x8AD0FF));
            s.rect(19, 5, 11, 3, c(0xE0B040)).rect(19, 2, 2, 3, c(0xE0B040)).rect(23, 0, 3, 5, c(0xE0B040)).rect(
                28,
                2,
                2,
                3,
                c(0xE0B040),
            );
            s.outline_with(c(0x08061A));
            cv.blit(&s, f * 48, 0);
        }
        cv
    })
}

pub fn element_orb(color: Rgba) -> TexId {
    gfx::cached(&format!("orb_{}", color.packed()), || {
        let mut cv = Canvas::new(14, 14);
        cv.circle(7.0, 7.0, 6.0, color.lerp(palette::OUTLINE, 0.35)).circle(7.0, 7.0, 4.5, color).circle(
            5.5,
            5.5,
            2.0,
            color.lerp(Rgba::WHITE, 0.7),
        );
        cv.outline();
        cv
    })
}

/// The Sky Altar: a pedestal with three crystal sockets.
pub fn altar() -> TexId {
    gfx::cached("altar", || {
        let mut cv = Canvas::new(32, 28);
        cv.rect(2, 18, 28, 10, STONE[1])
            .rect(2, 18, 28, 2, STONE[3])
            .rect(4, 8, 24, 11, STONE[2])
            .rect(4, 8, 24, 2, STONE[3]);
        cv.rect(13, 0, 6, 9, STONE[2]).rect(13, 0, 6, 1, STONE[3]).rect(14, 2, 4, 4, c(0xE8F0FF));
        for (x, color) in [(7, c(0xFF8A30)), (14, c(0x4A9AF0)), (21, c(0xF8E060))] {
            cv.rect(x, 11, 4, 4, palette::OUTLINE).rect(x + 1, 12, 2, 2, color.lerp(palette::OUTLINE, 0.4));
        }
        cv.rect(4, 22, 24, 1, STONE[0]);
        cv.outline();
        cv
    })
}

/// The First Concord: a floating ring of pale stone and gold with four empty sockets; the core is drawn separately.
pub fn concord() -> TexId {
    gfx::cached("concord", || {
        let mut cv = Canvas::new(56, 56);
        cv.circle(28.0, 28.0, 25.0, c(0x8A8070))
            .circle(28.0, 28.0, 23.0, c(0xD8D0C0))
            .circle(28.0, 28.0, 17.0, c(0xA8A090))
            .circle(28.0, 28.0, 15.0, Rgba::TRANSPARENT);
        cv.shade(|x, y, col| {
            let (dx, dy) = (x as f32 - 27.5, y as f32 - 27.5);
            if dx * dx + dy * dy < 225.0 { Rgba::TRANSPARENT } else { col }
        });
        for i in 0..8 {
            let a = i as f32 * TAU / 8.0 + PI / 8.0;
            cv.line(
                (28.0 + a.cos() * 17.0) as i32,
                (28.0 + a.sin() * 17.0) as i32,
                (28.0 + a.cos() * 24.0) as i32,
                (28.0 + a.sin() * 24.0) as i32,
                c(0xC8A050),
            );
        }
        for (x, y) in [(28.0, 6.0), (50.0, 28.0), (28.0, 50.0), (6.0, 28.0)] {
            cv.circle(x, y, 3.5, c(0x3A3040));
        }
        cv.outline();
        cv
    })
}

/// A ghost ship's cannon on a timber carriage.
pub fn cannon() -> TexId {
    gfx::cached("cannon", || {
        let mut cv = Canvas::new(18, 14);
        cv.rect(2, 8, 14, 4, c(0x5A3A24)).rect(3, 12, 3, 2, c(0x2A1A10)).rect(12, 12, 3, 2, c(0x2A1A10));
        cv.rect(4, 3, 12, 6, c(0x3A3440)).rect(15, 4, 3, 4, c(0x2A2430)).rect(5, 4, 9, 1, c(0x6A6478));
        cv.rect(16, 5, 1, 2, c(0x8A60B0));
        cv.outline();
        cv
    })
}

/// Blighted ground: dark veins that pulse purple across four frames.
pub fn blight_tile() -> TexId {
    gfx::cached("blight_tile", || {
        let mut cv = Canvas::new(64, 16);
        for f in 0..4 {
            for y in 0..16 {
                for x in 0..16 {
                    let vein = (x as f32 * 0.55 + (y as f32 * 0.4).cos() * 2.2).sin().abs();
                    let pulse = 0.5 + 0.5 * (f as f32 * PI / 2.0 + (x + y) as f32 * 0.3).sin();
                    let col = if vein < 0.18 {
                        Rgba::rgb(120, 40, 170).lerp(Rgba::rgb(220, 140, 255), pulse)
                    } else {
                        Rgba::rgb(40, 10, 50).mul(0.55)
                    };
                    cv.set(f * 16 + x, y, col);
                }
            }
        }
        cv
    })
}

pub fn tide_water() -> TexId {
    gfx::cached("tidewater", || {
        let mut cv = Canvas::new(64, 16);
        for f in 0..4 {
            for y in 0..16 {
                for x in 0..16 {
                    let wave = ((x + f * 4) as f32 * 0.4 + y as f32 * 0.9).sin();
                    let col =
                        if wave > 0.85 { Rgba::rgb(150, 200, 230).mul(0.9) } else { Rgba::rgb(40, 90, 150).mul(0.85) };
                    cv.set(f * 16 + x, y, col);
                }
            }
        }
        cv
    })
}

pub fn ripple() -> TexId {
    gfx::cached("ripple", || {
        let mut cv = Canvas::new(64, 12);
        for f in 0..4 {
            let r = (3 + f * 3) as f32;
            let mut a = 0.0f32;
            while a < TAU {
                cv.set(
                    ((f * 16 + 8) as f32 + a.cos() * r.min(7.5)) as i32,
                    (6.0 + a.sin() * r * 0.4) as i32,
                    Rgba::WHITE.mul(1.0 - f as f32 * 0.2),
                );
                a += 0.05;
            }
        }
        cv
    })
}
