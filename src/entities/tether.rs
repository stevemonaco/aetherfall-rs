//! The Aether Tether's hook: flies out along the aim, latches onto the first anchor, enemy or object it meets, and
//! either pulls the player across or reels the target in. Infused with the current element on contact.

use bevy::math::Vec2;

use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId, PLAYER_ID, TetherResponse};
use crate::art::combat as art;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, safe_dir, snap};
use crate::core::gfx::Batch;
use crate::entity_base;
use crate::gameplay::combat::{Faction, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::map::TILE;

const SPEED: f32 = 430.0;

pub struct TetherHook {
    base: EntityBase,
    velocity: Vec2,
    travelled: f32,
    hold: f32,
    returning: bool,
    latched: bool,
    reeling: Option<EntityId>,
}

impl TetherHook {
    pub fn new(center: Vec2, aim: Vec2) -> TetherHook {
        TetherHook {
            base: EntityBase { pos: center + aim * 6.0, width: 6.0, height: 6.0, ..EntityBase::default() },
            velocity: aim * SPEED,
            travelled: 0.0,
            hold: 0.0,
            returning: false,
            latched: false,
            reeling: None,
        }
    }

    fn range(world: &World) -> f32 {
        168.0 * (1.0 + world.state.mods().tether_range)
    }

    fn end_reel(&self, world: &mut World) {
        if let Some(id) = self.reeling {
            world.with_as::<Enemy, _>(id, |e, _| e.end_reel());
        }
    }

    /// Lets go of whatever the hook holds and removes it.
    pub fn release(&mut self, world: &mut World) {
        self.end_reel(world);
        self.base.removed = true;
    }

    fn finish(&mut self, world: &mut World) {
        self.end_reel(world);
        self.base.removed = true;
        world.with_player(|p, _| p.hook_done());
    }

    fn latch(&mut self, world: &mut World, id: EntityId, tether_box: Box2) {
        self.base.pos = tether_box.center();
        let faction = world.get(id).and_then(|e| e.faction());
        if let Some(f) = faction
            && f != Faction::Player
            && world.state.mode != Element::None
        {
            world.with_player(|p, w| {
                let hit = p.make_hit(w, 0.3, HitKind::Tether, None, 1.0, 1.2, 0.0);
                p.strike_target(w, id, hit);
            });
        }
        if world.get(id).is_none_or(|e| e.base().removed) {
            self.returning = true;
            return;
        }
        let response = world.with_entity(id, |e, w| e.on_tether(w)).unwrap_or(TetherResponse::None);
        match response {
            TetherResponse::PullPlayer => {
                let target = world.get(id).map(|e| e.base().pos).unwrap_or(self.base.pos);
                let dest = target + safe_dir(world.hero.pos - target, Vec2::Y) * 12.0;
                world.with_player(|p, _| p.start_zip(dest));
                self.latched = true;
                self.reeling = Some(id);
                self.hold = 0.05;
                audio::play(Sound::ShootWind, 0.8, 0.2);
            }
            TetherResponse::PullTarget => {
                self.latched = true;
                self.reeling = Some(id);
                self.hold = 0.28;
                audio::play(Sound::Swing, 0.8, -0.3);
            }
            TetherResponse::Ripped => {
                audio::play(Sound::Break, 0.9, 0.3);
                world.camera.shake(0.15, 2.0);
                self.returning = true;
            }
            TetherResponse::Activated => {
                audio::play(Sound::Door, 0.8, 0.0);
                self.returning = true;
            }
            TetherResponse::None => self.returning = true,
        }
    }
}

impl Entity for TetherHook {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        if self.latched {
            self.hold -= dt;
            if let Some(center) = self.reeling.and_then(|id| world.get(id)).map(|e| e.base().hitbox().center()) {
                self.base.pos = center;
            }
            if self.hold <= 0.0 {
                self.finish(world);
            }
            return;
        }

        if self.returning {
            let back = world.hero.center - self.base.pos;
            if back.length() < 10.0 {
                self.finish(world);
                return;
            }
            self.base.pos += back.normalize() * SPEED * 1.2 * dt;
            return;
        }

        let step = self.velocity * dt;
        self.base.pos += step;
        self.travelled += step.length();
        // Anchors block their own tile, so look for them before the wall check or a fast hook can skip past.
        let probe = Box2::from_center(self.base.pos, 8.0, 8.0);
        let hit = world
            .entities()
            .filter(|e| e.base().id != PLAYER_ID)
            .find_map(|e| e.tether_box().filter(|b| b.intersects(&probe)).map(|b| (e.base().id, b)));
        if let Some((id, tether_box)) = hit {
            self.latch(world, id, tether_box);
            return;
        }

        let t = TILE as f32;
        let wall = world.map.blocks_flying((self.base.pos.x / t).floor() as i32, ((self.base.pos.y + 8.0) / t).floor() as i32);
        if self.travelled > Self::range(world) || wall {
            self.returning = true;
            audio::play(Sound::Clink, 0.3, 0.2);
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let from = world.hero.center + Vec2::new(0.0, 1.0);
        let delta = self.base.pos - from;
        let links = (delta.length() / 5.0) as i32;
        let color = world.state.mode.light();
        for i in 1..links {
            let at = from + delta * (i as f32 / links as f32);
            batch.draw(art::chain_link(), snap(at - Vec2::splat(1.5)), color);
        }
        let rotation = self.velocity.y.atan2(self.velocity.x);
        batch.draw_ex(art::hook(), snap(self.base.pos), None, Rgba::WHITE, rotation, Vec2::splat(3.5), Vec2::ONE, false, false);
    }
}
