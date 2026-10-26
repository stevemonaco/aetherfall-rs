//! Trial Rifts: golden tears in the world that lead to a hard arena of rifts guarding support gems. Inside, a gate of
//! the same gold leads back out.

use bevy::math::Vec2;

use super::effects::Particle;
use super::entity::{Entity, EntityBase};
use crate::art::combat as art_combat;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::Batch;
use crate::entity_base;
use crate::scenes::world::World;
use crate::world::map::{EntityData, tile_feet};

const GOLD: Rgba = Rgba::rgb(255, 214, 110);

pub struct TrialGate {
    base: EntityBase,
    name: String,
    to: String,
    tile: (i32, i32),
    /// Inside the trial: leads back without asking.
    exit: bool,
    /// The condition that marks the trial as finished (every rift inside sealed).
    done: Option<String>,
    time: f32,
}

impl TrialGate {
    pub fn new(data: &EntityData) -> TrialGate {
        TrialGate {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 14.0,
                height: 6.0,
                ..EntityBase::default()
            },
            name: data.str_or("name", "Trial").to_string(),
            to: data.str_or("to", "fields").to_string(),
            tile: (data.int("tx", 0), data.int("ty", 0)),
            exit: data.bool("exit", false),
            done: data.str("done").map(str::to_string),
            time: 0.0,
        }
    }

    /// Cleared once: the first run's gems are taken and the last one waits for a Heat run.
    fn cleared(&self, world: &World) -> bool {
        self.done.as_deref().is_some_and(|c| world.state.check(Some(c)))
    }

    /// Cleared at Heat as well, so nothing is left inside.
    fn finished(&self, world: &World) -> bool {
        self.cleared(world)
            && self.done.as_deref().is_some_and(|c| world.state.check(Some(&c.replacen("golden:", "golden_heat:", 1))))
    }
}

impl Entity for TrialGate {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn light(&self, world: &World) -> Option<(f32, Rgba)> {
        Some((if self.finished(world) { 24.0 } else { 56.0 }, GOLD))
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if !self.finished(world) && world.rng.next_f32() < 0.25 {
            let offset = Vec2::new(world.rng.range(-8, 9) as f32, -6.0 - world.rng.below(16) as f32);
            world.add(Particle::new(self.base.pos + offset, Vec2::new(0.0, -16.0), GOLD, 0.7).spark());
        }
    }

    fn interact(&mut self, world: &mut World) {
        let warp =
            format!("warp:{},{},{},{}", self.to, self.tile.0, self.tile.1, if self.exit { "down" } else { "up" });
        if self.exit {
            audio::play(Sound::ChargeLevel, 0.7, -0.3);
            world.run_actions(&[warp]);
        } else if self.finished(world) {
            world.show_message(&format!("The {} lies quiet. Its gems are already yours.", self.name));
        } else if self.cleared(world) {
            world.confirm(
                format!(
                    "The {} smoulders. Its last gem waits for whoever rekindles it at Heat. Step through?",
                    self.name
                ),
                vec![warp],
            );
        } else {
            world.confirm(
                format!("A golden rift hums here: the {}. Its foes are fierce. Step through?", self.name),
                vec![warp],
            );
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let frame = (self.time * 8.0) as i32 % 4;
        let src = Some(IRect::new(frame * 32, 0, 32, 40));
        let tint = if self.finished(world) { Rgba::rgb(110, 110, 110) } else { Rgba::WHITE };
        batch.draw_src(art_combat::trial_portal(), snap(self.base.pos - Vec2::new(16.0, 38.0)), src, tint);
    }
}
