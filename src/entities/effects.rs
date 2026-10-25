//! Short-lived visual effects: particles, puffs, floating text and damage numbers.

use bevy::math::Vec2;

use crate::art::props;
use crate::core::color::Rgba;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::Batch;
use crate::core::{font, noise, palette, settings};
use crate::entities::entity::{Entity, EntityBase};
use crate::entity_base;
use crate::gameplay::element::Element;
use crate::scenes::world::World;

pub struct Particle {
    base: EntityBase,
    color: Rgba,
    life: f32,
    gravity: f32,
    spark: bool,
    velocity: Vec2,
    age: f32,
    z: f32,
    vz: f32,
}

impl Particle {
    pub fn new(pos: Vec2, velocity: Vec2, color: Rgba, life: f32) -> Particle {
        Particle {
            base: EntityBase::at(pos),
            color,
            life,
            gravity: 0.0,
            spark: false,
            velocity,
            age: 0.0,
            z: 0.0,
            vz: 0.0,
        }
    }

    pub fn spark(mut self) -> Particle {
        self.spark = true;
        self
    }

    /// Tosses the particle up at `vz`, then lets it fall back to the ground.
    pub fn falling(mut self, vz: f32, gravity: f32) -> Particle {
        self.vz = vz;
        self.gravity = gravity;
        self
    }

    pub fn burst(world: &mut World, at: Vec2, color: Rgba, count: i32, speed: f32, life: f32, sparks: bool) {
        for i in 0..count {
            let angle = world.rng.next_f32() * std::f32::consts::TAU;
            let v = Vec2::new(angle.cos(), angle.sin()) * speed * (0.4 + world.rng.next_f32() * 0.8);
            let mut p = Particle::new(at, v, color, life * (0.6 + world.rng.next_f32() * 0.6));
            p.spark = sparks && i % 3 == 0;
            world.add(p);
        }
    }
}

impl Entity for Particle {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        self.base.pos += self.velocity * dt;
        self.velocity *= 1.0 - dt * 2.5;
        self.vz -= self.gravity * dt;
        self.z = (self.z + self.vz * dt).max(0.0);
        if self.age >= self.life {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let pos = snap(self.base.pos - Vec2::new(0.0, self.z));
        let fade = 1.0 - self.age / self.life;
        let color = self.color.mul((fade * 2.0).min(1.0));
        if self.spark {
            batch.draw(props::spark(), pos - Vec2::ONE, color);
        } else {
            batch.fill_rect(pos.x, pos.y, 1.0, 1.0, color);
        }
    }
}

const PUFF_FRAME: f32 = 0.06;

pub struct Puff {
    base: EntityBase,
    tint: Rgba,
    age: f32,
}

impl Puff {
    pub fn new(center: Vec2, tint: Rgba) -> Puff {
        Puff { base: EntityBase::at(center), tint, age: 0.0 }
    }
}

impl Entity for Puff {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age >= PUFF_FRAME * 5.0 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let frame = ((self.age / PUFF_FRAME) as i32).min(4);
        batch.draw_src(
            props::puff(),
            snap(self.base.pos - Vec2::new(8.0, 8.0)),
            Some(IRect::new(frame * 16, 0, 16, 16)),
            self.tint,
        );
    }
}

pub struct FloatingText {
    base: EntityBase,
    text: String,
    color: Rgba,
    age: f32,
}

impl FloatingText {
    pub fn new(pos: Vec2, text: impl Into<String>, color: Rgba) -> FloatingText {
        FloatingText { base: EntityBase::at(pos), text: text.into(), color, age: 0.0 }
    }
}

impl Entity for FloatingText {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        self.base.pos.y -= if self.age < 0.3 { 40.0 } else { 6.0 } * dt;
        if self.age > 0.9 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        font::draw_centered(batch, &self.text, snap(self.base.pos), self.color, Some(palette::TEXT_SHADOW));
    }
}

/// A pop-up damage figure, coloured by element; crits are bigger, healing is green.
pub struct DamageNumber {
    base: EntityBase,
    text: String,
    color: Rgba,
    big: bool,
    velocity: Vec2,
    age: f32,
}

impl DamageNumber {
    pub fn new(at: Vec2, amount: i32, element: Element, crit: bool) -> DamageNumber {
        DamageNumber::styled(at, amount, element, crit, false, false, "")
    }

    pub fn hurt(at: Vec2, amount: i32, element: Element, crit: bool) -> DamageNumber {
        DamageNumber::styled(at, amount, element, crit, true, false, "")
    }

    pub fn heal(at: Vec2, amount: i32) -> DamageNumber {
        DamageNumber::styled(at, amount, Element::None, false, false, true, "")
    }

    pub fn styled(
        at: Vec2,
        amount: i32,
        element: Element,
        crit: bool,
        hurts_player: bool,
        heal: bool,
        suffix: &str,
    ) -> DamageNumber {
        let color = if heal {
            palette::EXPERIENCE
        } else if hurts_player {
            palette::LIFE
        } else if element == Element::None {
            Rgba::WHITE
        } else {
            element.light()
        };
        let jitter = (noise::hash01(at.x as i32, at.y as i32, amount) - 0.5) * 40.0;
        DamageNumber {
            base: EntityBase::at(at),
            text: format!("{amount}{suffix}"),
            color,
            big: crit,
            velocity: Vec2::new(jitter, -70.0),
            age: 0.0,
        }
    }
}

impl Entity for DamageNumber {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        let t = (self.age / 0.35).min(1.0);
        self.base.pos += self.velocity * (1.0 - t) * dt;
        if self.age > 0.8 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        if !settings::current().damage_numbers {
            return;
        }
        let alpha = ((0.8 - self.age) * 5.0).min(1.0);
        let at = snap(self.base.pos);
        if self.big {
            let w = font::measure(&self.text) * 2;
            let pos = at - Vec2::new(w as f32 / 2.0, 4.0);
            font::draw_scaled(batch, &self.text, pos + Vec2::ONE, palette::TEXT_SHADOW.mul(alpha), 2);
            font::draw_scaled(batch, &self.text, pos, self.color.mul(alpha), 2);
        } else {
            font::draw_centered(batch, &self.text, at, self.color.mul(alpha), Some(palette::TEXT_SHADOW.mul(alpha)));
        }
    }
}
