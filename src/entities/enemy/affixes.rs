//! Elite affixes: the modifiers that make an elite more than a bigger health bar. Elites roll one affix below level 20,
//! two below level 50 and three after that.

use bevy::math::Vec2;

use super::{Enemy, Phase, SHIELD_HP, strongest_resist};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::Box2;
use crate::core::gfx::Batch;
use crate::entities::combat_effects::{GroundMarker, Shockwave};
use crate::entities::effects::{FloatingText, Particle, Puff};
use crate::entities::entity::{Entity, EntityBase};
use crate::entities::hazards::GasCloud;
use crate::entity_base;
use crate::gameplay::combat::{CombatStats, Faction, Hit, HitKind};
use crate::gameplay::element::{BASIC, Element};
use crate::gameplay::trial_mods::TrialMod;
use crate::scenes::world::World;
use crate::world::map::TILE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Affix {
    /// Moves and recovers its attacks faster.
    Swift,
    /// Blinks to the hero's back now and then.
    Blinking,
    /// Enrages below half life.
    Frenzied,
    /// Heals when its attacks land.
    Vampiric,
    /// Carries a shield that regrows after it breaks.
    Shielded,
    /// Bursts a moment after it dies.
    Volatile,
    /// Leaves poison clouds in its wake.
    Noxious,
    /// Plain bolts glance off it.
    Thorned,
    /// Calls lesser kin to its side.
    Rallying,
    /// An elemental aura that blunts damage until its opposite strips it.
    AuraBound,
    /// More life, and hits don't make it flinch.
    Hulking,
}

const ALL: [Affix; 11] = [
    Affix::Swift,
    Affix::Blinking,
    Affix::Frenzied,
    Affix::Vampiric,
    Affix::Shielded,
    Affix::Volatile,
    Affix::Noxious,
    Affix::Thorned,
    Affix::Rallying,
    Affix::AuraBound,
    Affix::Hulking,
];

/// Seconds between a timed affix's actions.
const BLINK_EVERY: f32 = 5.0;
const CLOUD_EVERY: f32 = 2.5;
const RALLY_EVERY: f32 = 7.0;
const SHIELD_REGROW: f32 = 8.0;
const MAX_RALLIED: i32 = 4;

impl Affix {
    pub fn label(self) -> &'static str {
        match self {
            Affix::Swift => "Swift",
            Affix::Blinking => "Blinking",
            Affix::Frenzied => "Frenzied",
            Affix::Vampiric => "Vampiric",
            Affix::Shielded => "Shielded",
            Affix::Volatile => "Volatile",
            Affix::Noxious => "Noxious",
            Affix::Thorned => "Thorned",
            Affix::Rallying => "Rallying",
            Affix::AuraBound => "Aura-bound",
            Affix::Hulking => "Hulking",
        }
    }

    fn fits(self, enemy: &Enemy) -> bool {
        let still = enemy.def.movement == "static";
        match self {
            Affix::Shielded => !enemy.shield,
            Affix::AuraBound => !enemy.aura,
            Affix::Blinking | Affix::Swift => !still,
            _ => true,
        }
    }
}

/// How many affixes an elite of this level carries.
pub fn count_for(level: i32) -> usize {
    match level {
        ..20 => 1,
        20..50 => 2,
        _ => 3,
    }
}

impl Enemy {
    pub fn has_affix(&self, affix: Affix) -> bool {
        self.affixes.contains(&affix)
    }

    /// Rolls this elite's affixes and applies the ones that change it from the start.
    pub(super) fn roll_affixes(&mut self, world: &World, extra: usize) {
        let mut pool: Vec<Affix> = ALL.into_iter().filter(|a| a.fits(self)).collect();
        world.rng.shuffle(&mut pool);
        pool.truncate(count_for(self.level) + extra);
        self.affixes = pool;
        if self.has_affix(Affix::Hulking) {
            self.max_hp = self.max_hp * 7 / 5;
            self.hp = self.max_hp;
        }
        if self.has_affix(Affix::Shielded) {
            self.shield = true;
            self.shield_hp = SHIELD_HP;
        }
        if self.has_affix(Affix::AuraBound) {
            self.aura = true;
            let weakest = strongest_resist(self.def);
            self.aura_element =
                if self.def.resist_to(weakest) > 1.0 { weakest.opposite() } else { BASIC[world.rng.below(4) as usize] };
        }
        self.affix_timer = 1.5;
    }

    /// The elite's title, e.g. "Swift Vampiric".
    pub(super) fn affix_title(&self) -> String {
        self.affixes.iter().map(|a| a.label()).collect::<Vec<_>>().join(" ")
    }

    pub(super) fn update_affixes(&mut self, world: &mut World, dt: f32) {
        if self.affixes.is_empty() {
            return;
        }
        if self.has_affix(Affix::Shielded) && !self.shield {
            self.shield_regrow += dt;
            if self.shield_regrow >= SHIELD_REGROW {
                self.shield_regrow = 0.0;
                self.shield = true;
                self.shield_hp = SHIELD_HP;
                audio::play(Sound::Clink, 0.7, -0.3);
                Particle::burst(world, self.hurtbox().center(), Rgba::WHITE, 8, 40.0, 0.3, true);
            }
        }
        if self.has_affix(Affix::Frenzied) && !self.enraged && self.hp * 2 < self.max_hp {
            self.enraged = true;
            let at = self.base.pos - Vec2::new(0.0, self.base.height + 14.0);
            world.add(FloatingText::new(at, "FRENZY", Rgba::rgb(255, 80, 60)));
        }
        if !self.alerted || !matches!(self.phase, Phase::Chase | Phase::Idle) {
            return;
        }
        self.affix_timer -= dt;
        if self.affix_timer > 0.0 {
            return;
        }
        let mut next = f32::MAX;
        if self.has_affix(Affix::Blinking) {
            if world.hero.pos.distance(self.base.pos) > 48.0 {
                self.blink(world);
            }
            next = next.min(BLINK_EVERY);
        }
        if self.has_affix(Affix::Noxious) {
            world.add(GasCloud::new(self.base.pos, self.stats(), Vec2::ZERO));
            next = next.min(CLOUD_EVERY);
        }
        if self.has_affix(Affix::Rallying) {
            if self.rallied < MAX_RALLIED {
                self.rally(world);
            }
            next = next.min(RALLY_EVERY);
        }
        self.affix_timer = if next == f32::MAX { 1.0 } else { next };
    }

    pub(super) fn blink(&mut self, world: &mut World) {
        let behind = world.hero.pos - world.hero.facing.to_vec() * 22.0;
        let t = TILE as f32;
        let (tx, ty) = ((behind.x / t).floor() as i32, ((behind.y - 3.0) / t).floor() as i32);
        let flying = self.is_flying();
        let blocked = if flying { world.map.blocks_flying(tx, ty) } else { world.map.blocks_walking(tx, ty) };
        if blocked {
            return;
        }
        world.add(Puff::new(self.base.pos - Vec2::new(0.0, 6.0), Rgba::rgb(200, 140, 255)));
        self.base.pos = behind;
        world.add(Puff::new(behind - Vec2::new(0.0, 6.0), Rgba::rgb(200, 140, 255)));
        audio::play(Sound::Door, 0.5, 0.8);
        self.set_phase(Phase::Recover);
        self.attack = None;
    }

    fn rally(&mut self, world: &mut World) {
        let side = if self.rallied % 2 == 0 { 1.0 } else { -1.0 };
        let at = self.base.pos + Vec2::new(side * 14.0, 4.0);
        let t = TILE as f32;
        if world.map.blocks_walking((at.x / t).floor() as i32, ((at.y - 3.0) / t).floor() as i32) {
            return;
        }
        let mut kin = Enemy::new(self.def, at, Some((self.level - 2).max(1)));
        kin.alerted = true;
        kin.phase = Phase::Chase;
        world.add(kin);
        world.add(Puff::new(at - Vec2::new(0.0, 6.0), Rgba::rgb(200, 140, 255)));
        self.rallied += 1;
    }

    /// Thorned elites shrug off plain bolts.
    pub(super) fn deflects(&self, hit: &Hit) -> bool {
        self.has_affix(Affix::Thorned) && hit.kind == HitKind::Bolt
    }

    pub(super) fn feed_on_hit(&mut self, world: &mut World) {
        let thirst = world.trial_mod(TrialMod::Bloodthirst);
        if !(self.has_affix(Affix::Vampiric) || thirst) || self.hp <= 0 {
            return;
        }
        let amount = (self.max_hp / if self.has_affix(Affix::Vampiric) { 12 } else { 16 }).max(1);
        self.hp = (self.hp + amount).min(self.max_hp);
        Particle::burst(world, self.hurtbox().center(), Rgba::rgb(220, 40, 60), 6, 30.0, 0.3, true);
    }

    pub(super) fn affix_death(&self, world: &mut World) {
        if self.has_affix(Affix::Volatile) {
            world.add(Detonation::new(self.base.pos, self.stats(), Element::Fire, 30.0, 0.9, 1.6));
        }
    }

    pub(super) fn draw_elite_glow(&self, batch: &mut Batch, feet: Vec2) {
        if self.aura {
            return;
        }
        let glow = Rgba::rgb(255, 200, 90).mul(0.3 + (self.anim_time * 4.0).sin() * 0.1);
        for offset in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)] {
            self.strip.draw(batch, feet + offset, self.anim_frame(), glow, self.facing_right, true, self.scale());
        }
    }
}

/// A marked circle that explodes a moment later: a Volatile death, or one blast of an eruption.
pub struct Detonation {
    base: EntityBase,
    stats: CombatStats,
    element: Element,
    radius: f32,
    fuse: f32,
    power: f32,
}

impl Detonation {
    pub fn new(at: Vec2, stats: CombatStats, element: Element, radius: f32, fuse: f32, power: f32) -> Detonation {
        Detonation { base: EntityBase::at(at), stats, element, radius, fuse, power }
    }

    fn color(&self) -> Rgba {
        if self.element == Element::None { Rgba::rgb(255, 120, 60) } else { self.element.glow() }
    }
}

impl Entity for Detonation {
    entity_base!();

    fn added(&mut self, world: &mut World) {
        world.add(GroundMarker::new(self.base.pos, self.radius, self.fuse, self.color()));
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.fuse -= dt;
        if self.fuse > 0.0 {
            return;
        }
        self.base.removed = true;
        let center = self.base.pos;
        let r = self.radius;
        let area = Box2::new(center.x - r, center.y - r * 0.7, r * 2.0, r * 1.4);
        if world.hero.hurtbox.intersects(&area) && world.hero.pos.distance(center) < r + 6.0 {
            let mut hit = Hit {
                power: self.power,
                element: self.element,
                from: center,
                knockback: 220.0,
                ..Hit::new(Faction::Enemy, self.stats)
            };
            world.hit_player(&mut hit);
        }
        let color = self.color();
        world.add(Shockwave::new(center, r, color));
        Particle::burst(world, center - Vec2::new(0.0, 6.0), color, 20, 90.0, 0.5, true);
        world.camera.shake(0.2, 2.0);
        audio::play(Sound::Break, 0.8, -0.2);
    }

    fn draw(&self, _world: &World, _batch: &mut Batch) {}
}
