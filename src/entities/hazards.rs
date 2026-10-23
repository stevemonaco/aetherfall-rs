//! Lingering hazards: the Cyclone and fire patches left by Arts, decoys, steam and mist, and drifting gas.

use bevy::math::Vec2;

use super::combat_effects::Shockwave;
use super::effects::Particle;
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId, PLAYER_ID};
use crate::art::hero::{HeroPose, hero_sprite};
use crate::art::{combat as art, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{Box2, IRect, safe_dir, snap};
use crate::core::gfx::Batch;
use crate::entity_base;
use crate::gameplay::combat::{self, CombatStats, Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::map::{EntityData, tile_feet};

/// A slow tornado that drags enemies toward its eye and carries fire it passes over.
pub struct Cyclone {
    base: EntityBase,
    velocity: Vec2,
    age: f32,
    tick: f32,
    burning: bool,
}

impl Cyclone {
    pub fn new(at: Vec2, direction: Vec2) -> Cyclone {
        Cyclone {
            base: EntityBase::at(at + Vec2::new(0.0, 8.0)),
            velocity: direction * 55.0,
            age: 0.0,
            tick: 0.0,
            burning: false,
        }
    }
}

impl Entity for Cyclone {
    entity_base!();

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        self.burning.then_some((40.0, Rgba::rgb(255, 150, 60)))
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        self.base.pos += self.velocity * dt;
        let (tx, ty) = self.base.tile_pos();
        if self.age > 2.6 || world.map.blocks_flying(tx, ty) {
            self.base.removed = true;
            Particle::burst(
                world,
                self.base.pos - Vec2::new(0.0, 10.0),
                Rgba::rgb(200, 255, 240),
                12,
                50.0,
                0.4,
                false,
            );
            return;
        }
        let dir = self.velocity.normalize_or_zero();
        world.chem_fan(tx, ty, Some(dir));
        if !self.burning && world.is_burning(tx, ty) {
            self.burning = true;
            audio::play(Sound::ShootFire, 0.7, 0.0);
        }
        if self.burning {
            world.chem_ignite(tx, ty);
        }

        let eye = self.base.pos;
        for id in world.ids_of::<Enemy>() {
            world.with_as::<Enemy, _>(id, |enemy, w| {
                if enemy.heavy() {
                    return;
                }
                let pull = eye - enemy.base().pos;
                if pull.length() > 4.0 && pull.length() < 48.0 {
                    let flying = enemy.is_flying();
                    w.move_entity(enemy.base_mut(), pull.normalize() * 70.0 * dt, flying, false, false);
                }
            });
        }
        for id in world.ids_of::<GasCloud>() {
            world.with_as::<GasCloud, _>(id, |gas, _| {
                if gas.base.pos.distance(eye) < 40.0 {
                    gas.push(dir * 40.0);
                }
            });
        }

        self.tick -= dt;
        if self.tick <= 0.0 {
            self.tick = 0.35;
            let element = if self.burning { Element::Fire } else { Element::Wind };
            world.with_player(|p, w| {
                p.strike_area(w, eye - Vec2::new(0.0, 8.0), 20.0, None, move |p, w| {
                    p.make_hit(w, 0.45, HitKind::Art, Some(element), 1.0, 1.2, 30.0)
                })
            });
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let frame = (self.age * 16.0) as i32 % 4;
        let tint = if self.burning { Rgba::rgb(255, 170, 110) } else { Rgba::WHITE };
        let src = IRect::new(frame * 16, 0, 16, 22);
        batch.draw_ex(
            props::tornado(),
            snap(self.base.pos - Vec2::new(16.0, 40.0)),
            Some(src),
            tint,
            0.0,
            Vec2::ZERO,
            Vec2::splat(2.0),
            false,
            false,
        );
    }
}

/// A patch of flame left on the ground by fire Arts; burns enemies that stand in it.
pub struct FirePatch {
    base: EntityBase,
    life: f32,
    age: f32,
    tick: f32,
}

impl FirePatch {
    pub fn new(at: Vec2, life: f32) -> FirePatch {
        FirePatch { base: EntityBase::at(at), life, age: 0.0, tick: 0.0 }
    }
}

impl Entity for FirePatch {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((24.0, Rgba::rgb(255, 140, 50)))
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if self.age >= self.life {
            self.base.removed = true;
            return;
        }
        self.tick -= dt;
        if self.tick > 0.0 {
            return;
        }
        self.tick = 0.4;
        let at = self.base.pos - Vec2::new(0.0, 4.0);
        world.with_player(|p, w| {
            p.strike_area(w, at, 9.0, None, |p, w| p.make_hit(w, 0.3, HitKind::Art, Some(Element::Fire), 0.3, 1.0, 0.0))
        });
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let alpha = ((self.life - self.age) * 2.0).min(1.0);
        let frame = (self.age * 12.0 + self.base.pos.x) as i32 % 4;
        let src = IRect::new(frame * 16, 0, 16, 22);
        batch.draw_src(
            art::flames(),
            snap(self.base.pos - Vec2::new(8.0, 18.0)),
            Some(src),
            Rgba::WHITE.mul(alpha * 0.85),
        );
    }
}

/// A shimmering copy of the player that enemies attack instead for a few seconds.
pub struct Decoy {
    base: EntityBase,
    age: f32,
}

impl Decoy {
    pub fn new(at: Vec2) -> Decoy {
        Decoy { base: EntityBase::at(at), age: 0.0 }
    }
}

impl Entity for Decoy {
    entity_base!();

    fn added(&mut self, world: &mut World) {
        world.decoy = Some(self.base.id);
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if self.age > 3.0 {
            self.base.removed = true;
            if world.decoy == Some(self.base.id) {
                world.decoy = None;
            }
            Particle::burst(world, self.base.pos - Vec2::new(0.0, 8.0), Rgba::rgb(200, 220, 255), 10, 40.0, 0.4, false);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let alpha = 0.45 + (self.age * 12.0).sin() * 0.15;
        hero_sprite::draw(
            batch,
            self.base.pos,
            Direction::Down,
            HeroPose::Idle,
            0,
            Rgba::rgb(180, 210, 255).mul(alpha),
            true,
            1.0,
            0.0,
        );
    }
}

/// A puff of steam or mist: harmless, but enemies can't see through it.
pub struct SteamCloud {
    base: EntityBase,
    life: f32,
    mist: bool,
    age: f32,
    drift: Vec2,
}

impl SteamCloud {
    pub fn new(at: Vec2, mist: bool) -> SteamCloud {
        SteamCloud { base: EntityBase::at(at), life: if mist { 2.8 } else { 1.6 }, mist, age: 0.0, drift: Vec2::ZERO }
    }

    pub fn radius(&self) -> f32 {
        12.0
    }
}

impl Entity for SteamCloud {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn added(&mut self, world: &mut World) {
        self.drift = Vec2::new(world.rng.range(-8, 9) as f32, -10.0);
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.age += dt;
        self.base.pos += self.drift * dt;
        if self.age >= self.life {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let t = self.age / self.life;
        let alpha = if t < 0.2 { t / 0.2 } else { 1.0 - (t - 0.2) / 0.8 } * if self.mist { 0.55 } else { 0.7 };
        let tint = if self.mist { Rgba::rgb(190, 220, 255) } else { Rgba::WHITE };
        batch.draw(art::cloud(), snap(self.base.pos - Vec2::new(12.0, 20.0)), tint.mul(alpha));
    }
}

/// A drifting cloud of spores or swamp gas. Poisons whoever breathes it; fire detonates it; wind blows it away (into
/// enemies, ideally).
pub struct GasCloud {
    base: EntityBase,
    source: CombatStats,
    velocity: Vec2,
    age: f32,
    tick: f32,
}

impl GasCloud {
    pub fn new(at: Vec2, source: CombatStats, drift: Vec2) -> GasCloud {
        GasCloud { base: EntityBase::at(at), source, velocity: drift, age: 0.0, tick: 0.0 }
    }

    pub fn push(&mut self, velocity: Vec2) {
        self.velocity = velocity;
    }

    fn explode(&mut self, world: &mut World) {
        if self.base.removed {
            return;
        }
        self.base.removed = true;
        let pos = self.base.pos;
        world.add(Shockwave::new(pos, 34.0, Rgba::rgb(255, 160, 60)));
        Particle::burst(world, pos - Vec2::new(0.0, 8.0), Rgba::rgb(255, 180, 80), 26, 90.0, 0.5, true);
        world.camera.shake(0.2, 2.0);
        audio::play(Sound::Break, 0.9, 0.1);
        let (cx, cy) = self.base.tile_pos();
        for y in -1..=1 {
            for x in -1..=1 {
                world.chem_ignite(cx + x, cy + y);
            }
        }
        let mut stats = world.state.stats(world.state.mode);
        stats.atk = stats.atk.max(self.source.atk);
        for (id, faction, hurtbox) in world.damageables() {
            if id == self.base.id || hurtbox.center().distance(pos - Vec2::new(0.0, 8.0)) > 36.0 {
                continue;
            }
            let to_player = faction == Faction::Player;
            let mut hit = Hit {
                power: 2.2,
                element: Element::Fire,
                kind: HitKind::Environment,
                from: pos,
                knockback: 200.0,
                poise: 4.0,
                status_power: 2.0,
                ..Hit::new(
                    if to_player { Faction::Neutral } else { Faction::Player },
                    if to_player { self.source } else { stats },
                )
            };
            world.hit(id, &mut hit);
        }
        let chained: Vec<EntityId> =
            world.entities_of::<GasCloud>().filter(|g| g.base.pos.distance(pos) < 40.0).map(|g| g.base.id).collect();
        for id in chained {
            world.with_as::<GasCloud, _>(id, |g, w| g.explode(w));
        }
    }

    fn cloud_box(&self) -> Box2 {
        Box2::from_center(self.base.pos - Vec2::new(0.0, 8.0), 22.0, 16.0)
    }
}

impl Entity for GasCloud {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Neutral)
    }

    fn hurtbox(&self) -> Box2 {
        self.cloud_box()
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if hit.element == Element::Fire {
            self.explode(world);
            return true;
        }
        if hit.element == Element::Wind {
            self.push(safe_dir(self.base.pos - hit.from, Vec2::Y) * 70.0);
        }
        false
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        self.base.pos += self.velocity * dt;
        self.velocity *= 1.0 - dt * 0.8;
        let (tx, ty) = self.base.tile_pos();
        if world.is_burning(tx, ty) {
            self.explode(world);
            return;
        }
        if self.age > 9.0 {
            self.base.removed = true;
            return;
        }
        self.tick -= dt;
        if self.tick > 0.0 {
            return;
        }
        self.tick = 0.6;
        if world.hero.hurtbox.intersects(&self.cloud_box()) {
            let mut hit = Hit {
                power: 0.25,
                kind: HitKind::Environment,
                from: self.base.pos,
                knockback: 0.0,
                can_crit: false,
                ..Hit::new(Faction::Enemy, self.source)
            };
            world.hit(PLAYER_ID, &mut hit);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let alpha = (self.age * 3.0).min(1.0) * ((9.0 - self.age) / 1.5).min(1.0) * 0.75;
        let wobble = (self.age * 3.0).sin() * 1.5;
        batch.draw(
            art::cloud(),
            snap(self.base.pos - Vec2::new(12.0 + wobble, 20.0)),
            Rgba::rgb(190, 140, 230).mul(alpha),
        );
        batch.draw(
            art::cloud(),
            snap(self.base.pos - Vec2::new(8.0 - wobble, 16.0)),
            Rgba::rgb(160, 220, 120).mul(alpha * 0.6),
        );
    }
}

/// A crack in the ground that vents swamp gas every few seconds.
pub struct GasVent {
    base: EntityBase,
    interval: f32,
    timer: f32,
}

impl GasVent {
    pub fn new(data: &EntityData) -> GasVent {
        let interval = data.float("interval", 5.0);
        GasVent { base: EntityBase::at(tile_feet(data.x as i32, data.y as i32)), interval, timer: interval * 0.5 }
    }
}

impl Entity for GasVent {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.timer -= dt;
        if self.timer > 0.0 {
            return;
        }
        self.timer = self.interval;
        let pos = self.base.pos;
        if world.entities_of::<GasCloud>().filter(|g| g.base.pos.distance(pos) < 60.0).count() >= 3 {
            return;
        }
        let drift = Vec2::new(world.rng.range(-10, 11) as f32, world.rng.range(-10, 11) as f32);
        world.add(GasCloud::new(pos, combat::environment(world.chem.zone_level), drift));
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        batch.draw(art::vent(), snap(self.base.pos - Vec2::new(8.0, 10.0)), Rgba::WHITE);
    }
}
