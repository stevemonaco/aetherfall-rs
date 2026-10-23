//! Screen-wide atmosphere: falling leaves, snow, ash, fog banks, fireflies and so on. Motes live in view space and
//! scroll against the camera so they read as part of the world. Glowing kinds draw after lighting.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use crate::art::combat as art;
use crate::core::color::Rgba;
use crate::core::geom::snap;
use crate::core::gfx::Batch;
use crate::core::rng::Rng;
use crate::scenes::world::{SCREEN_HEIGHT, SCREEN_WIDTH};

const W: i32 = SCREEN_WIDTH;
const H: i32 = SCREEN_HEIGHT;

struct Mote {
    pos: Vec2,
    vel: Vec2,
    phase: f32,
    size: f32,
}

pub struct Ambience {
    kinds: Vec<(String, Vec<Mote>)>,
    rng: Rng,
    last_camera: Option<Vec2>,
}

impl Ambience {
    pub fn new(weather: Option<&str>, theme: &str) -> Ambience {
        let kinds = weather.unwrap_or(match theme {
            "field" => "leaves",
            "marsh" => "fog,fireflies",
            "snow" => "snow",
            "cave" => "dust",
            "temple" => "bubbles",
            "tower" => "sparks",
            "citadel" => "clouds,motes",
            "ash" => "ash,embers",
            _ => "",
        });
        let mut ambience = Ambience { kinds: Vec::new(), rng: Rng::from_time(), last_camera: None };
        for kind in kinds.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            let count = match kind {
                "snow" => 70,
                "rain" => 110,
                "ash" => 60,
                "dust" => 30,
                "leaves" | "petals" => 14,
                "bubbles" => 18,
                "fog" => 7,
                "fireflies" => 18,
                "motes" => 26,
                "sparks" => 14,
                "embers" => 22,
                "clouds" => 5,
                _ => 0,
            };
            let motes = (0..count).map(|_| ambience.spawn(kind, true)).collect();
            ambience.kinds.push((kind.to_string(), motes));
        }
        ambience
    }

    fn spawn(&self, kind: &str, anywhere: bool) -> Mote {
        let r = &self.rng;
        let mut m = Mote {
            phase: r.next_f32() * TAU,
            size: 1.0 + r.next_f32(),
            pos: Vec2::new(r.below(W) as f32, if anywhere { r.below(H) as f32 } else { -8.0 }),
            vel: Vec2::ZERO,
        };
        m.vel = match kind {
            "snow" => Vec2::new((-6 + r.below(12)) as f32, (18 + r.below(16)) as f32),
            "rain" => Vec2::new(-30.0, (220 + r.below(60)) as f32),
            "ash" => Vec2::new((8 + r.below(10)) as f32, (10 + r.below(10)) as f32),
            "leaves" | "petals" => Vec2::new((12 + r.below(10)) as f32, (14 + r.below(10)) as f32),
            "dust" => Vec2::new(r.range(-4, 5) as f32, r.range(-3, 4) as f32),
            "bubbles" => Vec2::new(r.range(-3, 4) as f32, (-10 - r.below(10)) as f32),
            "fog" => Vec2::new((5 + r.below(6)) as f32, 0.0),
            "fireflies" => Vec2::new(r.range(-8, 9) as f32, r.range(-8, 9) as f32),
            "motes" => Vec2::new(r.range(-5, 6) as f32, (-6 - r.below(8)) as f32),
            "sparks" | "embers" => Vec2::new(r.range(-6, 7) as f32, (-14 - r.below(12)) as f32),
            "clouds" => Vec2::new((6 + r.below(6)) as f32, 0.0),
            _ => Vec2::ZERO,
        };
        if matches!(kind, "fog" | "clouds") {
            m.size = 1.4 + r.next_f32() * 1.4;
        }
        if m.vel.y < 0.0 && !anywhere {
            m.pos.y = (H + 8) as f32;
        }
        m
    }

    pub fn update(&mut self, dt: f32, camera: Vec2) {
        let scroll = self.last_camera.map(|last| camera - last).unwrap_or(Vec2::ZERO);
        self.last_camera = Some(camera);
        for (kind, motes) in &mut self.kinds {
            let parallax = if kind == "clouds" { 0.6 } else { 1.0 };
            let margin = if matches!(kind.as_str(), "fog" | "clouds") { 80.0 } else { 10.0 };
            for m in motes.iter_mut() {
                m.phase += dt;
                let sway = if matches!(kind.as_str(), "leaves" | "petals" | "snow") {
                    Vec2::new((m.phase * 2.0).sin() * 10.0, 0.0)
                } else {
                    Vec2::ZERO
                };
                if matches!(kind.as_str(), "fireflies" | "dust") {
                    m.vel += Vec2::new(self.rng.next_f32() - 0.5, self.rng.next_f32() - 0.5) * 20.0 * dt;
                }
                m.pos += (m.vel + sway) * dt - scroll * parallax;
                let (w, h) = (W as f32, H as f32);
                if m.pos.x < -margin {
                    m.pos.x += w + margin * 2.0;
                }
                if m.pos.x > w + margin {
                    m.pos.x -= w + margin * 2.0;
                }
                if m.pos.y < -margin {
                    m.pos.y += h + margin * 2.0;
                }
                if m.pos.y > h + margin {
                    m.pos.y -= h + margin * 2.0;
                }
            }
        }
    }

    /// Weather that should be shaded by the area's lighting.
    pub fn draw_lit(&self, batch: &mut Batch) {
        for (kind, motes) in &self.kinds {
            for m in motes {
                let p = snap(m.pos);
                match kind.as_str() {
                    "snow" => {
                        let s = if m.size > 1.5 { 2.0 } else { 1.0 };
                        batch.fill_rect(p.x, p.y, s, s, Rgba::WHITE.mul(0.9));
                    }
                    "rain" => batch.fill_rect(p.x, p.y, 1.0, 5.0, Rgba::rgb(170, 200, 255).mul(0.5)),
                    "ash" => batch.fill_rect(p.x, p.y, 1.0 + (m.size - 1.0).trunc(), 1.0, Rgba::rgb(70, 60, 60).mul(0.75)),
                    "leaves" | "petals" => {
                        let color = if kind == "leaves" {
                            if m.size > 1.5 { Rgba::rgb(150, 200, 80) } else { Rgba::rgb(220, 170, 60) }
                        } else {
                            Rgba::rgb(255, 200, 220)
                        };
                        let flip = (m.phase * 4.0).sin() > 0.0;
                        batch.fill_rect(p.x, p.y, if flip { 3.0 } else { 2.0 }, if flip { 2.0 } else { 3.0 }, color);
                        batch.fill_rect(p.x, p.y + if flip { 1.0 } else { 2.0 }, 1.0, 1.0, color.lerp(Rgba::BLACK, 0.35));
                    }
                    "dust" => batch.fill_rect(p.x, p.y, 1.0, 1.0, Rgba::rgb(200, 190, 170).mul(0.25 + 0.2 * (m.phase * 2.0).sin())),
                    "bubbles" => {
                        batch.fill_rect(p.x, p.y, 2.0, 2.0, Rgba::rgb(190, 240, 255).mul(0.35));
                        batch.fill_rect(p.x, p.y, 1.0, 1.0, Rgba::WHITE.mul(0.5));
                    }
                    "fog" | "clouds" => {
                        let tint = if kind == "fog" { Rgba::rgb(200, 220, 210).mul(0.09) } else { Rgba::WHITE.mul(0.16) };
                        batch.draw_ex(art::cloud(), p, None, tint, 0.0, Vec2::new(12.0, 10.0), Vec2::splat(m.size * 2.2), false, false);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Self-lit motes drawn on top of the lighting pass.
    pub fn draw_emissive(&self, batch: &mut Batch) {
        for (kind, motes) in &self.kinds {
            for m in motes {
                let p = snap(m.pos);
                let pulse = 0.5 + 0.5 * (m.phase * 3.0).sin();
                match kind.as_str() {
                    "fireflies" => {
                        batch.fill_rect(p.x - 1.0, p.y - 1.0, 3.0, 3.0, Rgba::rgb(200, 255, 120).mul(0.15 * pulse));
                        batch.fill_rect(p.x, p.y, 1.0, 1.0, Rgba::rgb(230, 255, 160).mul(pulse));
                    }
                    "motes" => batch.fill_rect(p.x, p.y, 1.0, 1.0, Rgba::rgb(255, 230, 255).mul(0.3 + 0.5 * pulse)),
                    "sparks" => batch.fill_rect(p.x, p.y, 1.0, 2.0, Rgba::rgb(200, 190, 255).mul(0.3 + 0.5 * pulse)),
                    "embers" => batch.fill_rect(p.x, p.y, 1.0, 1.0, Rgba::rgb(255, 150, 60).mul(0.4 + 0.6 * pulse)),
                    _ => {}
                }
            }
        }
    }
}
