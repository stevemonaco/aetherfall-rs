//! Visual combat effects: sword arcs, afterimages, telegraphs, shockwaves, lightning and beams.

use bevy::math::Vec2;

use crate::art::combat as art;
use crate::art::hero::{HeroPose, hero_sprite};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::{self, Batch};
use crate::core::{font, noise, palette};
use crate::entities::entity::{Entity, EntityBase};
use crate::entity_base;
use crate::gameplay::skills::SkillDef;
use crate::gameplay::element::Element;
use crate::scenes::world::World;

fn simple(pos: Vec2) -> EntityBase {
    EntityBase::at(pos)
}

/// A sword arc that follows the player for a few frames.
pub struct SlashEffect {
    base: EntityBase,
    aim: Vec2,
    element: Element,
    radius: f32,
    heavy: bool,
    flip: bool,
    age: f32,
}

const SLASH_LIFE: f32 = 0.16;

impl SlashEffect {
    pub fn new(center: Vec2, aim: Vec2, element: Element, radius: f32, heavy: bool, flip: bool) -> SlashEffect {
        SlashEffect { base: simple(center), aim, element, radius, heavy, flip, age: 0.0 }
    }
}

impl Entity for SlashEffect {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        self.base.pos = world.hero.center;
        if self.age >= SLASH_LIFE {
            self.base.removed = true;
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let texture = art::slash_arc(self.element, self.heavy);
        let (w, h) = gfx::size(texture);
        let frames = w / h;
        let frame = (frames - 1).min((self.age / SLASH_LIFE * frames as f32) as i32);
        let scale = self.radius / (h as f32 / 2.0);
        let rotation = self.aim.y.atan2(self.aim.x);
        batch.draw_ex(
            texture,
            snap(world.hero.center + self.aim * 4.0),
            Some(IRect::new(frame * h, 0, h, h)),
            Rgba::WHITE.mul(1.0 - self.age / SLASH_LIFE * 0.4),
            rotation,
            Vec2::splat(h as f32 / 2.0),
            Vec2::splat(scale),
            false,
            self.flip,
        );
    }
}

/// A full circle of blade light around the player, for spin attacks.
pub struct SpinEffect {
    base: EntityBase,
    element: Element,
    life: f32,
    age: f32,
}

impl SpinEffect {
    pub fn new(center: Vec2, element: Element, life: f32) -> SpinEffect {
        SpinEffect { base: simple(center), element, life, age: 0.0 }
    }
}

impl Entity for SpinEffect {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age >= self.life {
            self.base.removed = true;
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let texture = art::slash_arc(self.element, true);
        let (_, h) = gfx::size(texture);
        let rotation = self.age * 28.0;
        for i in 0..2 {
            batch.draw_ex(
                texture,
                snap(world.hero.center),
                Some(IRect::new(h, 0, h, h)),
                Rgba::WHITE.mul(1.0 - self.age / self.life),
                rotation + i as f32 * std::f32::consts::PI,
                Vec2::splat(h as f32 / 2.0),
                Vec2::splat(32.0 / (h as f32 / 2.0)),
                false,
                false,
            );
        }
    }
}

/// A fading copy of the player's silhouette left behind by dodges and dashes.
pub struct Afterimage {
    base: EntityBase,
    facing: Direction,
    color: Rgba,
    age: f32,
}

impl Afterimage {
    pub fn new(feet: Vec2, facing: Direction, mode: Element) -> Afterimage {
        Afterimage { base: simple(feet), facing, color: mode.glow(), age: 0.0 }
    }
}

impl Entity for Afterimage {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 0.22 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let color = self.color.mul(0.5 * (1.0 - self.age / 0.22));
        hero_sprite::draw(batch, self.base.pos, self.facing, HeroPose::Dodge, 0, color, true, 1.0, 0.0);
    }
}

/// The skill's name and level flashing above the player as it is performed.
pub struct SkillBanner {
    base: EntityBase,
    skill: &'static SkillDef,
    level: i32,
    age: f32,
}

impl SkillBanner {
    pub fn new(at: Vec2, skill: &'static SkillDef, level: i32) -> SkillBanner {
        SkillBanner { base: simple(at), skill, level, age: 0.0 }
    }
}

impl Entity for SkillBanner {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 0.9 {
            self.base.removed = true;
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let alpha = ((0.9 - self.age) * 4.0).min(1.0);
        let at = world.hero.pos - Vec2::new(0.0, 36.0 + (self.age * 30.0).min(6.0));
        let text = match self.level {
            1 => self.skill.name.to_string(),
            2 => format!("{} II", self.skill.name),
            _ => format!("{} III", self.skill.name),
        };
        font::draw_centered(
            batch,
            &text,
            snap(at),
            self.skill.element.light().mul(alpha),
            Some(palette::TEXT_SHADOW.mul(alpha)),
        );
    }
}

/// An expanding ring on the ground, for bursts and landings.
pub struct Shockwave {
    base: EntityBase,
    radius: f32,
    color: Rgba,
    age: f32,
}

impl Shockwave {
    pub fn new(center: Vec2, radius: f32, color: Rgba) -> Shockwave {
        Shockwave { base: simple(center), radius, color, age: 0.0 }
    }
}

impl Entity for Shockwave {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 0.3 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let t = self.age / 0.3;
        let r = self.radius * (0.3 + 0.7 * t);
        let ring = art::ring();
        let (w, _) = gfx::size(ring);
        let w = w as f32;
        batch.draw_ex(
            ring,
            self.base.pos - Vec2::new(0.0, 2.0),
            None,
            self.color.mul(1.0 - t),
            0.0,
            Vec2::splat(w / 2.0),
            Vec2::new(r * 2.0 / w, r * 1.3 / w),
            false,
            false,
        );
    }
}

/// A telegraph: a circle on the ground that fills as the attack it warns of approaches.
pub struct GroundMarker {
    base: EntityBase,
    radius: f32,
    duration: f32,
    color: Rgba,
    age: f32,
}

impl GroundMarker {
    pub fn new(center: Vec2, radius: f32, duration: f32, color: Rgba) -> GroundMarker {
        GroundMarker { base: simple(center), radius, duration, color, age: 0.0 }
    }
}

impl Entity for GroundMarker {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age >= self.duration {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let t = (self.age / self.duration).min(1.0);
        let disc = art::disc();
        let (dw, _) = gfx::size(disc);
        let dw = dw as f32;
        let origin = Vec2::splat(dw / 2.0);
        let scale = Vec2::new(self.radius * 2.0 / dw, self.radius * 1.4 / dw);
        let pos = self.base.pos;
        batch.draw_ex(disc, pos, None, self.color.mul(0.18), 0.0, origin, scale, false, false);
        batch.draw_ex(disc, pos, None, self.color.mul(0.3), 0.0, origin, scale * t, false, false);
        let ring = art::ring();
        let (rw, _) = gfx::size(ring);
        let rw = rw as f32;
        batch.draw_ex(ring, pos, None, self.color.mul(0.8), 0.0, Vec2::splat(rw / 2.0), scale * dw / rw, false, false);
    }
}

/// A telegraph line from an attacker along the path a charge or beam will take.
pub struct LineMarker {
    base: EntityBase,
    to: Vec2,
    width: f32,
    duration: f32,
    color: Rgba,
    age: f32,
}

impl LineMarker {
    pub fn new(from: Vec2, to: Vec2, width: f32, duration: f32, color: Rgba) -> LineMarker {
        LineMarker { base: simple(from), to, width, duration, color, age: 0.0 }
    }
}

impl Entity for LineMarker {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age >= self.duration {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let t = (self.age / self.duration).min(1.0);
        let from = self.base.pos;
        let delta = self.to - from;
        let (length, angle) = (delta.length(), delta.y.atan2(delta.x));
        let origin = Vec2::new(0.0, 0.5);
        batch.draw_ex(
            gfx::pixel(),
            from,
            None,
            self.color.mul(0.2),
            angle,
            origin,
            Vec2::new(length, self.width),
            false,
            false,
        );
        batch.draw_ex(
            gfx::pixel(),
            from,
            None,
            self.color.mul(0.45),
            angle,
            origin,
            Vec2::new(length * t, self.width * 0.5),
            false,
            false,
        );
    }
}

/// A bolt from the top of the screen down to a point, fading quickly.
pub struct LightningStrike {
    base: EntityBase,
    age: f32,
}

impl LightningStrike {
    pub fn new(target: Vec2) -> LightningStrike {
        LightningStrike { base: simple(target), age: 0.0 }
    }
}

impl Entity for LightningStrike {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((60.0, Rgba::rgb(255, 250, 200)))
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 0.25 {
            self.base.removed = true;
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let alpha = 1.0 - self.age / 0.25;
        let mut y = world.camera.view().y as f32;
        let mut x = self.base.pos.x;
        while y < self.base.pos.y {
            let nx = self.base.pos.x + (noise::hash01(y as i32, (self.age * 20.0) as i32, 9) - 0.5) * 8.0;
            batch.fill_rect(x.min(nx), y, (nx - x).abs() + 2.0, 4.0, Rgba::WHITE.mul(alpha));
            batch.fill_rect(x.min(nx) - 1.0, y, (nx - x).abs() + 4.0, 4.0, Rgba::rgb(255, 230, 90).mul(alpha * 0.5));
            x = nx;
            y += 4.0;
        }
    }
}

/// Draws a straight line of the given width.
pub fn draw_line(batch: &mut Batch, a: Vec2, b: Vec2, color: Rgba, width: f32) {
    let delta = b - a;
    batch.draw_ex(
        gfx::pixel(),
        a,
        None,
        color,
        delta.y.atan2(delta.x),
        Vec2::new(0.0, 0.5),
        Vec2::new(delta.length(), width),
        false,
        false,
    );
}

/// A jagged electric arc between two points.
pub struct LightningArc {
    base: EntityBase,
    to: Vec2,
    age: f32,
}

impl LightningArc {
    pub fn new(from: Vec2, to: Vec2) -> LightningArc {
        LightningArc { base: simple(from), to, age: 0.0 }
    }
}

impl Entity for LightningArc {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 0.2 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let alpha = 1.0 - self.age / 0.2;
        let from = self.base.pos;
        let mut prev = from;
        let steps = ((from.distance(self.to) / 6.0) as i32).max(2);
        let d = self.to - from;
        let normal = (Vec2::new(-d.y, d.x) + Vec2::splat(0.0001)).normalize();
        for i in 1..=steps {
            let mut p = from.lerp(self.to, i as f32 / steps as f32);
            if i < steps {
                p += normal * (noise::hash01(i, (self.age * 40.0) as i32, 77) - 0.5) * 8.0;
            }
            draw_line(batch, prev, p, Rgba::rgb(255, 240, 120).mul(alpha), 3.0);
            draw_line(batch, prev, p, Rgba::WHITE.mul(alpha), 1.0);
            prev = p;
        }
    }
}

/// A straight beam that fades out.
pub struct Beam {
    base: EntityBase,
    from: Vec2,
    to: Vec2,
    color: Rgba,
    age: f32,
}

impl Beam {
    pub fn new(from: Vec2, to: Vec2, color: Rgba) -> Beam {
        Beam { base: simple((from + to) / 2.0), from, to, color, age: 0.0 }
    }
}

impl Entity for Beam {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((50.0, self.color))
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 0.4 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let t = self.age / 0.4;
        let width = 10.0 * (1.0 - t) + 2.0;
        draw_line(batch, self.from, self.to, self.color.mul(1.0 - t), width + 4.0);
        draw_line(batch, self.from, self.to, Rgba::WHITE.mul(1.0 - t), width);
    }
}

/// An exclamation mark that pops above an enemy when it notices you.
pub struct Exclaim {
    base: EntityBase,
    owner: crate::entities::entity::EntityId,
    owner_height: f32,
    last: Vec2,
    age: f32,
}

impl Exclaim {
    pub fn new(owner: crate::entities::entity::EntityId, at: Vec2, owner_height: f32) -> Exclaim {
        Exclaim { base: simple(at), owner, owner_height, last: at, age: 0.0 }
    }
}

impl Entity for Exclaim {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if let Some(owner) = world.get(self.owner) {
            self.last = owner.base().pos;
        }
        self.base.pos = self.last;
        if self.age > 0.6 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let hop = if self.age < 0.15 { self.age / 0.15 * 4.0 } else { 4.0 };
        let at = self.last - Vec2::new(0.0, self.owner_height + 14.0 + hop);
        font::draw_centered(batch, "!", snap(at), palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
    }
}

/// Three claw streaks for an enemy's melee swipe.
pub struct ClawEffect {
    base: EntityBase,
    dir: Vec2,
    color: Rgba,
    age: f32,
}

impl ClawEffect {
    pub fn new(at: Vec2, dir: Vec2, color: Rgba) -> ClawEffect {
        ClawEffect { base: simple(at), dir, color, age: 0.0 }
    }
}

impl Entity for ClawEffect {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 0.15 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let normal = Vec2::new(-self.dir.y, self.dir.x);
        let alpha = 1.0 - self.age / 0.15;
        for i in -1..=1 {
            let start = self.base.pos + normal * i as f32 * 4.0 - self.dir * 6.0;
            draw_line(
                batch,
                start,
                start + self.dir * 12.0 + normal * 2.0,
                self.color.lerp(Rgba::WHITE, 0.5).mul(alpha),
                1.5,
            );
        }
    }
}
