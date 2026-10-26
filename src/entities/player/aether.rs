//! The Aether state. Hits fill the Aether meter (weaknesses faster) and taking a hit drains whatever has not locked
//! in yet; progress locks after a few seconds unhurt, or at once on a finisher. When it is full Aether wakes by itself:
//! the equipped form fires once, and for a few seconds the hero is hard to hurt and every hit may pierce to the
//! target's weakness instead of landing in the current mode's element. Afterwards the meter starts again from empty.

use bevy::math::Vec2;

use super::Player;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::rotate;
use crate::entities::blocks::AetherSeal;
use crate::entities::combat_effects::{Beam, Shockwave, SkillBanner, SpinEffect};
use crate::entities::effects::{DamageNumber, FloatingText, Particle};
use crate::entities::entity::{Entity, EntityId};
use crate::gameplay::combat::{Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::gameplay::pacts;
use crate::gameplay::supports::Group;
use crate::scenes::world::World;
use crate::world::map::TILE;

/// Share of the meter Afterglow leaves locked in when Aether fades.
const AFTERGLOW: f32 = 25.0;
/// Share of the meter a hit taken while building drains, never below the locked progress.
const HURT_DRAIN: f32 = 15.0;
/// Damage taken while Aether is awake.
pub const AWAKE_DAMAGE: f32 = 0.7;
/// How far the opening forms reach.
const REACH: f32 = 56.0;
/// Seconds between Sanctum's cleansing pulses while Aether is awake.
const SANCTUM_PULSE: f32 = 2.0;

impl Player {
    pub fn aether_active(&self) -> bool {
        self.aether_time > 0.0
    }

    /// Sanctum's level while its aura holds: Aether awake with Sanctum as the form.
    pub(super) fn sanctum_level(&self, world: &World) -> Option<i32> {
        let sanctum = world.state.equipped_skill(Element::Aether).is_some_and(|s| s.id == "sanctum");
        (self.aether_active() && sanctum).then(|| world.state.aether_level())
    }

    /// The share of damage Sanctum's aura lets through.
    pub(super) fn sanctum_shield(&self, world: &World) -> f32 {
        match self.sanctum_level(world) {
            Some(1) => 0.7,
            Some(2) => 0.6,
            Some(_) => 0.5,
            None => 1.0,
        }
    }

    /// The meter as a fraction: filling while building, draining while awake.
    pub fn aether_meter(&self) -> f32 {
        self.aether / 100.0
    }

    /// The share of the meter locked in, which hits can no longer drain.
    pub fn aether_locked(&self) -> f32 {
        self.aether_lock / 100.0
    }

    /// Fills the meter for a hit that landed on a foe.
    pub(super) fn feed_aether(&mut self, world: &mut World, hit: &Hit) {
        let crescendo = world.state.mods().has("crescendo");
        let amount = match (hit.kind, crescendo) {
            (HitKind::Finisher, true) => 6.0,
            (HitKind::ChargedBolt, true) => 4.0,
            (_, true) => 0.0,
            (HitKind::Melee, _) => 1.2,
            (HitKind::Finisher, _) => 3.0,
            (HitKind::Bolt, _) => 0.6,
            (HitKind::ChargedBolt, _) => 2.0,
            (HitKind::Skill, _) => 1.5,
            _ => 0.5,
        } * if hit.result.element_mod > 1.0 { 1.75 } else { 1.0 }
            * Group::of(hit.kind).map_or(1.0, |g| Self::supports(world, g).aether);
        self.add_aether(world, amount);
        let anchored = hit.kind == HitKind::ChargedBolt && world.state.mods().has("anchor");
        if (hit.kind == HitKind::Finisher || anchored) && !self.aether_active() {
            self.lock_aether();
        }
    }

    fn lock_aether(&mut self) {
        self.aether_lock = self.aether;
    }

    pub(super) fn add_aether(&mut self, world: &mut World, amount: f32) {
        if self.aether_active() || world.state.dormant() {
            return;
        }
        let state = &world.state;
        let rate = state.acc().aether
            * state.mods().aether_gain
            * if state.has("prism_band") { 1.2 } else { 1.0 }
            * if state.prism("everlight") { 0.8 } else { 1.0 }
            * if pacts::active(state, "volatility") { 0.5 } else { 1.0 }
            * if state.mods().has("unbroken") { 0.75 } else { 1.0 };
        self.aether = (self.aether + amount * rate * 100.0 / state.aether_capacity()).min(100.0);
        if state.mods().has("unbroken") {
            self.lock_aether();
        }
        if self.aether >= 100.0 {
            self.wake_aether(world);
        }
    }

    /// A hit taken while building drains progress that has not locked in yet. Aether itself is untouched by hits.
    pub(super) fn hurt_aether(&mut self, world: &World) {
        self.aether_calm = 0.0;
        if self.aether_active() {
            return;
        }
        let scale = if pacts::active(&world.state, "volatility") { 2.0 } else { 1.0 };
        self.aether = (self.aether - HURT_DRAIN * scale).max(self.aether_lock);
    }

    pub(super) fn update_aether(&mut self, world: &mut World, dt: f32) {
        let fed = std::mem::take(&mut world.aether_feed);
        if fed > 0.0 {
            self.add_aether(world, fed);
        }
        if self.aether_active() {
            if self.sanctum_level(world).is_some() {
                self.sanctum_pulse -= dt;
                if self.sanctum_pulse <= 0.0 {
                    self.sanctum_pulse = SANCTUM_PULSE;
                    self.sanctum_cleanse(world, REACH * 0.7, 0.5);
                }
            }
            self.aether_time -= dt;
            self.aether = (100.0 * self.aether_time / world.state.aether_duration()).clamp(0.0, 100.0);
            if self.aether_time <= 0.0 {
                self.aether_time = 0.0;
                self.aether = if world.state.mods().has("afterglow") { AFTERGLOW } else { 0.0 };
                self.aether_lock = self.aether;
                audio::play(Sound::ChargeLevel, 0.5, -0.4);
                world.add(FloatingText::new(
                    self.center() - Vec2::new(0.0, 14.0),
                    "Aether fades",
                    Element::Aether.glow(),
                ));
            }
        } else {
            self.aether_calm += dt;
            if self.aether_calm >= world.state.aether_lock_delay() && self.aether_lock < self.aether {
                self.lock_aether();
            }
        }
    }

    fn wake_aether(&mut self, world: &mut World) {
        self.aether = 100.0;
        self.aether_lock = 0.0;
        self.aether_time = world.state.aether_duration();
        world.flash(Element::Aether.glow(), 0.25);
        audio::play(Sound::ChargeLevel, 1.0, 0.6);
        Particle::burst(world, self.center(), Element::Aether.glow(), 24, 70.0, 0.5, true);
        world.add(FloatingText::new(self.center() - Vec2::new(0.0, 16.0), "AETHER", Element::Aether.glow()));
        let level = world.state.aether_level();
        let Some(form) = world.state.equipped_skill(Element::Aether) else { return };
        let reach = REACH;
        match form.id {
            "prism_ray" => self.prism_ray(world, level),
            "sanctum" => self.sanctum(world, level, reach),
            _ => self.prism_edge(world, level, reach),
        }
        world.add(SpinEffect::new(self.center(), Element::Aether, 0.3));
        world.add(SkillBanner::new(self.base.pos, form, level));
    }

    fn form_hit(&self, world: &World, power: f32, poise: f32, knock: f32) -> Hit {
        self.make_hit(world, power, HitKind::Skill, None, poise, 1.5, knock)
    }

    /// Prism Edge: blades fly at the nearest foes in turn.
    fn prism_edge(&mut self, world: &mut World, level: i32, reach: f32) {
        let blades = match level {
            1 => 3,
            2 => 4,
            _ => 6,
        };
        let center = self.center();
        let color = Element::Aether.glow();
        let mut targets: Vec<(EntityId, Vec2)> = world
            .damageables()
            .into_iter()
            .filter(|(_, f, b)| *f == Faction::Enemy && b.center().distance(center) <= reach * 1.5 + b.w / 2.0)
            .map(|(id, _, b)| (id, b.center()))
            .collect();
        if world.state.aether_awakened() {
            targets.extend(
                world
                    .entities_of::<AetherSeal>()
                    .map(|seal| (seal.base().id, seal.hurtbox().center()))
                    .filter(|(_, at)| at.distance(center) <= reach * 1.5),
            );
        }
        targets.sort_by(|a, b| a.1.distance(center).total_cmp(&b.1.distance(center)));
        targets.truncate(blades);
        for i in 0..if targets.is_empty() { 0 } else { blades } {
            let (id, at) = targets[i % targets.len()];
            world.add(Shockwave::new(at, 12.0 + 2.0 * (i / targets.len()) as f32, color));
            let dir = (at - center).normalize_or_zero();
            world.add(Particle::new(center + dir * 6.0, dir * 220.0, color, 0.2).spark());
            let hit = self.form_hit(world, 1.0, 2.0, 60.0);
            self.strike_target(world, id, hit);
        }
        if level >= 3 {
            self.strike_area(world, center, reach * 0.8, None, |p, w| p.form_hit(w, 1.5, 3.0, 180.0));
            world.add(Shockwave::new(self.base.pos, reach * 0.8, color));
            world.camera.shake(0.2, 2.0);
        }
    }

    /// Prism Ray: a piercing beam along the aim (three at full strength).
    fn prism_ray(&mut self, world: &mut World, level: i32) {
        let (width, power, angles): (f32, f32, &[f32]) = match level {
            1 => (8.0, 2.0, &[0.0]),
            2 => (12.0, 2.6, &[0.0]),
            _ => (10.0, 3.0, &[-0.3, 0.0, 0.3]),
        };
        let mut struck = Vec::new();
        for &angle in angles {
            self.prism_beam(world, rotate(self.aim, angle), width, power, &mut struck);
        }
        world.camera.shake(0.2, 2.0);
    }

    fn prism_beam(&mut self, world: &mut World, dir: Vec2, width: f32, power: f32, struck: &mut Vec<EntityId>) {
        let start = self.center();
        let t = TILE as f32;
        let mut end = start;
        let mut i = 0;
        while i < 220 {
            let probe = start + dir * i as f32;
            if world.map.blocks_flying((probe.x / t).floor() as i32, ((probe.y + 6.0) / t).floor() as i32) {
                break;
            }
            end = probe;
            i += 4;
        }
        for (id, faction, b) in world.damageables() {
            if faction == Faction::Player || struck.contains(&id) {
                continue;
            }
            let c = b.center();
            let along = (c - start).dot(dir);
            if along < 0.0
                || along > start.distance(end) + 6.0
                || (start + dir * along).distance(c) >= width + b.w / 2.0
            {
                continue;
            }
            struck.push(id);
            let hit = self.form_hit(world, power, 4.0, 160.0);
            self.strike_target(world, id, hit);
        }
        world.add(Beam::new(start, end, Element::Aether.glow()));
    }

    /// Sanctum's wake: a heal and a cleansing pulse, then an aura for as long as Aether lasts (see `sanctum_shield`)
    /// that pulses again every couple of seconds.
    fn sanctum(&mut self, world: &mut World, level: i32, reach: f32) {
        let share = match level {
            1 => 0.06,
            2 => 0.10,
            _ => 0.15,
        };
        let amount = (world.state.max_hp() as f32 * share) as i32;
        world.heal_player(amount);
        world.add(DamageNumber::heal(self.center(), amount));
        audio::sfx(Sound::Heal);
        self.sanctum_pulse = SANCTUM_PULSE;
        self.sanctum_cleanse(world, reach * 0.7, 0.8);
    }

    /// Cures ailments and blight and strikes the foes around the hero.
    fn sanctum_cleanse(&mut self, world: &mut World, radius: f32, power: f32) {
        let center = self.center();
        self.strike_area(world, center, radius, None, |p, w| p.form_hit(w, power, 2.0, 160.0));
        self.cure_ailments();
        world.state.corruption = 0.0;
        world.add(Shockwave::new(self.base.pos, radius, Element::Aether.glow()));
        Particle::burst(world, center, Rgba::rgb(255, 220, 255), 12, 50.0, 0.5, true);
    }
}
