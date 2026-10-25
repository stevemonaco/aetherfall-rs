//! A monster rift: a tear where stolen light leaks out. When the player comes near it splits open into a breach and a
//! fixed swarm pours out of its edge all at once; once every foe it sent has fallen it seals itself, releasing
//! whatever it held (a villager, a building, a soul, a cleared path...). Sealing sets the flag `rift:id`.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::effects::{FloatingText, Particle, Puff};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use crate::art::{combat as art_combat, props as art};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::{self, Batch};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::game_data::data;
use crate::gameplay::pacts;
use crate::gameplay::trial_mods::TrialMod;
use crate::scenes::world::World;
use crate::world::map::{EntityData, tile_center};

/// How far the breach spreads from the rift.
pub const BREACH_RADIUS: f32 = 76.0;
/// Seconds between foes as the swarm pours out.
const SPAWN_GAP: f32 = 0.12;
const PURPLE: Rgba = Rgba::rgb(200, 120, 255);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RiftState {
    Dormant,
    Open,
    Sealed,
}

pub struct Rift {
    base: EntityBase,
    id: String,
    kinds: Vec<String>,
    then: Vec<String>,
    quota: i32,
    level: Option<i32>,
    restores: Option<String>,
    soul: Option<String>,
    elite: Option<String>,
    wake: f32,
    spawned: Vec<EntityId>,
    /// Foes still to pour out of the breach.
    pending: i32,
    state: RiftState,
    time: f32,
    spawn_timer: f32,
    spawn_count: i32,
}

impl Rift {
    pub fn new(data: &EntityData) -> Rift {
        let level = data.int("level", 0);
        Rift {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 8.0),
                width: 20.0,
                height: 12.0,
                ..EntityBase::default()
            },
            id: data.str("id").map(str::to_string).unwrap_or_else(|| format!("{}_{}", data.x, data.y)),
            kinds: data
                .str_or("kinds", "")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            then: data.strings("do"),
            quota: data.int("count", if data.str("elite").is_some() { 3 } else { 6 }),
            level: (level > 0).then_some(level),
            restores: data.str("restores").map(str::to_string),
            soul: data.str("soul").map(str::to_string),
            elite: data.str("elite").map(str::to_string),
            wake: data.float("wake", 64.0),
            spawned: Vec::new(),
            pending: 0,
            state: RiftState::Dormant,
            time: 0.0,
            spawn_timer: 0.0,
            spawn_count: 0,
        }
    }

    pub fn flag(&self) -> String {
        format!("rift:{}", self.id)
    }

    fn quota(&self, world: &World) -> i32 {
        let swarm = if pacts::active(&world.state, "swarm") { 1.5 } else { 1.0 };
        (self.quota as f32 * swarm) as i32 + i32::from(self.elite.is_some())
    }

    /// Foes still to beat before the rift seals: those yet to pour out and those still standing.
    fn remaining(&self) -> i32 {
        self.pending + self.spawned.len() as i32
    }

    /// The kinds this rift pours out; without a list it borrows the map's own foes.
    fn pick_kind(&self, world: &World) -> String {
        if !self.kinds.is_empty() {
            return self.kinds[world.rng.below(self.kinds.len() as i32) as usize].clone();
        }
        let roster: Vec<&str> =
            world.data.entities.iter().filter(|e| e.kind == "enemy").filter_map(|e| e.str("kind")).collect();
        if roster.is_empty() {
            return self.elite.clone().unwrap_or_else(|| "slime".to_string());
        }
        roster[world.rng.below(roster.len() as i32) as usize].to_string()
    }

    fn open(&mut self, world: &mut World) {
        self.state = RiftState::Open;
        self.pending = self.quota(world);
        self.spawn_timer = 0.5;
        audio::play(Sound::BossRoar, 0.6, 0.5);
        world.camera.shake(0.4, 1.5);
        world.add(BreachZone::new(self.base.pos, self.flag()));
        world.add(FloatingText::new(self.base.pos - Vec2::new(0.0, 40.0), "A breach tears open!", PURPLE));
    }

    fn pour(&mut self, world: &mut World, dt: f32) {
        self.spawned.retain(|&id| world.get_as::<Enemy>(id).is_some_and(|e| !e.base.removed && e.hp > 0));
        if self.remaining() <= 0 {
            self.seal(world);
            return;
        }
        if self.pending <= 0 {
            return;
        }
        self.spawn_timer -= dt;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = SPAWN_GAP;
        let leader = self.elite.is_some() && self.spawn_count == 0;
        let kind = if leader { self.elite.clone().unwrap_or_default() } else { self.pick_kind(world) };
        self.pending -= 1;
        self.spawn_count += 1;
        let Some(def) = data().enemies.get(&kind) else { return };
        let angle = world.rng.next_f32() * TAU;
        let reach = BREACH_RADIUS * (0.55 + world.rng.next_f32() * 0.4);
        let at = world.nearest_open(self.base.pos + Vec2::new(angle.cos() * reach, angle.sin() * reach * 0.7));
        let zone = world.chem.zone_level;
        let level = self.level.unwrap_or(if zone > 1 { zone } else { def.level });
        let elite = leader
            || (pacts::active(&world.state, "swarm") && self.spawn_count % 3 == 0)
            || (world.trial_mod(TrialMod::EliteTide) && self.spawn_count % 4 == 0);
        let enemy = if elite { Enemy::elite(def, at, Some(level), world) } else { Enemy::new(def, at, Some(level)) };
        let id = world.add(enemy.hunting());
        self.spawned.push(id);
        world.add(Puff::new(at - Vec2::new(0.0, 6.0), Rgba::rgb(200, 140, 255)));
        audio::play(Sound::Door, 0.4, 0.6);
    }

    fn seal(&mut self, world: &mut World) {
        self.state = RiftState::Sealed;
        world.state.flags.insert(self.flag());
        world.rank.style(3.0);
        world.hit_stop(0.12);
        world.flash(Rgba::rgb(230, 200, 255), 0.4);
        world.camera.shake(0.4, 2.0);
        audio::play(Sound::BossDie, 0.6, 0.4);
        let center = self.base.pos - Vec2::new(0.0, 14.0);
        Particle::burst(world, center, Rgba::rgb(230, 190, 255), 40, 110.0, 0.9, true);
        world.add(RestoreMote::new(center));
        world.add(FloatingText::new(self.base.pos - Vec2::new(0.0, 40.0), "The breach seals.", palette::HIGHLIGHT));
        if let Some(r) = &self.restores {
            world.toast(format!("Restored: {r}"));
        }
        if let Some(soul) = self.soul.clone() {
            world.free_soul(&soul);
        }
        if !self.then.is_empty() {
            let actions = self.then.clone();
            world.run_actions(&actions);
        }
        self.base.removed = true;
    }
}

impl Entity for Rift {
    entity_base!();

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        (self.state != RiftState::Sealed).then_some((70.0, PURPLE))
    }

    fn added(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag()) {
            self.base.removed = true;
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        match self.state {
            RiftState::Dormant if world.hero.pos.distance(self.base.pos) < self.wake => self.open(world),
            RiftState::Open => self.pour(world, dt),
            _ => {}
        }
        if self.state != RiftState::Sealed && world.rng.next_f32() < 0.25 {
            let a = world.rng.next_f32() * TAU;
            let center = self.base.pos - Vec2::new(0.0, 14.0);
            let at = center + Vec2::new(a.cos(), a.sin() * 0.6) * 26.0;
            world.add(Particle::new(at, (center - at) * 1.5, Rgba::rgb(190, 110, 255), 0.6));
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let center = self.base.pos - Vec2::new(0.0, 14.0);
        let pulse = 1.0 + (self.time * 4.0).sin() * 0.08;
        let size = if self.state == RiftState::Dormant { 0.75 } else { 1.1 };
        let frame = (self.time * 8.0) as i32 % 4;
        batch.draw(art::shadow(24), snap(self.base.pos - Vec2::new(12.0, 4.0)), Rgba::WHITE.mul(0.6));
        batch.draw_ex(
            art_combat::rift_portal(),
            snap(center),
            Some(IRect::new(frame * 32, 0, 32, 40)),
            Rgba::WHITE,
            0.0,
            Vec2::new(16.0, 20.0),
            Vec2::splat(size * pulse),
            false,
            false,
        );
        if self.state == RiftState::Open {
            font::draw_centered(
                batch,
                &self.remaining().to_string(),
                snap(center - Vec2::new(0.0, 30.0)),
                Rgba::rgb(230, 200, 255),
                Some(palette::TEXT_SHADOW),
            );
        }
    }
}

/// The breach's mark on the ground: it spreads open when the rift tears and fades once the rift is sealed.
pub struct BreachZone {
    base: EntityBase,
    flag: String,
    age: f32,
    fade: f32,
}

impl BreachZone {
    pub fn new(center: Vec2, flag: String) -> BreachZone {
        BreachZone { base: EntityBase::at(center), flag, age: 0.0, fade: 1.0 }
    }
}

impl Entity for BreachZone {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if world.state.flags.contains(&self.flag) {
            self.fade -= dt * 1.5;
            if self.fade <= 0.0 {
                self.base.removed = true;
            }
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let grow = (self.age / 0.8).min(1.0);
        let r = BREACH_RADIUS * (1.0 - (1.0 - grow) * (1.0 - grow));
        let pulse = 0.85 + (self.age * 3.0).sin() * 0.15;
        let disc = art_combat::disc();
        let (dw, _) = gfx::size(disc);
        let scale = Vec2::new(r * 2.0 / dw as f32, r * 1.4 / dw as f32);
        let origin = Vec2::splat(dw as f32 / 2.0);
        batch.draw_ex(disc, self.base.pos, None, PURPLE.mul(0.12 * self.fade), 0.0, origin, scale, false, false);
        let ring = art_combat::ring();
        let (rw, _) = gfx::size(ring);
        let ring_scale = scale * dw as f32 / rw as f32;
        let rorigin = Vec2::splat(rw as f32 / 2.0);
        batch.draw_ex(
            ring,
            self.base.pos,
            None,
            PURPLE.mul(0.6 * pulse * self.fade),
            0.0,
            rorigin,
            ring_scale,
            false,
            false,
        );
    }
}
/// A mote of restored light that rises from a sealed rift and streaks away toward what it restored.
pub struct RestoreMote {
    base: EntityBase,
    age: f32,
}

impl RestoreMote {
    pub fn new(start: Vec2) -> RestoreMote {
        RestoreMote { base: EntityBase::at(start), age: 0.0 }
    }
}

impl Entity for RestoreMote {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((40.0, Rgba::rgb(255, 240, 200)))
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        self.base.pos += Vec2::new((self.age * 6.0).sin() * 30.0, -40.0 - self.age * 160.0) * dt;
        if world.rng.next_f32() < 0.8 {
            world.add(Particle::new(self.base.pos, Vec2::ZERO, Rgba::rgb(255, 240, 200), 0.5).spark());
        }
        if self.age > 2.0 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        batch.draw(art::spark(), snap(self.base.pos - Vec2::ONE), Rgba::WHITE);
        batch.draw(art::orb(), snap(self.base.pos - Vec2::splat(4.0)), Rgba::rgb(255, 240, 200).mul(0.6));
    }
}
