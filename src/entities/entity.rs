//! The entity model. Entities live in the world's list and are updated one at a time: the world takes the entity out
//! of its slot, calls it with `&mut World`, and puts it back. While an entity runs it can reach every other entity
//! through the world, but nothing can call back into it; anything that must react to it (a parried attacker, a
//! reflected shot) reads the response fields on [`Hit`] after the call returns.

use std::any::Any;

use bevy::math::Vec2;

use crate::core::color::Rgba;
use crate::core::geom::Box2;
use crate::core::gfx::Batch;
use crate::gameplay::combat::{Faction, Hit};
use crate::scenes::world::World;
use crate::world::map::TILE;

pub type EntityId = u32;

/// The player always has this id, so hits and effects can refer to them like any entity.
pub const PLAYER_ID: EntityId = 1;

/// What the Aether Tether does when its hook latches onto something.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TetherResponse {
    None,
    PullPlayer,
    PullTarget,
    Ripped,
    Activated,
}

#[derive(Clone, Debug)]
pub struct EntityBase {
    pub id: EntityId,
    /// The bottom-centre point of the entity, where it touches the ground.
    pub pos: Vec2,
    pub width: f32,
    pub height: f32,
    pub solid: bool,
    pub removed: bool,
}

impl Default for EntityBase {
    fn default() -> Self {
        EntityBase { id: 0, pos: Vec2::ZERO, width: 12.0, height: 10.0, solid: false, removed: false }
    }
}

impl EntityBase {
    pub fn at(pos: Vec2) -> EntityBase {
        EntityBase { pos, ..EntityBase::default() }
    }

    pub fn hitbox(&self) -> Box2 {
        Box2::new(self.pos.x - self.width / 2.0, self.pos.y - self.height, self.width, self.height)
    }

    pub fn tile_pos(&self) -> (i32, i32) {
        ((self.pos.x / TILE as f32).floor() as i32, ((self.pos.y - 3.0) / TILE as f32).floor() as i32)
    }
}

pub trait Entity: Any {
    fn base(&self) -> &EntityBase;
    fn base_mut(&mut self) -> &mut EntityBase;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// 0 draws flat on the ground, 1 is y-sorted with other actors, 2 draws above everything.
    fn layer(&self) -> i32 {
        1
    }

    fn sort_y(&self) -> f32 {
        self.base().pos.y
    }

    fn can_interact(&self, _world: &World) -> bool {
        false
    }

    fn added(&mut self, _world: &mut World) {}
    fn update(&mut self, _world: &mut World, _dt: f32) {}
    fn draw(&self, world: &World, batch: &mut Batch);
    fn interact(&mut self, _world: &mut World) {}

    /// Called when the player overlaps this entity; used by triggers and pickups.
    fn touch(&mut self, _world: &mut World) {}

    /// Light this entity casts in dark areas: radius in pixels and colour.
    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        None
    }

    /// The faction of a damageable entity, or None if hits pass through it.
    fn faction(&self) -> Option<Faction> {
        None
    }

    fn hurtbox(&self) -> Box2 {
        self.base().hitbox()
    }

    /// Applies a hit and returns whether it connected (so non-piercing projectiles know to stop).
    fn take_hit(&mut self, _world: &mut World, _hit: &mut Hit) -> bool {
        false
    }

    /// Entities that catch fire from burning tiles or fire hits.
    fn flammable(&self) -> bool {
        false
    }

    fn ignite(&mut self, _world: &mut World) {}

    /// Entities that react when electricity reaches them through water or metal.
    fn conductor(&self) -> bool {
        false
    }

    fn energize(&mut self, _world: &mut World) {}

    /// Whether ground hazards (burning grass, live water) reach this entity.
    fn grounded(&self) -> bool {
        true
    }

    /// Where the tether's hook can latch on, for tetherable entities.
    fn tether_box(&self) -> Option<Box2> {
        None
    }

    fn on_tether(&mut self, _world: &mut World) -> TetherResponse {
        TetherResponse::None
    }
}

/// Implements the base and downcasting accessors for a struct with a `base: EntityBase` field.
#[macro_export]
macro_rules! entity_base {
    () => {
        fn base(&self) -> &$crate::entities::entity::EntityBase {
            &self.base
        }
        fn base_mut(&mut self) -> &mut $crate::entities::entity::EntityBase {
            &mut self.base
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    };
}
