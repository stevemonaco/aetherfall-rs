//! The formulas: levels, stats, experience and damage, plus the [`Hit`] passed between attacker and receiver.

use bevy::math::Vec2;

use super::element::Element;
use crate::core::rng::Rng;
use crate::entities::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Faction {
    Neutral,
    Player,
    Enemy,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HitKind {
    Melee,
    Finisher,
    Bolt,
    ChargedBolt,
    Art,
    Contact,
    Attack,
    Environment,
    Status,
    Tether,
}

/// An attacker's combat numbers, captured when the hit is created.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct CombatStats {
    pub level: i32,
    pub atk: f32,
    pub def: f32,
    pub foc: f32,
}

impl CombatStats {
    pub const fn new(level: i32, atk: f32, def: f32, foc: f32) -> CombatStats {
        CombatStats { level, atk, def, foc }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DamageResult {
    pub amount: i32,
    pub crit: bool,
    pub element_mod: f32,
    pub blocked: bool,
    pub perfect: bool,
}

impl DamageResult {
    pub const NONE: DamageResult =
        DamageResult { amount: 0, crit: false, element_mod: 1.0, blocked: false, perfect: false };

    pub fn new(amount: i32, crit: bool, element_mod: f32) -> DamageResult {
        DamageResult { amount, crit, element_mod, ..DamageResult::NONE }
    }

    pub fn blocked(perfect: bool) -> DamageResult {
        DamageResult { blocked: true, perfect, ..DamageResult::NONE }
    }
}

/// Everything a receiver needs to resolve one strike. The receiver fills in `result` and the reaction flags so the
/// attacker can respond after the call returns.
#[derive(Clone, Debug)]
pub struct Hit {
    pub source: Faction,
    pub attacker: CombatStats,
    pub from: Vec2,
    pub power: f32,
    pub element: Element,
    pub kind: HitKind,
    /// Damage to the target's break gauge.
    pub poise: f32,
    /// Multiplier on elemental status buildup.
    pub status_power: f32,
    pub knockback: f32,
    pub projectile: bool,
    pub unblockable: bool,
    pub can_crit: bool,
    pub owner: Option<EntityId>,
    pub result: DamageResult,
    /// Set by a perfect guard: the attacking enemy is staggered.
    pub parried: bool,
    /// Set when a guard or ward stops a projectile: the projectile should vanish.
    pub expire_owner: bool,
    /// Set when the player reflects a projectile: its new faction, stats and direction.
    pub reflect: Option<(Faction, CombatStats, Vec2)>,
}

impl Hit {
    pub fn new(source: Faction, attacker: CombatStats) -> Hit {
        Hit {
            source,
            attacker,
            from: Vec2::ZERO,
            power: 1.0,
            element: Element::None,
            kind: HitKind::Attack,
            poise: 1.0,
            status_power: 1.0,
            knockback: 120.0,
            projectile: false,
            unblockable: false,
            can_crit: true,
            owner: None,
            result: DamageResult::NONE,
            parried: false,
            expire_owner: false,
            reflect: None,
        }
    }

    pub fn is_player(&self) -> bool {
        self.source == Faction::Player
    }
}

pub const MAX_LEVEL: i32 = 99;
pub const CRIT_MULTIPLIER: f32 = 1.5;

pub fn exp_to_next(level: i32) -> i32 {
    if level >= MAX_LEVEL {
        return 0;
    }
    (((50.0 + 30.0 * (level as f32).powf(1.8)) / 10.0).round() * 10.0) as i32
}

pub fn player_base_hp(level: i32) -> f32 {
    110.0 + 14.0 * level as f32
}

pub fn player_base_atk(level: i32) -> f32 {
    10.0 + 2.4 * level as f32
}

pub fn player_base_def(level: i32) -> f32 {
    8.0 + 2.0 * level as f32
}

pub fn player_base_foc(level: i32) -> f32 {
    8.0 + 1.6 * level as f32
}

pub fn enemy_hp(level: i32) -> f32 {
    24.0 + 13.0 * level as f32
}

pub fn enemy_atk(level: i32) -> f32 {
    12.0 + 3.2 * level as f32
}

pub fn enemy_def(level: i32) -> f32 {
    6.0 + 2.0 * level as f32
}

pub fn enemy_foc(level: i32) -> f32 {
    5.0 + 1.5 * level as f32
}

pub fn enemy_exp(level: i32) -> f32 {
    8.0 * (level as f32).powf(1.5)
}

/// Over-levelled players earn less and under-levelled ones a little more, so farming weak areas fades quickly.
pub fn exp_gap_factor(enemy_level: i32, player_level: i32) -> f32 {
    (1.0 + 0.1 * (enemy_level - player_level) as f32).clamp(0.1, 1.5)
}

pub fn crit_chance(foc: f32) -> f32 {
    foc / (foc + 200.0)
}

/// Environmental hazards hit as if from a creature of the map's level.
pub fn environment(level: i32) -> CombatStats {
    CombatStats::new(level, enemy_atk(level), 0.0, enemy_foc(level))
}

pub fn resolve(hit: &Hit, defender: CombatStats, element_mod: f32, extra_mod: f32, rng: &Rng) -> DamageResult {
    let atk = hit.attacker.atk.max(1.0);
    let def = defender.def.max(0.0);
    let mut raw = atk * hit.power * atk / (atk + def);
    if hit.is_player() && defender.level > hit.attacker.level {
        raw *= (1.0 - 0.06 * (defender.level - hit.attacker.level) as f32).max(0.4);
    }
    let crit = hit.can_crit && rng.next_f32() < crit_chance(hit.attacker.foc);
    let amount =
        raw * element_mod * extra_mod * if crit { CRIT_MULTIPLIER } else { 1.0 } * (0.95 + rng.next_f32() * 0.1);
    DamageResult::new(if element_mod <= 0.0 { 0 } else { (amount.round() as i32).max(1) }, crit, element_mod)
}

#[derive(Clone, Copy, Debug)]
pub struct ModeMods {
    pub atk: f32,
    pub def: f32,
    pub speed: f32,
    pub foc: f32,
}

/// Stat multipliers granted by an element mode. Thunder sharpens focus: more crits and stronger ailments.
pub fn mode_mods(mode: Element) -> ModeMods {
    let (atk, def, speed, foc) = match mode {
        Element::Wind => (1.0, 1.0, 1.15, 1.0),
        Element::Fire => (1.15, 0.9, 1.0, 1.0),
        Element::Water => (1.0, 1.15, 0.95, 1.0),
        Element::Thunder => (1.0, 1.0, 1.0, 1.6),
        Element::Aether => (1.08, 1.08, 1.05, 1.2),
        Element::None => (1.0, 1.0, 1.0, 1.0),
    };
    ModeMods { atk, def, speed, foc }
}

/// How much a creature in `mode` takes from `incoming`.
pub fn mode_resist(mode: Element, incoming: Element) -> f32 {
    if mode == Element::None || incoming == Element::None {
        return 1.0;
    }
    if mode == Element::Aether {
        return 0.85;
    }
    if incoming == mode {
        return 0.5;
    }
    if incoming == mode.opposite() { 1.25 } else { 1.0 }
}
