//! Gold, hearts and item drops. They pop out, then drift toward the player once close.

use bevy::math::Vec2;

use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::Batch;
use crate::core::palette;
use crate::entities::effects::{DamageNumber, FloatingText};
use crate::entities::entity::{Entity, EntityBase};
use crate::entity_base;
use crate::gameplay::items;
use crate::scenes::world::{World, overflow_note};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PickupKind {
    Gold,
    Heart,
    Item(String),
}

pub struct Pickup {
    base: EntityBase,
    kind: PickupKind,
    amount: i32,
    age: f32,
    z: f32,
    vz: f32,
    scatter: Vec2,
}

impl Pickup {
    pub fn new(pos: Vec2, kind: PickupKind, amount: i32) -> Pickup {
        Pickup {
            base: EntityBase { pos, width: 10.0, height: 8.0, ..EntityBase::default() },
            kind,
            amount,
            age: 0.0,
            z: 6.0,
            vz: 50.0,
            scatter: Vec2::ZERO,
        }
    }

    pub fn gold(pos: Vec2, amount: i32) -> Pickup {
        Pickup::new(pos, PickupKind::Gold, amount)
    }

    pub fn heart(pos: Vec2, amount: i32) -> Pickup {
        Pickup::new(pos, PickupKind::Heart, amount)
    }

    pub fn item(pos: Vec2, item: &str) -> Pickup {
        Pickup::new(pos, PickupKind::Item(item.to_string()), 1)
    }

    fn expires(&self) -> bool {
        !matches!(self.kind, PickupKind::Item(_))
    }
}

impl Entity for Pickup {
    entity_base!();

    fn added(&mut self, world: &mut World) {
        let a = world.rng.next_f32() * std::f32::consts::TAU;
        self.scatter = Vec2::new(a.cos(), a.sin()) * (20 + world.rng.below(20)) as f32;
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        self.vz -= 300.0 * dt;
        self.z = (self.z + self.vz * dt).max(0.0);
        if self.z == 0.0 {
            self.vz = if self.vz.abs() > 30.0 { -self.vz * 0.4 } else { 0.0 };
        }
        if self.scatter.length_squared() > 1.0 {
            world.move_entity(&mut self.base, self.scatter * dt, false, false, false);
            self.scatter *= 1.0 - dt * 5.0;
        }
        let to_player = world.hero.pos - self.base.pos;
        if self.age > 0.4 && to_player.length() < 40.0 && !world.hero.dead && to_player.length() > 0.0 {
            self.base.pos += to_player.normalize() * to_player.length().min((60.0 + self.age * 60.0) * dt);
        }
        if self.age > 14.0 && self.expires() {
            self.base.removed = true;
        }
    }

    fn touch(&mut self, world: &mut World) {
        if self.age < 0.25 {
            return;
        }
        self.base.removed = true;
        match &self.kind {
            PickupKind::Heart => {
                world.heal_player(self.amount);
                audio::play(Sound::Heal, 0.6, 0.0);
                world.add(DamageNumber::heal(world.hero.center, self.amount));
            }
            PickupKind::Item(item) => {
                let got = world.receive(item, self.amount);
                audio::play(Sound::Pickup, 0.6, 0.0);
                let name = items::name_of(item);
                world.toast(overflow_note(&name, got, self.amount));
            }
            PickupKind::Gold => {
                world.state.gold += self.amount;
                audio::sfx(Sound::Coin);
                world.add(FloatingText::new(
                    self.base.pos - Vec2::new(0.0, 14.0),
                    format!("{}G", self.amount),
                    palette::GOLD,
                ));
            }
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        if self.age > 12.0 && self.expires() && (self.age * 12.0) as i32 % 2 == 0 {
            return;
        }
        batch.draw(props::shadow(8), snap(self.base.pos - Vec2::new(4.0, 2.0)), Rgba::WHITE);
        let pos = snap(self.base.pos - Vec2::new(4.0, 8.0 + self.z));
        match &self.kind {
            PickupKind::Heart => batch.draw(props::heart_pickup(), pos, Rgba::WHITE),
            PickupKind::Item(item) => {
                let bob = (self.age * 4.0).sin() * 1.5;
                batch.draw(props::icon(item), snap(pos - Vec2::new(2.0, 4.0 + bob)), Rgba::WHITE);
                if (self.age * 3.0) as i32 % 3 == 0 {
                    batch.draw(props::spark(), snap(pos + Vec2::new(6.0, -4.0 - bob)), Rgba::WHITE);
                }
            }
            PickupKind::Gold => batch.draw_src(
                props::coin(),
                pos,
                Some(IRect::new((self.age * 10.0) as i32 % 4 * 8, 0, 8, 8)),
                Rgba::WHITE,
            ),
        }
    }
}
