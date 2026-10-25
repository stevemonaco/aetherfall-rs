//! The flourish when the hero changes element: a whirling gust, a pillar of flame, a water spout or a lightning strike,
//! each shaped so the new mode reads at a glance.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::combat_effects::{LightningArc, LightningStrike, Shockwave, draw_line};
use super::effects::Particle;
use super::entity::{Entity, EntityBase};
use crate::core::color::{Rgba, c};
use crate::core::geom::snap;
use crate::core::gfx::Batch;
use crate::core::noise;
use crate::entity_base;
use crate::gameplay::element::Element;
use crate::scenes::world::World;

const LIFE: f32 = 0.55;

pub struct ModeBurst {
    base: EntityBase,
    element: Element,
    age: f32,
}

impl ModeBurst {
    pub fn new(feet: Vec2, element: Element) -> ModeBurst {
        ModeBurst { base: EntityBase::at(feet), element, age: 0.0 }
    }

    fn t(&self) -> f32 {
        (self.age / LIFE).min(1.0)
    }

    fn draw_gust(&self, batch: &mut Batch) {
        let t = self.t();
        let fade = 1.0 - t;
        for ring in 0..3 {
            let radius = 5.0 + t * 16.0 + ring as f32 * 4.0;
            let lift = ring as f32 * 7.0 + t * 10.0;
            let spin = self.age * (14.0 - ring as f32 * 3.0) + ring as f32 * 2.1;
            for i in 0..3 {
                let a = spin + i as f32 * TAU / 3.0;
                let at = |a: f32| self.base.pos + Vec2::new(a.cos() * radius, a.sin() * radius * 0.45 - lift);
                let color = if i == 0 { c(0xE8FFF8) } else { c(0x8AEAD0) };
                draw_line(batch, at(a), at(a + 0.9), color.mul(fade), 1.0);
                draw_line(batch, at(a + 0.9), at(a + 1.3), color.mul(fade * 0.5), 1.0);
            }
        }
    }

    fn draw_flame(&self, batch: &mut Batch) {
        let t = self.t();
        let rise = (t * 3.0).min(1.0);
        let height = 34.0 * rise * (1.0 - (t - 0.6).max(0.0) * 1.6);
        for (half, color) in [(6.0, c(0xC83A1E)), (4.0, c(0xFF8A2A)), (2.2, c(0xFFD24A)), (1.0, c(0xFFF4C8))] {
            let mut y = 0.0;
            while y < height {
                let taper = 1.0 - y / height.max(1.0);
                let flicker = (noise::hash01(y as i32, (self.age * 30.0) as i32, 31) - 0.5) * 2.0;
                let w = (half * (0.4 + taper * 0.8) + flicker * taper).max(0.5);
                let at = self.base.pos - Vec2::new(w, y + 2.0);
                batch.fill_rect(at.x.round(), at.y.round(), (w * 2.0).round(), 2.0, color.mul(0.6));
                y += 2.0;
            }
        }
    }

    fn draw_spout(&self, batch: &mut Batch) {
        let t = self.t();
        let rise = (t * 2.5).min(1.0);
        let height = 30.0 * rise * (1.0 - (t - 0.7).max(0.0) * 2.5);
        let mut y = 0.0;
        while y < height {
            let sway = (y * 0.35 + self.age * 18.0).sin() * 1.5;
            let w = 3.0 + (y / 30.0) * 3.0;
            let at = self.base.pos + Vec2::new(sway - w, -y - 2.0);
            let band = if (y as i32 / 2 + (self.age * 20.0) as i32) % 3 == 0 { c(0x9AD8FF) } else { c(0x4A9AF0) };
            batch.fill_rect(at.x.round(), at.y.round(), (w * 2.0).round(), 2.0, band.mul(0.6));
            y += 2.0;
        }
        if height > 4.0 {
            let top = snap(self.base.pos - Vec2::new(6.0, height + 3.0));
            batch.fill_rect(top.x, top.y, 12.0, 2.0, Rgba::WHITE.mul(0.9));
        }
    }

    fn draw_sparks(&self, batch: &mut Batch) {
        let fade = 1.0 - self.t();
        let center = self.base.pos - Vec2::new(0.0, 8.0);
        let frame = (self.age * 24.0) as i32;
        for i in 0..6 {
            let a = noise::hash01(i, frame, 41) * TAU;
            let r = 8.0 + noise::hash01(i, frame, 43) * 10.0;
            let mid = center + Vec2::new(a.cos(), a.sin()) * r * 0.5 + Vec2::new(0.0, (i % 2) as f32 * 2.0 - 1.0);
            let end = center + Vec2::new(a.cos(), a.sin()) * r;
            draw_line(batch, center, mid, c(0xFFE45A).mul(fade), 1.0);
            draw_line(batch, mid, end, Rgba::WHITE.mul(fade), 1.0);
        }
    }
}

impl Entity for ModeBurst {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((48.0 * (1.0 - self.t()), self.element.glow()))
    }

    fn added(&mut self, world: &mut World) {
        let feet = self.base.pos;
        match self.element {
            Element::Wind => {
                for i in 0..12 {
                    let a = i as f32 * TAU / 12.0;
                    let tangent = Vec2::new(-a.sin(), a.cos() * 0.5);
                    let at = feet + Vec2::new(a.cos() * 8.0, a.sin() * 4.0 - 6.0);
                    world.add(Particle::new(at, tangent * 90.0 - Vec2::new(0.0, 25.0), c(0xE8FFF8), 0.5));
                }
            }
            Element::Fire => {
                world.add(Shockwave::new(feet, 20.0, c(0xFF8A2A)));
                for _ in 0..14 {
                    let at = feet + Vec2::new(world.rng.range(-6, 7) as f32, -2.0);
                    let v = Vec2::new(world.rng.range(-15, 16) as f32, -70.0 - world.rng.below(50) as f32);
                    let color = if world.rng.next_f32() < 0.5 { c(0xFFD24A) } else { c(0xFF8A2A) };
                    world.add(Particle::new(at, v, color, 0.5).spark());
                }
            }
            Element::Water => {
                world.add(Shockwave::new(feet, 22.0, c(0x9AD8FF)));
                for i in 0..16 {
                    let a = i as f32 * TAU / 16.0;
                    let v = Vec2::new(a.cos() * 45.0, a.sin() * 20.0);
                    world.add(Particle::new(feet - Vec2::new(0.0, 4.0), v, c(0x9AD8FF), 0.5).falling(60.0, 260.0));
                }
            }
            Element::Thunder => {
                world.add(LightningStrike::new(feet));
                let center = feet - Vec2::new(0.0, 8.0);
                for i in 0..4 {
                    let a = i as f32 * TAU / 4.0 + 0.4;
                    world.add(LightningArc::new(center, center + Vec2::new(a.cos() * 20.0, a.sin() * 12.0)));
                }
                world.camera.shake(0.08, 1.0);
            }
            _ => Particle::burst(world, feet - Vec2::new(0.0, 8.0), self.element.glow(), 14, 50.0, 0.4, true),
        }
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age >= LIFE {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        match self.element {
            Element::Wind => self.draw_gust(batch),
            Element::Fire => self.draw_flame(batch),
            Element::Water => self.draw_spout(batch),
            Element::Thunder => self.draw_sparks(batch),
            _ => {}
        }
    }
}
