//! Circuit triggers: passives that fire when something happens in combat, each with its own cooldown.

use bevy::math::Vec2;

use super::Player;
use crate::core::audio::{self, Sound};
use crate::entities::combat_effects::Shockwave;
use crate::entities::effects::{DamageNumber, FloatingText};
use crate::entities::hazards::SteamCloud;
use crate::gameplay::circuit::{Trigger, TriggerDef};
use crate::gameplay::combat::{Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;

impl Player {
    /// Runs every unlocked trigger listening for `event` whose mode matches and whose cooldown has run out.
    pub(super) fn trigger(&mut self, world: &mut World, event: Trigger, hit: Option<&Hit>) {
        let mode = world.state.mode;
        let ready: Vec<TriggerDef> = world
            .state
            .mods()
            .triggers
            .iter()
            .filter(|t| t.event == event && t.mode.is_none_or(|m| m == mode))
            .filter(|t| self.trigger_cooldowns.get(&t.effect).is_none_or(|&left| left <= 0.0))
            .cloned()
            .collect();
        for t in ready {
            if t.cooldown > 0.0 {
                self.trigger_cooldowns.insert(t.effect.clone(), t.cooldown);
            }
            self.run_trigger(world, &t.effect, hit);
        }
    }

    pub(super) fn tick_triggers(&mut self, world: &mut World, dt: f32) {
        for left in self.trigger_cooldowns.values_mut() {
            *left -= dt;
        }
        for event in std::mem::take(&mut world.trigger_queue) {
            self.trigger(world, event, None);
        }
    }

    fn run_trigger(&mut self, world: &mut World, effect: &str, hit: Option<&Hit>) {
        match effect {
            "riposte" => {
                if let Some(hit) = hit {
                    self.counter(world, hit);
                }
            }
            "siphon" => {
                let amount = (world.state.max_hp() / 20).max(1);
                world.heal_player(amount);
                world.add(DamageNumber::heal(self.center(), amount));
            }
            "retaliation" => {
                let center = self.center();
                self.strike_area(world, center, 40.0, None, |p, w| {
                    p.make_hit(w, 1.2, HitKind::Skill, Some(Element::None), 2.0, 1.0, 220.0)
                });
                world.add(Shockwave::new(self.pos(), 40.0, Element::None.glow()));
                audio::play(Sound::Break, 0.7, 0.2);
            }
            "spirit_tap" => world.add_sp(15.0),
            "tempest_ward" => self.begin_reflect(2.0),
            "blaze_step" => self.dodge_trail = true,
            "arc_step" => self.dodge_shock = true,
            "mist_veil" => {
                self.hide(1.5);
                for _ in 0..8 {
                    let offset = Vec2::new(world.rng.range(-14, 15) as f32, world.rng.range(-8, 9) as f32);
                    world.add(SteamCloud::new(self.pos() + offset, true));
                }
            }
            "flicker" => world.slow_enemies(2.0),
            "overflow" => {
                world.state.sp = (world.state.sp + 100.0).min(world.state.max_sp());
                let at = self.center() - Vec2::new(0.0, 22.0);
                world.add(FloatingText::new(at, "+1 SP", Element::Aether.light()));
            }
            _ => {}
        }
    }
}
