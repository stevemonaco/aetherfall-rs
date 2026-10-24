//! A training dummy for the Hall of Echoes: it never falls, takes hits like a foe of the player's level (weak to one
//! element if set), and shows the damage it has taken over the last few seconds.

use std::collections::VecDeque;

use bevy::math::Vec2;

use super::effects::DamageNumber;
use super::entity::{Entity, EntityBase};
use crate::art::props as art;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::{Box2, snap};
use crate::core::gfx::Batch;
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{self, CombatStats, Faction, Hit};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::map::{EntityData, tile_feet};

/// Damage is averaged over this many seconds.
const WINDOW: f32 = 5.0;

pub struct TrainingDummy {
    base: EntityBase,
    weak: Element,
    hits: VecDeque<(f32, i32)>,
    time: f32,
    shake: f32,
}

impl TrainingDummy {
    pub fn new(data: &EntityData) -> TrainingDummy {
        TrainingDummy {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 10.0,
                height: 6.0,
                ..EntityBase::default()
            },
            weak: Element::parse(data.str("weak")),
            hits: VecDeque::new(),
            time: 0.0,
            shake: 0.0,
        }
    }

    fn element_mod(&self, element: Element) -> f32 {
        match element {
            Element::None | Element::Aether => 1.0,
            e if e == self.weak => 1.5,
            e if self.weak != Element::None && e == self.weak.opposite() => 0.75,
            _ => 1.0,
        }
    }

    fn dps(&self) -> f32 {
        let span = self.time.clamp(1.0, WINDOW);
        self.hits.iter().map(|h| h.1).sum::<i32>() as f32 / span
    }

    /// An Aether hit may pierce to the dummy's weakness, like a real foe.
    fn resolve_element(&self, world: &World, hit: &Hit) -> Element {
        let element = hit.element;
        let aether = hit.aether || element == Element::Aether;
        if aether
            && self.weak != Element::None
            && (element == Element::Aether || world.rng.next_f32() < world.state.pierce_chance(self.weak))
        {
            return self.weak;
        }
        if element == Element::Aether { Element::None } else { element }
    }
}

impl Entity for TrainingDummy {
    entity_base!();

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Enemy)
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 6.0, self.base.pos.y - 20.0, 12.0, 20.0)
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.time += dt;
        self.shake = (self.shake - dt).max(0.0);
        while self.hits.front().is_some_and(|h| self.time - h.0 > WINDOW) {
            self.hits.pop_front();
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if !hit.is_player() {
            return false;
        }
        let element = self.resolve_element(world, hit);
        let level = world.state.level;
        let stats =
            CombatStats::new(level, combat::enemy_atk(level), combat::enemy_def(level), combat::enemy_foc(level));
        let element_mod = self.element_mod(element);
        let mut probe = hit.clone();
        probe.element = element;
        let result = combat::resolve(&probe, stats, element_mod, 1.0, &world.rng);
        hit.result = result;
        self.hits.push_back((self.time, result.amount));
        self.shake = 0.15;
        let suffix = if element_mod > 1.0 { "!" } else { "" };
        let at = self.base.pos - Vec2::new(0.0, 26.0);
        world.add(DamageNumber::styled(at, result.amount, element, result.crit, false, false, suffix));
        audio::play(Sound::Hit, 0.7, 0.1);
        true
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let wobble = (self.shake * 60.0).sin() * self.shake * 10.0;
        let tint = if self.shake > 0.1 { Rgba::rgb(255, 200, 200) } else { Rgba::WHITE };
        batch.draw(art::training_dummy(), snap(self.base.pos + Vec2::new(wobble - 9.0, -26.0)), tint);
        if self.weak != Element::None {
            batch.fill_rect(self.base.pos.x - 3.0, self.base.pos.y - 12.0, 7.0, 2.0, self.weak.glow());
        }
        let label = self.weak.label();
        let color = if self.weak == Element::None { palette::TEXT_DIM } else { self.weak.light() };
        font::draw_centered(batch, label, snap(self.base.pos + Vec2::new(0.0, 4.0)), color, Some(palette::TEXT_SHADOW));
        if !self.hits.is_empty() {
            let dps = format!("{} DPS", self.dps().round());
            font::draw_centered(
                batch,
                &dps,
                snap(self.base.pos - Vec2::new(0.0, 36.0)),
                palette::HIGHLIGHT,
                Some(palette::TEXT_SHADOW),
            );
        }
    }
}
