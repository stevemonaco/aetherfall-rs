//! Hazards of the Blighted expeditions (the Inverted Spire's lightning grid, ghost-ship cannons) and the Blight Gate.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::combat_effects::LightningStrike;
use super::effects::Particle;
use super::entity::{Entity, EntityBase};
use super::projectile::Projectile;
use crate::art::{bosses, combat as art_combat};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, IRect, safe_dir, snap};
use crate::core::gfx::{self, Batch};
use crate::entity_base;
use crate::gameplay::combat::{self, Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::chemistry::conductive;
use crate::world::map::{EntityData, TILE, tile_feet};

/// The Inverted Spire's lightning rods: on a fixed beat, lightning falls on alternating 2x2 blocks of an area, so the
/// whole floor has to be crossed in time with it.
pub struct StormGrid {
    base: EntityBase,
    area: IRect,
    period: f32,
    time: f32,
    beat: i32,
}

impl StormGrid {
    pub fn new(data: &EntityData) -> StormGrid {
        StormGrid {
            base: EntityBase::default(),
            area: IRect::new(data.x as i32, data.y as i32, data.int("w", 6), data.int("h", 6)),
            period: data.float("period", 2.4),
            time: data.float("offset", 0.0),
            beat: 0,
        }
    }
}

impl Entity for StormGrid {
    entity_base!();

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if self.time < self.period {
            return;
        }
        self.time -= self.period;
        self.beat += 1;
        let stats = combat::environment(world.chem.zone_level);
        let area = self.area;
        for by in (area.y..area.bottom()).step_by(2) {
            for bx in (area.x..area.right()).step_by(2) {
                if ((bx - area.x) / 2 + (by - area.y) / 2 + self.beat) % 2 != 0 {
                    continue;
                }
                let at = Vec2::new(((bx + 1) * TILE) as f32, ((by + 1) * TILE) as f32);
                let hit = Hit {
                    power: 0.9,
                    element: Element::Thunder,
                    from: at,
                    kind: HitKind::Environment,
                    status_power: 0.6,
                    knockback: 120.0,
                    ..Hit::new(Faction::Enemy, stats)
                };
                world.add(StrikeMarker::new(at, 1.1, hit));
            }
        }
    }

    fn draw(&self, _world: &World, _batch: &mut Batch) {}
}

/// A ring that tightens on a spot, then calls down lightning there.
pub struct StrikeMarker {
    base: EntityBase,
    delay: f32,
    hit: Hit,
    age: f32,
}

impl StrikeMarker {
    const RADIUS: f32 = 14.0;

    pub fn new(at: Vec2, delay: f32, hit: Hit) -> StrikeMarker {
        StrikeMarker { base: EntityBase::at(at), delay, hit, age: 0.0 }
    }
}

impl Entity for StrikeMarker {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if self.age < self.delay {
            return;
        }
        self.base.removed = true;
        let pos = self.base.pos;
        world.add(LightningStrike::new(pos));
        audio::play(Sound::Clink, 1.0, -0.7);
        world.camera.shake(0.12, 1.0);
        if world.hero.pos.distance(pos) < Self::RADIUS + 4.0 {
            let mut hit = self.hit.clone();
            world.hit_player(&mut hit);
        }
        let (tx, ty) = self.base.tile_pos();
        if conductive(world.map.at(tx, ty)) {
            world.chem_electrify(tx, ty, None);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let t = self.age / self.delay;
        let blink = ((self.age * (6.0 + t * 14.0)) as i32 % 2) as f32;
        let color = Rgba::rgb(255, 240, 120).lerp(Rgba::WHITE, t).mul(0.4 + 0.6 * blink);
        let pos = self.base.pos;
        let mut a = 0.0f32;
        while a < TAU {
            batch.fill_rect(pos.x + a.cos() * Self::RADIUS, pos.y + a.sin() * Self::RADIUS * 0.5, 1.0, 1.0, color);
            a += 0.2;
        }
        let inner = Self::RADIUS * t;
        let mut a = 0.0f32;
        while a < TAU {
            batch.fill_rect(pos.x + a.cos() * inner, pos.y + a.sin() * inner * 0.5, 1.0, 1.0, color.mul(0.7));
            a += 0.35;
        }
    }
}

/// A ghost ship's cannon. It lobs blight shot at the player; Wind or a guard can send it back.
pub struct Cannon {
    base: EntityBase,
    interval: f32,
    timer: f32,
    hp: i32,
    flash: f32,
}

impl Cannon {
    pub fn new(data: &EntityData) -> Cannon {
        Cannon {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 14.0,
                height: 8.0,
                ..EntityBase::default()
            },
            interval: data.float("interval", 3.5),
            timer: data.float("offset", 1.0),
            hp: 6,
            flash: 0.0,
        }
    }
}

impl Entity for Cannon {
    entity_base!();

    fn faction(&self) -> Option<Faction> {
        Some(if self.hp > 0 { Faction::Enemy } else { Faction::Neutral })
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 8.0, self.base.pos.y - 14.0, 16.0, 14.0)
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
        if self.hp <= 0 {
            return;
        }
        self.timer -= dt;
        let pos = self.base.pos;
        if self.timer > 0.0 || world.hero.pos.distance(pos) > 220.0 {
            return;
        }
        self.timer = self.interval;
        let stats = combat::environment(world.chem.zone_level);
        let muzzle = pos - Vec2::new(0.0, 10.0);
        let dir = safe_dir(world.hero.center - pos, Vec2::Y);
        world.add(Projectile::enemy_shot("dark", muzzle, dir, stats, 1.1, 0.9));
        audio::play(Sound::Break, 0.6, -0.6);
        Particle::burst(world, muzzle, Rgba::rgb(120, 100, 140), 6, 40.0, 0.4, false);
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if self.hp <= 0 || !hit.is_player() {
            return false;
        }
        self.hp -= if matches!(hit.kind, HitKind::Finisher | HitKind::Art) { 2 } else { 1 };
        self.flash = 0.12;
        audio::play(Sound::Clink, 0.8, -0.2);
        if self.hp <= 0 {
            let (tx, ty) = self.base.tile_pos();
            world.map.block(tx, ty, false);
            self.base.solid = false;
            Particle::burst(world, self.base.pos - Vec2::new(0.0, 8.0), Rgba::rgb(150, 130, 160), 20, 80.0, 0.6, true);
            audio::sfx(Sound::Break);
        }
        true
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let art = if self.hp > 0 { bosses::cannon() } else { art_combat::rubble() };
        let (w, h) = gfx::size(art);
        let flip = world.hero.pos.x < self.base.pos.x;
        let tex = if self.flash > 0.0 { gfx::silhouette(art) } else { art };
        let at = snap(self.base.pos - Vec2::new(w as f32 / 2.0, h as f32));
        batch.draw_ex(tex, at, None, Rgba::WHITE, 0.0, Vec2::ZERO, Vec2::ONE, flip, false);
    }
}

/// A torn place in the world that leads into the Blighted expeditions.
pub struct BlightGate {
    base: EntityBase,
    time: f32,
}

impl BlightGate {
    pub fn new(data: &EntityData) -> BlightGate {
        BlightGate {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 14.0,
                height: 6.0,
                ..EntityBase::default()
            },
            time: 0.0,
        }
    }
}

impl Entity for BlightGate {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((56.0, Rgba::rgb(200, 120, 255)))
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if world.rng.next_f32() < 0.25 {
            let offset = Vec2::new(world.rng.range(-8, 9) as f32, -6.0 - world.rng.below(16) as f32);
            let mote = Particle::new(self.base.pos + offset, Vec2::new(0.0, -16.0), Rgba::rgb(190, 110, 240), 0.7);
            world.add(mote.spark());
        }
    }

    fn interact(&mut self, world: &mut World) {
        world.open_blight_gate();
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let frame = (self.time * 8.0) as i32 % 4;
        let src = Some(IRect::new(frame * 32, 0, 32, 40));
        batch.draw_src(
            art_combat::rift_portal(),
            snap(self.base.pos - Vec2::new(16.0, 38.0)),
            src,
            Rgba::rgb(255, 140, 170),
        );
    }
}
