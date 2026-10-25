//! Guarding. A guard raised just before a hit lands parries it outright, and one raised a moment sooner still blocks it
//! cleanly; any other block lets some damage through and wears down the guard's strength, more as it weakens. Holding
//! the guard up drains it slowly, and an emptied guard breaks, leaving the hero stunned.

use bevy::math::Vec2;

use super::Player;
use crate::art::combat as art;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::snap;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entities::effects::{FloatingText, Particle};
use crate::gameplay::combat::{self, DamageResult, Hit};
use crate::gameplay::pacts;
use crate::gameplay::weave::Trigger;
use crate::scenes::world::World;

pub(super) const GUARD_MAX: f32 = 100.0;
/// Strength lost each second the guard is held up.
const HELD_DRAIN: f32 = 5.0;
/// Strength regained each second once the guard has rested.
const REGEN: f32 = 40.0;
/// Seconds after a block or lowering the guard before strength returns.
const REST: f32 = 0.8;
/// Seconds a broken guard cannot be raised.
const BROKEN_LOCK: f32 = 1.2;
/// Clean blocks extend this many perfect windows past the parry.
const CLEAN_WINDOWS: f32 = 2.0;

impl Player {
    pub(super) fn update_guard(&mut self, dt: f32) {
        let held =
            input::held(GameAction::Guard) && !self.airborne() && self.throw_held < 0.0 && self.guard_lock <= 0.0;
        if !held {
            self.guard_time = -1.0;
        } else if self.guard_time < 0.0 {
            self.guard_time = 0.0;
            audio::play(Sound::Clink, 0.25, -0.4);
        } else {
            self.guard_time += dt;
        }
    }

    /// Wears the guard down while it is up and restores it once lowered, whatever else the hero is doing.
    pub(super) fn tick_guard(&mut self, dt: f32) {
        self.guard_lock = (self.guard_lock - dt).max(0.0);
        if self.guarding() {
            self.guard_strength = (self.guard_strength - HELD_DRAIN * dt).max(1.0);
            self.guard_rest = 0.0;
            return;
        }
        self.guard_rest += dt;
        if self.guard_rest >= REST {
            self.guard_strength = (self.guard_strength + REGEN * dt).min(GUARD_MAX);
        }
    }

    /// Guarding cancels a swing or a charging throw, so the guard can always come up.
    pub(super) fn guard_cancels(&mut self) {
        if !input::pressed(GameAction::Guard) || self.airborne() || self.guard_lock > 0.0 {
            return;
        }
        if self.attack_time >= 0.0 {
            self.attack_time = -1.0;
            self.combo_step = 0;
            self.combo_buffered = false;
        }
        self.throw_held = -1.0;
    }

    pub(super) fn perfect_window(&self, world: &World) -> f32 {
        let mods = world.state.mods();
        let window = 0.15 + world.state.acc().perfect_window + mods.perfect_window;
        if mods.has("stillwater") { window * 2.0 } else { window }
    }

    pub(super) fn block(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        let window = self.perfect_window(world);
        let timed = !pacts::active(&world.state, "brittle") && self.guard_time >= 0.0;
        if timed && self.guard_time < window {
            self.parry(world, hit);
            return true;
        }
        let clean = timed && self.guard_time < window * CLEAN_WINDOWS;
        let heavy = hit.power >= 1.5;
        let cost = (12.0 + 14.0 * hit.power) * if hit.projectile { 0.6 } else { 1.0 } * if clean { 0.5 } else { 1.0 };
        self.guard_strength -= cost;
        self.guard_rest = 0.0;
        if hit.projectile {
            hit.expire_owner = true;
        }
        hit.result = DamageResult::blocked(false);
        if self.guard_strength <= 0.0 {
            self.break_guard(world, hit);
            return true;
        }

        let weakness = 1.0 - self.guard_strength / GUARD_MAX;
        let chip = if clean { 0.0 } else { 0.15 + 0.35 * weakness + if heavy { 0.15 } else { 0.0 } };
        if chip > 0.0 {
            self.chip(world, hit, chip);
        }
        audio::play(Sound::Clink, 0.8, if clean { 0.2 } else { -0.2 });
        let color = if clean { Rgba::WHITE } else { Rgba::rgb(220, 220, 255) };
        Particle::burst(world, self.center() + self.aim * 8.0, color, 6, 40.0, 0.2, true);
        self.knock(hit.from, if heavy { 160.0 } else { 90.0 }, 0.08);
        if heavy && !clean {
            self.guard_time = -1.0;
            self.stun_time = 0.35;
        }
        true
    }

    fn parry(&mut self, world: &mut World, hit: &mut Hit) {
        hit.result = DamageResult::blocked(true);
        self.guard_strength = (self.guard_strength + 20.0).min(GUARD_MAX);
        world.add_sp(50.0);
        world.hit_stop(0.09);
        world.rank.style(2.0);
        audio::play(Sound::Clink, 1.0, 0.6);
        Particle::burst(world, self.center() + self.aim * 8.0, Rgba::WHITE, 12, 70.0, 0.3, true);
        world.add(FloatingText::new(self.center() - Vec2::new(0.0, 16.0), "PERFECT", palette::HIGHLIGHT));
        if hit.projectile {
            hit.expire_owner = true;
        } else {
            hit.parried = true;
        }
        self.trigger(world, Trigger::PerfectGuard, Some(hit));
    }

    /// Damage that leaks through a block, as a share of the full hit.
    fn chip(&mut self, world: &mut World, hit: &Hit, share: f32) {
        let mode = Self::mode(world);
        let resist = combat::mode_resist(mode, hit.element);
        let result = combat::resolve(hit, world.state.stats(mode), resist, share, &world.rng);
        self.apply_damage(world, result.amount, hit.element, false);
    }

    fn break_guard(&mut self, world: &mut World, hit: &Hit) {
        self.guard_strength = 0.0;
        self.guard_time = -1.0;
        self.guard_lock = BROKEN_LOCK;
        self.stun_time = 0.6;
        self.chip(world, hit, 0.7);
        self.knock(hit.from, 200.0, 0.12);
        audio::play(Sound::Break, 0.9, -0.2);
        world.camera.shake(0.2, 2.0);
        Particle::burst(world, self.center() + self.aim * 8.0, Rgba::rgb(255, 120, 90), 14, 70.0, 0.4, true);
        world.add(FloatingText::new(self.center() - Vec2::new(0.0, 16.0), "GUARD BREAK", Rgba::rgb(255, 110, 90)));
    }

    pub(super) fn draw_shield(&self, world: &World, batch: &mut Batch, lift: Vec2) {
        let perfect = self.guard_time >= 0.0 && self.guard_time < self.perfect_window(world);
        let at = snap(self.center() + lift + self.aim * 7.0 + Vec2::new(0.0, 2.0));
        let worn = Rgba::WHITE.lerp(Rgba::rgb(255, 120, 100), 1.0 - self.guard_strength / GUARD_MAX);
        let color = if perfect { Rgba::WHITE.lerp(palette::HIGHLIGHT, 0.6) } else { worn };
        let origin = Vec2::new(5.0, 6.0);
        batch.draw_ex(art::guard_shield(), at, None, color, 0.0, origin, Vec2::ONE, false, false);
        if perfect {
            batch.draw_ex(
                art::guard_shield(),
                at,
                None,
                Rgba::WHITE.mul(0.5),
                0.0,
                origin,
                Vec2::splat(1.3),
                false,
                false,
            );
        }
    }

    /// A thin bar under the hero while the guard is up or recovering.
    pub(super) fn draw_guard_bar(&self, batch: &mut Batch, lift: Vec2) {
        if !self.guarding() && self.guard_strength >= GUARD_MAX {
            return;
        }
        let fill = self.guard_strength / GUARD_MAX;
        let at = snap(self.base.pos + lift + Vec2::new(-7.0, 7.0));
        let color = if self.guard_lock > 0.0 {
            Rgba::rgb(255, 110, 90)
        } else {
            Rgba::rgb(255, 120, 100).lerp(Rgba::rgb(150, 190, 255), fill)
        };
        batch.fill_rect(at.x - 1.0, at.y - 1.0, 16.0, 3.0, palette::OUTLINE);
        batch.fill_rect(at.x, at.y, 14.0 * fill, 1.0, color);
    }
}
