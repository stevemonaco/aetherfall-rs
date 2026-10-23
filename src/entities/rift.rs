//! A monster rift: a tear where stolen light leaks out. Once the player comes near it pours out waves of enemies;
//! defeating its quota leaves it unstable, and striking it then seals it for good, releasing whatever it held (a
//! villager, a building, a soul...). Sealing sets the flag `rift:id`.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::effects::{FloatingText, Particle, Puff};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use crate::art::{combat as art_combat, props as art};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::{Box2, IRect, snap};
use crate::core::gfx::Batch;
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{Faction, Hit};
use crate::gameplay::game_data::data;
use crate::gameplay::{pacts, souls};
use crate::scenes::world::World;
use crate::world::map::{EntityData, tile_center};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RiftState {
    Dormant,
    Pouring,
    Unstable,
    Sealed,
}

pub struct Rift {
    base: EntityBase,
    id: String,
    kinds: Vec<String>,
    then: Vec<String>,
    quota: i32,
    alive: i32,
    level: Option<i32>,
    restores: Option<String>,
    soul: Option<String>,
    elite: Option<String>,
    wake: f32,
    spawned: Vec<EntityId>,
    state: RiftState,
    time: f32,
    spawn_timer: f32,
    flash: f32,
    killed: i32,
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
            kinds: data.str_or("kinds", "slime").split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
            then: data.strings("do"),
            quota: data.int("count", 6),
            alive: data.int("alive", 3),
            level: (level > 0).then_some(level),
            restores: data.str("restores").map(str::to_string),
            soul: data.str("soul").map(str::to_string),
            elite: data.str("elite").map(str::to_string),
            wake: data.float("wake", 120.0),
            spawned: Vec::new(),
            state: RiftState::Dormant,
            time: 0.0,
            spawn_timer: 0.0,
            flash: 0.0,
            killed: 0,
            spawn_count: 0,
        }
    }

    pub fn flag(&self) -> String {
        format!("rift:{}", self.id)
    }

    fn swarm(world: &World) -> bool {
        pacts::active(&world.state, "swarm")
    }

    fn quota(&self, world: &World) -> i32 {
        if Self::swarm(world) { self.quota * 3 / 2 } else { self.quota }
    }

    fn remaining(&self, world: &World) -> i32 {
        if self.elite.is_some() {
            if self.killed > 0 { 0 } else { 1 }
        } else {
            self.quota(world) - self.killed
        }
    }

    fn pour(&mut self, world: &mut World, dt: f32) {
        let mut i = self.spawned.len();
        while i > 0 {
            i -= 1;
            let id = self.spawned[i];
            match world.get_as::<Enemy>(id) {
                Some(e) if !e.base.removed => {}
                Some(e) => {
                    if e.hp <= 0 {
                        self.killed += 1;
                    }
                    self.spawned.remove(i);
                }
                None => {
                    // Removed enemies leave the world before the rift sees them; a missing one was defeated.
                    self.killed += 1;
                    self.spawned.remove(i);
                }
            }
        }

        if self.remaining(world) <= 0 {
            self.state = RiftState::Unstable;
            audio::play(Sound::ChargeLevel, 1.0, 0.3);
            world.add(FloatingText::new(self.base.pos - Vec2::new(0.0, 40.0), "The rift wavers... strike it!", palette::HIGHLIGHT));
            return;
        }

        let pending = if self.elite.is_some() {
            1 - self.spawned.len() as i32 - self.killed
        } else {
            self.alive.min(self.quota(world) - self.killed) - self.spawned.len() as i32
        };
        if pending <= 0 {
            return;
        }
        self.spawn_timer -= dt;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = 1.1;
        let kind = self.elite.clone().unwrap_or_else(|| self.kinds[world.rng.below(self.kinds.len() as i32) as usize].clone());
        let Some(def) = data().enemies.get(&kind) else { return };
        let angle = world.rng.next_f32() * TAU;
        let at = self.base.pos + Vec2::new(angle.cos() * 14.0, angle.sin() * 8.0 + 6.0);
        let zone = world.chem.zone_level;
        let level = self.level.unwrap_or(if zone > 1 { zone } else { def.level });
        let elite = self.elite.is_some() || (Self::swarm(world) && {
            self.spawn_count += 1;
            self.spawn_count % 3 == 0
        });
        let enemy = if elite { Enemy::elite(def, at, Some(level), world) } else { Enemy::new(def, at, Some(level)) };
        let id = world.add(enemy);
        self.spawned.push(id);
        world.add(Puff::new(at - Vec2::new(0.0, 6.0), Rgba::rgb(200, 140, 255)));
        audio::play(Sound::Door, 0.5, 0.6);
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
        if let Some(r) = &self.restores {
            world.toast(format!("Restored: {r}"));
        }
        if let Some(soul) = self.soul.as_deref().and_then(souls::get) {
            world.free_soul(soul);
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

    fn faction(&self) -> Option<Faction> {
        Some(if self.state == RiftState::Unstable { Faction::Neutral } else { Faction::Player })
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 12.0, self.base.pos.y - 28.0, 24.0, 28.0)
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        (self.state != RiftState::Sealed).then_some((70.0, Rgba::rgb(200, 120, 255)))
    }

    fn added(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag()) {
            self.base.removed = true;
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        self.flash = (self.flash - dt).max(0.0);
        match self.state {
            RiftState::Dormant => {
                if world.hero.pos.distance(self.base.pos) < self.wake {
                    self.state = RiftState::Pouring;
                    self.spawn_timer = 0.6;
                    audio::play(Sound::BossRoar, 0.5, 0.5);
                    world.camera.shake(0.3, 1.0);
                    world.add(FloatingText::new(self.base.pos - Vec2::new(0.0, 40.0), "A rift tears open!", Rgba::rgb(220, 160, 255)));
                }
            }
            RiftState::Pouring => self.pour(world, dt),
            RiftState::Unstable => {
                if world.rng.next_f32() < 0.3 {
                    let at = self.base.pos - Vec2::new(world.rng.range(-10, 11) as f32, 14.0 + world.rng.below(10) as f32);
                    world.add(Particle::new(at, Vec2::new(0.0, -20.0), Rgba::WHITE, 0.4).spark());
                }
            }
            RiftState::Sealed => {}
        }
        if self.state != RiftState::Sealed && world.rng.next_f32() < 0.25 {
            let a = world.rng.next_f32() * TAU;
            let center = self.base.pos - Vec2::new(0.0, 14.0);
            let at = center + Vec2::new(a.cos(), a.sin() * 0.6) * 26.0;
            world.add(Particle::new(at, (center - at) * 1.5, Rgba::rgb(190, 110, 255), 0.6));
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if hit.source != Faction::Player {
            return false;
        }
        if self.state != RiftState::Unstable {
            self.flash = 0.1;
            audio::play(Sound::Clink, 0.5, -0.3);
            return true;
        }
        self.seal(world);
        true
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let center = self.base.pos - Vec2::new(0.0, 14.0);
        let pulse = 1.0 + (self.time * 4.0).sin() * 0.08;
        let size = match self.state {
            RiftState::Unstable => 0.8 + (self.time * 18.0).sin() * 0.12,
            RiftState::Dormant => 0.75,
            _ => 1.0,
        };
        let frame = (self.time * 8.0) as i32 % 4;
        let tint = if self.flash > 0.0 {
            Rgba::WHITE
        } else if self.state == RiftState::Unstable {
            Rgba::WHITE.lerp(Rgba::rgb(255, 230, 255), 0.5)
        } else {
            Rgba::WHITE
        };
        batch.draw(art::shadow(24), snap(self.base.pos - Vec2::new(12.0, 4.0)), Rgba::WHITE.mul(0.6));
        let src = Some(IRect::new(frame * 32, 0, 32, 40));
        batch.draw_ex(art_combat::rift_portal(), snap(center), src, tint, 0.0, Vec2::new(16.0, 20.0), Vec2::splat(size * pulse), false, false);
        if self.state == RiftState::Pouring {
            let text = self.remaining(world).to_string();
            font::draw_centered(batch, &text, snap(center - Vec2::new(0.0, 30.0)), Rgba::rgb(230, 200, 255), Some(palette::TEXT_SHADOW));
        }
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
