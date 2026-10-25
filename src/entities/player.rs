//! The player: movement, jumping and falling, taking damage and ailments, melee combos, bolts, dodges, guards, element
//! modes and skills (ported from Player.cs, Player.Combat.cs and Player.Draw.cs).

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::math::Vec2;

use super::combat_effects::{Afterimage, Shockwave, SkillBanner, SlashEffect};
use super::companion::Companion;
use super::effects::{DamageNumber, FloatingText, Particle};
use super::entity::{Entity, EntityBase, EntityId, PLAYER_ID};
use super::hazards::FirePatch;
use super::projectile::{Projectile, ProjectileHook};
use super::skill_actions::SkillAction;
use super::tether::TetherHook;
use crate::art::combat as art;
use crate::art::hero::{HeroPose, hero_sprite};
use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{Box2, safe_dir, snap};
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{self, CombatStats, DamageResult, Faction, Hit, HitKind};
use crate::gameplay::element::{Element, MODES};
use crate::gameplay::pacts;
use crate::gameplay::skills::{self, SkillDef};
use crate::gameplay::status::{Ailment, StatusSet};
use crate::gameplay::supports::{Group, SupportMods};
use crate::gameplay::weave::Trigger;
use crate::scenes::world::{HeroInfo, World};
use crate::world::map::{TILE, Terrain, tile_center};

mod aether;
mod triggers;

const BASE_SPEED: f32 = 92.0;
const HURT_INVULNERABLE: f32 = 0.6;
const JUMP_DURATION: f32 = 0.42;
const JUMP_HEIGHT: f32 = 12.0;
const FALL_DURATION: f32 = 0.7;
const SWING_DELAY: f32 = 0.12;
const FULL_CHARGE: f32 = 0.55;
const SWING_LENGTH: [f32; 3] = [0.24, 0.24, 0.34];
/// Real seconds a mode key must be held before the quick-change wheel opens.
const WHEEL_DELAY: f32 = 0.2;

pub struct Player {
    base: EntityBase,
    pub facing: Direction,
    status: StatusSet,
    walk_time: f32,
    invulnerable: f32,
    knock_time: f32,
    death_time: f32,
    jump_time: f32,
    fall_time: f32,
    knock_velocity: Vec2,
    last_safe: Vec2,
    move_dir: Vec2,
    moving: bool,
    revived: bool,
    regen: f32,
    frame_dt: f32,
    heat_tick: f32,
    blight_tick: f32,
    pub heat: f32,
    pub z: f32,
    pub aim: Vec2,
    /// The Aether meter, 0 to 100: filling from hits, then draining while Aether is awake.
    aether: f32,

    swing_hit: Vec<EntityId>,
    attack_time: f32,
    combo_reset: f32,
    throw_held: f32,
    bolt_cooldown: f32,
    stun_time: f32,
    hidden_time: f32,
    mode_flash: f32,
    /// Real seconds a mode key has been held, or negative when none is; holding opens the quick-change wheel.
    wheel_hold: f32,
    wheel_step: i32,
    wheel_open: bool,
    wheel_pick: Option<Element>,
    /// Seconds before the dormant blade complains again.
    dormant_note: f32,
    combo_step: usize,
    combo_buffered: bool,
    charged_sound_played: bool,

    dodging: bool,
    near_missed: bool,
    dodge_time: f32,
    dodge_invulnerable: f32,
    dodge_recharge: f32,
    dodge_lockout: f32,
    dodge_length: f32,
    dodge_speed: f32,
    dodge_charges: f32,
    dodge_dir: Vec2,
    /// Set by Weave triggers for the current dodge: leave fire behind, or shock what it passes.
    dodge_trail: bool,
    dodge_shock: bool,
    dodge_rammed: Vec<EntityId>,
    trigger_cooldowns: HashMap<String, f32>,

    guard_time: f32,
    aether_time: f32,
    aether_rest: f32,
    aether_lock: f32,
    aether_calm: f32,
    reflect_time: f32,
    mirror_time: f32,
    mirror_charges: i32,
    mirror_nova: bool,
    static_time: f32,
    static_tick: f32,
    static_radius: f32,
    static_power: f32,
    aegis_time: f32,
    ward_time: f32,
    /// Real seconds the skill button has been held, or negative when not charging.
    skill_held: f32,
    skill_level_reached: i32,
    last_skill: Option<(&'static SkillDef, i32)>,
    skill: Option<Box<SkillAction>>,
    /// Power of the skill being performed: 1, or less for an Echo gem's repeat.
    skill_scale: f32,
    /// An Echo gem's pending repeat: the skill, its level and the seconds until it fires.
    echo: Option<(&'static SkillDef, i32, f32)>,
    hook: Option<EntityId>,
    released_hook: Option<EntityId>,
    zipping: bool,
    zip_target: Vec2,
    zip_time: f32,

    interact: Option<EntityId>,
    touch_check: bool,
    time: f64,
}

impl Player {
    pub fn new(pos: Vec2, facing: Direction) -> Player {
        Player {
            base: EntityBase { id: PLAYER_ID, pos, width: 10.0, height: 8.0, ..EntityBase::default() },
            facing,
            status: StatusSet::default(),
            walk_time: 0.0,
            invulnerable: 0.0,
            knock_time: 0.0,
            death_time: 0.0,
            jump_time: -1.0,
            fall_time: -1.0,
            knock_velocity: Vec2::ZERO,
            last_safe: pos,
            move_dir: Vec2::Y,
            moving: false,
            revived: false,
            regen: 0.0,
            frame_dt: 0.0,
            heat_tick: 0.0,
            blight_tick: 0.0,
            heat: 0.0,
            z: 0.0,
            aim: Vec2::Y,
            aether: 0.0,
            swing_hit: Vec::new(),
            attack_time: -1.0,
            combo_reset: 0.0,
            throw_held: -1.0,
            bolt_cooldown: 0.0,
            stun_time: 0.0,
            hidden_time: 0.0,
            mode_flash: 0.0,
            wheel_hold: -1.0,
            wheel_step: 0,
            wheel_open: false,
            wheel_pick: None,
            dormant_note: 0.0,
            combo_step: 0,
            combo_buffered: false,
            charged_sound_played: false,
            dodging: false,
            near_missed: false,
            dodge_time: 0.0,
            dodge_invulnerable: 0.0,
            dodge_recharge: 0.0,
            dodge_lockout: 0.0,
            dodge_length: 0.0,
            dodge_speed: 0.0,
            dodge_charges: 0.0,
            dodge_dir: Vec2::ZERO,
            dodge_trail: false,
            dodge_shock: false,
            dodge_rammed: Vec::new(),
            trigger_cooldowns: HashMap::new(),
            guard_time: -1.0,
            aether_time: 0.0,
            aether_rest: 0.0,
            aether_lock: 0.0,
            aether_calm: 0.0,
            reflect_time: 0.0,
            mirror_time: 0.0,
            mirror_charges: 0,
            mirror_nova: false,
            static_time: 0.0,
            static_tick: 0.0,
            static_radius: 40.0,
            static_power: 0.35,
            aegis_time: 0.0,
            ward_time: 0.0,
            skill_held: -1.0,
            skill_level_reached: 0,
            last_skill: None,
            skill: None,
            skill_scale: 1.0,
            echo: None,
            hook: None,
            released_hook: None,
            zipping: false,
            zip_target: Vec2::ZERO,
            zip_time: 0.0,
            interact: None,
            touch_check: false,
            time: 0.0,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.death_time > 0.0
    }

    pub fn airborne(&self) -> bool {
        self.jump_time >= 0.0 || self.zipping
    }

    pub fn center(&self) -> Vec2 {
        self.base.pos - Vec2::new(0.0, 8.0 + self.z)
    }

    pub fn pos(&self) -> Vec2 {
        self.base.pos
    }

    /// Enemies lose track of the player while hidden (Mist Step).
    pub fn hidden(&self) -> bool {
        self.hidden_time > 0.0
    }

    pub fn status(&self) -> &StatusSet {
        &self.status
    }

    pub fn throw_charge(&self) -> f32 {
        if self.throw_held < SWING_DELAY { 0.0 } else { ((self.throw_held - SWING_DELAY) / FULL_CHARGE).min(1.0) }
    }

    pub fn aiming(&self) -> bool {
        self.throw_held >= SWING_DELAY
    }

    pub fn guarding(&self) -> bool {
        self.guard_time >= 0.0
    }

    /// The quick-change wheel's highlighted mode while it is open.
    pub fn wheel(&self) -> Option<Option<Element>> {
        self.wheel_open.then_some(self.wheel_pick)
    }

    pub fn mode_flash(&self) -> f32 {
        self.mode_flash
    }

    pub fn info(&self) -> HeroInfo {
        HeroInfo {
            pos: self.base.pos,
            center: self.center(),
            hitbox: self.base.hitbox(),
            hurtbox: self.hurtbox(),
            facing: self.facing,
            airborne: self.airborne(),
            dead: self.is_dead(),
            hidden: self.hidden(),
            aim: self.aim,
        }
    }

    /// The entity the player tried to talk to or open this frame, for the world to run once the player is back in place.
    pub fn take_interact(&mut self) -> Option<EntityId> {
        self.interact.take()
    }

    /// Whether the player finished a normal move this frame, so overlapped triggers and pickups should fire.
    pub fn take_touch_check(&mut self) -> bool {
        std::mem::take(&mut self.touch_check)
    }

    fn can_act(&self) -> bool {
        self.stun_time <= 0.0 && self.skill.is_none() && !self.dodging && self.hook.is_none() && !self.zipping
    }

    fn movement_allowed(&self) -> bool {
        self.can_act() && self.attack_time < 0.0
    }

    fn faces_aim(&self) -> bool {
        self.aiming() || self.guarding() || self.attack_time >= 0.0 || self.charging_skill()
    }

    pub fn charging_skill(&self) -> bool {
        self.skill_held >= 0.0
    }

    /// The level the skill being charged has reached, or 0 when not charging.
    pub fn skill_charge_level(&self) -> i32 {
        if self.charging_skill() { self.skill_level_reached } else { 0 }
    }

    fn mode(world: &World) -> Element {
        world.state.mode
    }

    /// The support gems socketed for a group in the current mode.
    fn supports(world: &World, group: Group) -> SupportMods {
        world.state.support_mods(world.state.mode, group)
    }

    fn stats(world: &World) -> CombatStats {
        world.state.stats(world.state.mode)
    }

    fn terrain_underfoot(&self, world: &World) -> Terrain {
        let t = TILE as f32;
        world.map.at((self.base.pos.x / t).floor() as i32, ((self.base.pos.y - 3.0) / t).floor() as i32)
    }

    pub fn tile_pos(&self) -> (i32, i32) {
        self.base.tile_pos()
    }

    fn move_by(&mut self, world: &World, delta: Vec2, flying: bool, corner_assist: bool) -> Vec2 {
        world.move_entity(&mut self.base, delta, flying, corner_assist, false)
    }

    fn move_speed(&self, world: &World) -> f32 {
        let mut speed = BASE_SPEED
            * world.state.mods().speed
            * combat::mode_mods(Self::mode(world)).speed
            * world.state.acc().speed
            * self.status.speed_factor();
        let under = self.terrain_underfoot(world);
        if !self.airborne() && under == Terrain::Bog {
            speed *= 0.55;
        }
        if !self.airborne() && under == Terrain::Shallows {
            speed *= 0.8;
        }
        if self.aiming() || self.charging_skill() {
            speed *= 0.55;
        }
        if self.guarding() {
            speed *= 0.4;
        }
        if self.attack_time >= 0.0 {
            speed *= 0.25;
        }
        speed
    }

    // ---- Movement and survival ----------------------------------------------------------------------------------

    fn update_player(&mut self, world: &mut World, dt: f32) {
        self.time = world.time;
        if self.is_dead() {
            self.death_time += dt;
            self.facing = Direction::from_index((self.death_time * 10.0) as i32);
            if self.death_time > 1.6 {
                world.on_player_defeated();
            }
            return;
        }

        self.frame_dt = dt;
        self.tick_triggers(world, dt);
        self.invulnerable = (self.invulnerable - dt).max(0.0);
        self.update_status(world, dt);
        self.update_combat_timers(world, dt);

        if self.fall_time >= 0.0 {
            self.update_fall(world, dt);
            return;
        }

        if self.knock_time > 0.0 {
            self.knock_time -= dt;
            let delta = self.knock_velocity * dt;
            self.move_by(world, delta, false, false);
            return;
        }

        if world.input_locked() {
            self.moving = false;
            self.cancel_actions();
            return;
        }

        self.update_aim(world);
        if self.update_actions(world, dt) {
            self.after_move(world);
            return;
        }

        if input::pressed(GameAction::Jump) && world.state.can_jump() && !self.airborne() && self.can_act() {
            self.jump_time = 0.0;
            audio::play(Sound::MenuMove, 0.7, -0.4);
        }

        let mut movement = input::movement();
        self.moving = movement != Vec2::ZERO && self.movement_allowed();
        if self.moving {
            if movement.length_squared() > 1.0 {
                movement = movement.normalize();
            }
            self.move_dir = movement.normalize();
            if !self.faces_aim() {
                self.facing = Direction::from_vec(movement, self.facing);
            }
            let speed = self.move_speed(world);
            let airborne = self.airborne();
            self.move_by(world, movement * speed * dt, airborne, true);
            self.walk_time += dt;
        }
        if self.faces_aim() {
            self.facing = Direction::from_vec(self.aim, self.facing);
        }

        self.update_jump(world, dt);
        self.after_move(world);
    }

    fn after_move(&mut self, world: &mut World) {
        let under = self.terrain_underfoot(world);
        if !self.airborne() && !self.dodging && under.is_hazard() {
            self.start_fall(world);
            return;
        }
        if !self.airborne() && !self.dodging {
            self.last_safe = self.base.pos;
        }
        self.update_heat(world, self.frame_dt);
        self.update_blight(world, self.frame_dt);
        let rate = world.state.acc().regen + world.state.mods().regen;
        if rate > 0.0 && world.state.hp < world.state.max_hp() && self.status.burn <= 0.0 {
            self.regen += world.state.max_hp() as f32 * rate * self.frame_dt;
        }
        if self.regen >= 1.0 {
            let whole = self.regen as i32;
            world.state.hp = (world.state.hp + whole).min(world.state.max_hp());
            self.regen -= whole as f32;
        }
        self.touch_check = true;
    }

    fn update_jump(&mut self, world: &mut World, dt: f32) {
        if self.jump_time < 0.0 {
            return;
        }
        self.jump_time += dt;
        let t = self.jump_time / (JUMP_DURATION * world.state.acc().jump_time);
        self.z = (t.min(1.0) * PI).sin() * JUMP_HEIGHT;
        if t < 1.0 {
            return;
        }
        self.jump_time = -1.0;
        self.z = 0.0;
        if self.terrain_underfoot(world).is_hazard() {
            self.start_fall(world);
        } else {
            Particle::burst(world, self.base.pos, Rgba::rgb(220, 210, 190), 4, 20.0, 0.25, false);
        }
    }

    fn update_heat(&mut self, world: &mut World, dt: f32) {
        if world.map_heat() <= 0.0 {
            self.heat = (self.heat - 30.0 * dt).max(0.0);
            return;
        }
        let cooling = Self::mode(world) == Element::Water
            || matches!(self.terrain_underfoot(world), Terrain::Shallows | Terrain::Ice | Terrain::Water);
        self.heat = (self.heat + if cooling { -18.0 } else { world.map_heat() } * dt).clamp(0.0, 100.0);
        if self.heat < 100.0 {
            return;
        }
        self.heat_tick -= dt;
        if self.heat_tick > 0.0 {
            return;
        }
        self.heat_tick = 1.0;
        let mut hit = Hit {
            power: 0.3,
            element: Element::Fire,
            kind: HitKind::Status,
            status_power: 0.5,
            from: self.base.pos,
            knockback: 0.0,
            can_crit: false,
            ..Hit::new(Faction::Neutral, combat::environment(world.chem.zone_level))
        };
        self.take_hit_inner(world, &mut hit);
    }

    fn update_blight(&mut self, world: &mut World, dt: f32) {
        if !world.map_blighted() {
            world.state.corruption = 0.0;
            return;
        }
        if self.terrain_underfoot(world) == Terrain::Blight && !self.airborne() && !self.is_dead() {
            world.state.corruption = (world.state.corruption + 5.0 * world.state.acc().blight_taken * dt).min(50.0);
            self.blight_tick -= dt;
            if self.blight_tick <= 0.0 {
                self.blight_tick = 0.7;
                let loss = (world.state.max_hp() / 40).max(1);
                world.state.hp = (world.state.hp - loss).max(1);
                Particle::burst(world, self.center(), Rgba::rgb(170, 90, 220), 3, 30.0, 0.3, false);
            }
        }
        world.state.hp = world.state.hp.min(world.state.max_hp());
    }

    fn start_fall(&mut self, world: &mut World) {
        self.fall_time = 0.0;
        self.cancel_actions();
        let water = self.terrain_underfoot(world) == Terrain::Water;
        audio::sfx(if water { Sound::Burn } else { Sound::MenuCancel });
        if water {
            Particle::burst(world, self.base.pos, Rgba::rgb(170, 220, 255), 12, 50.0, 0.4, false);
        }
    }

    fn update_fall(&mut self, world: &mut World, dt: f32) {
        self.fall_time += dt;
        if self.fall_time < FALL_DURATION {
            return;
        }
        self.fall_time = -1.0;
        self.base.pos = self.last_safe;
        self.invulnerable = 1.0;
        if world.state.acc().safe_falls {
            return;
        }
        let max = world.state.max_hp();
        let damage = (if world.lava_pits() { max / 4 } else { max / 12 }).max(2);
        world.state.hp = (world.state.hp - damage).max(1);
        audio::sfx(Sound::PlayerHurt);
        world.add(DamageNumber::hurt(self.center(), damage, Element::None, false));
    }

    fn update_status(&mut self, world: &mut World, dt: f32) {
        let (burn, pulse) = self.status.update(dt);
        if burn > 0.0 {
            let doubled = if pacts::active(&world.state, "kindling") { 2.0 } else { 1.0 };
            let damage = ((burn * 0.5 * doubled) as i32).max(1);
            world.state.hp = (world.state.hp - damage).max(1);
            world.add(DamageNumber::hurt(self.center(), damage, Element::Fire, false));
        }
        if pulse {
            self.stun_time = 0.15;
            Particle::burst(world, self.center(), Element::Thunder.glow(), 5, 30.0, 0.2, true);
        }
    }

    fn take_hit_inner(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if self.is_dead() || self.fall_time >= 0.0 || hit.source == Faction::Player {
            return false;
        }
        if hit.kind == HitKind::Environment && self.airborne() {
            return false;
        }
        if (self.dodging && self.dodge_invulnerable > 0.0)
            || self.invulnerable > 0.0
            || self.zipping
            || self.ward_time > 0.0
        {
            if self.dodging && !self.near_missed && hit.kind != HitKind::Environment {
                self.near_miss(world);
            }
            return false;
        }

        if hit.projectile && hit.owner.is_some() {
            let reflectable = hit.owner.and_then(|id| world.get_as::<Projectile>(id)).is_some_and(|s| s.reflectable);
            if self.reflect_time > 0.0 && hit.source == Faction::Enemy && reflectable {
                let dir = safe_dir(hit.from - self.center(), -self.aim);
                hit.reflect = Some((Faction::Player, Self::stats(world), dir));
                audio::play(Sound::Clink, 0.8, 0.5);
                world.add_sp(10.0);
                return true;
            }
            if self.aegis_time > 0.0 {
                hit.expire_owner = true;
                return true;
            }
            let at = hit.owner.and_then(|id| world.get(id)).map(|e| e.base().pos).unwrap_or(hit.from);
            let blocked = world
                .companion
                .and_then(|id| world.with_as::<Companion, _>(id, |c, w| c.try_block(w, at)))
                .unwrap_or(false);
            if blocked {
                hit.expire_owner = true;
                return true;
            }
        }

        if self.guarding() && !hit.unblockable && hit.kind != HitKind::Environment && self.facing_hit(hit.from) {
            return self.block(world, hit);
        }

        if self.art_absorbs(world, hit) {
            return true;
        }

        let mode = Self::mode(world);
        let fire_taken = if hit.element == Element::Fire { world.state.acc().fire_taken } else { 1.0 };
        let element_mod = combat::mode_resist(mode, hit.element) * fire_taken;
        let mut stats = world.state.stats(mode);
        stats.def *= self.status.defense_factor();
        let glass = if pacts::active(&world.state, "glass") { 1.5 } else { 1.0 };
        let awake = self.aether_active();
        let woven = |p: &str| world.state.mods().has(p);
        let last_stand = woven("last_stand") && world.state.hp * 10 < world.state.max_hp() * 3;
        let steady = awake || woven("bulwark");
        let resolve = if woven("iron_will") { 0.6 } else { 1.0 };
        let taken = glass * if awake { aether::AWAKE_DAMAGE } else { 1.0 } * if last_stand { 0.75 } else { 1.0 };
        let result = combat::resolve(hit, stats, element_mod, taken, &world.rng);
        hit.result = result;
        self.apply_damage(world, result.amount, hit.element, result.crit);

        let buildup = 30.0
            * hit.status_power
            * (1.0 + hit.attacker.foc / 150.0)
            * element_mod
            * if awake { 0.5 } else { 1.0 }
            * resolve;
        let ailment = self.status.add(hit.element, buildup, hit.attacker, false);
        if ailment == Ailment::Gust {
            self.knock(hit.from, 220.0, 0.25);
        }
        if ailment != Ailment::None {
            world.add(FloatingText::new(self.center() - Vec2::new(0.0, 10.0), ailment.label(), hit.element.glow()));
        }

        self.invulnerable = HURT_INVULNERABLE;
        world.rank.player_hurt();
        self.hurt_aether(world);
        if !self.is_dead() {
            self.trigger(world, Trigger::Hurt, Some(hit));
        }
        if hit.kind != HitKind::Environment && hit.power >= 0.4 && !steady {
            self.cancel_actions();
            let time = if self.airborne() { 0.0 } else { 0.14 };
            self.knock(hit.from, hit.knockback + 60.0, time);
        }
        audio::sfx(Sound::PlayerHurt);
        world.camera.shake(0.15, 1.0);
        world.hit_stop(0.05);
        true
    }

    fn apply_damage(&mut self, world: &mut World, amount: i32, element: Element, crit: bool) {
        world.state.hp = (world.state.hp - amount).max(0);
        world.player_hurt(amount);
        world.add(DamageNumber::hurt(self.center(), amount, element, crit));
        if world.state.hp > 0 {
            return;
        }
        if world.state.acc().auto_revive && !self.revived {
            self.revived = true;
            world.state.hp = world.state.max_hp() / 3;
            self.invulnerable = 2.0;
            audio::sfx(Sound::Heal);
            world.add(FloatingText::new(self.center() - Vec2::new(0.0, 14.0), "REVIVED!", palette::HIGHLIGHT));
            Particle::burst(world, self.center(), Rgba::rgb(255, 200, 120), 30, 80.0, 0.8, true);
            return;
        }
        self.death_time = 0.0001;
        self.jump_time = -1.0;
        self.z = 0.0;
        self.cancel_actions();
        audio::sfx(Sound::EnemyDie);
    }

    fn knock(&mut self, from: Vec2, speed: f32, time: f32) {
        let away = self.base.hitbox().center() - from;
        self.knock_velocity =
            if away.length_squared() > 0.01 { away.normalize() } else { -self.facing.to_vec() } * speed;
        self.knock_time = time;
    }

    pub fn cure_ailments(&mut self) {
        self.status.clear();
    }

    pub fn facing_toward(&self, target: Vec2) -> Direction {
        Direction::from_vec(target - self.base.pos, self.facing)
    }

    // ---- Combat timers and aim ----------------------------------------------------------------------------------

    fn update_combat_timers(&mut self, world: &mut World, dt: f32) {
        self.bolt_cooldown = (self.bolt_cooldown - dt).max(0.0);
        self.stun_time = (self.stun_time - dt).max(0.0);
        self.hidden_time = (self.hidden_time - dt).max(0.0);
        self.mode_flash = (self.mode_flash - dt).max(0.0);
        self.reflect_time = (self.reflect_time - dt).max(0.0);
        self.mirror_time = (self.mirror_time - dt).max(0.0);
        self.aegis_time = (self.aegis_time - dt).max(0.0);
        self.ward_time = (self.ward_time - dt).max(0.0);
        if let Some(echo) = self.echo.as_mut() {
            echo.2 -= dt;
        }

        if self.combo_reset > 0.0 {
            self.combo_reset -= dt;
            if self.combo_reset <= 0.0 {
                self.combo_step = 0;
            }
        }

        let max_charges = world.state.dodge_charges() as f32;
        if self.dodge_lockout > 0.0 {
            self.dodge_lockout -= dt;
        } else if self.dodge_charges < max_charges && !self.dodging {
            let wind = if Self::mode(world) == Element::Wind { 1.4 } else { 1.0 };
            self.dodge_recharge -= dt * wind * world.state.mods().dodge_recharge;
            if self.dodge_recharge <= 0.0 {
                self.dodge_charges = (self.dodge_charges + 1.0).min(max_charges);
                self.dodge_recharge = 0.3;
            }
        }

        self.update_aether(world, dt);
        self.dormant_note = (self.dormant_note - dt).max(0.0);

        if self.static_time > 0.0 {
            self.static_time -= dt;
            self.static_tick -= dt;
            if self.static_tick <= 0.0 {
                self.static_tick = 0.5;
                let center = self.center();
                let (radius, power) = (self.static_radius, self.static_power);
                self.strike_area(world, center, radius, None, move |p, w| {
                    p.make_hit(w, power, HitKind::Skill, Some(Element::Thunder), 0.5, 1.5, 30.0)
                });
                if world.rng.next_f32() < 0.5 {
                    audio::play(Sound::Clink, 0.3, 0.8);
                }
            }
        }
    }

    fn update_aim(&mut self, world: &World) {
        if let Some(direct) = input::direct_aim() {
            self.aim = direct;
        } else if input::mouse_aiming() {
            let mouse_world = world.camera.screen_to_world(input::mouse_screen());
            let to = mouse_world - self.center();
            if to.length_squared() > 4.0 {
                self.aim = to.normalize();
            }
        } else {
            let mut basis = if self.moving { self.move_dir } else { self.facing.to_vec() };
            if input::movement() != Vec2::ZERO {
                basis = input::movement().normalize();
            }
            self.aim = self.assist_aim(world, basis);
        }
    }

    /// Without a mouse or stick, nudge the aim toward the nearest enemy roughly in front.
    fn assist_aim(&self, world: &World, basis: Vec2) -> Vec2 {
        let mut best = None;
        let mut best_score = f32::MAX;
        for enemy in world.entities_of::<super::enemy::Enemy>() {
            if !enemy.targetable() {
                continue;
            }
            let to = enemy.hurtbox().center() - self.center();
            let distance = to.length();
            if !(1.0..=150.0).contains(&distance) {
                continue;
            }
            let dir = to / distance;
            let dot = dir.dot(basis);
            if dot < 0.82 {
                continue;
            }
            let score = distance * (2.0 - dot);
            if score < best_score {
                best_score = score;
                best = Some(dir);
            }
        }
        best.unwrap_or(basis)
    }

    /// Runs whichever action owns the player this frame; returns true if it also handled movement.
    fn update_actions(&mut self, world: &mut World, dt: f32) -> bool {
        if self.stun_time > 0.0 {
            return true;
        }
        if let Some(mut skill) = self.skill.take() {
            let done = skill.update(self, world, dt);
            if !done && self.skill.is_none() {
                self.skill = Some(skill);
            }
            return true;
        }
        if self.zipping {
            self.update_zip(world, dt);
            return true;
        }
        if self.dodging {
            self.update_dodge(world, dt);
            return true;
        }
        if self.hook.is_some() {
            return true;
        }

        if self.charging_skill() {
            self.update_skill_charge(world);
            return false;
        }
        if let Some((skill, level, delay)) = self.echo
            && delay <= 0.0
        {
            self.echo = None;
            self.skill_scale = 0.5;
            self.skill = Some(Box::new(SkillAction::create(skill, level)));
            world.add(SkillBanner::new(self.base.pos, skill, level));
            return true;
        }

        self.update_mode_switch(world);
        if self.wheel_open {
            return true;
        }

        if world.state.dormant() {
            return self.update_dormant(world);
        }

        if input::pressed(GameAction::Skill) && !self.airborne() && self.begin_skill_charge(world) {
            return false;
        }

        if input::pressed(GameAction::Dodge)
            && !self.airborne()
            && self.dodge_charges >= 1.0
            && self.dodge_lockout <= 0.0
        {
            let movement = input::movement();
            let dir = if movement != Vec2::ZERO { movement.normalize() } else { self.facing.to_vec() };
            self.start_dodge(world, dir, 0.2, 235.0, true);
            self.trigger(world, Trigger::Dodge, None);
            return true;
        }

        if input::pressed(GameAction::Tether) && world.state.has_tether() && !self.airborne() {
            self.cancel_actions();
            self.hook = Some(world.add(TetherHook::new(self.center(), self.aim)));
            audio::play(Sound::ShootWind, 0.6, 0.6);
            return true;
        }

        if input::pressed(GameAction::Attack) {
            let target = if self.airborne() { None } else { world.find_interactable(&self.interact_probe()) };
            if let Some(id) = target {
                if let Some(pos) = world.get(id).map(|e| e.base().pos) {
                    self.facing = self.facing_toward(pos);
                }
                self.interact = Some(id);
                input::consume();
                return true;
            }
            if self.attack_time >= 0.0 {
                self.combo_buffered = true;
            } else if !self.guarding() {
                self.start_swing(world);
            }
        }

        if self.attack_time >= 0.0 {
            self.update_swing(world, dt);
            return true;
        }

        self.update_throw(world, dt);
        self.update_guard(dt);
        false
    }

    /// Before the first sigil the hero can talk, open things and dodge, but the blade will not swing.
    fn update_dormant(&mut self, world: &mut World) -> bool {
        if input::pressed(GameAction::Attack) {
            let target = if self.airborne() { None } else { world.find_interactable(&self.interact_probe()) };
            if let Some(id) = target {
                if let Some(pos) = world.get(id).map(|e| e.base().pos) {
                    self.facing = self.facing_toward(pos);
                }
                self.interact = Some(id);
                input::consume();
                return true;
            }
            if self.dormant_note <= 0.0 {
                self.dormant_note = 1.5;
                audio::sfx(Sound::MenuCancel);
                let at = self.center() - Vec2::new(0.0, 14.0);
                world.add(FloatingText::new(at, "The blade won't wake", palette::TEXT_DIM));
            }
        }
        if input::pressed(GameAction::Dodge)
            && !self.airborne()
            && self.dodge_charges >= 1.0
            && self.dodge_lockout <= 0.0
        {
            let movement = input::movement();
            let dir = if movement != Vec2::ZERO { movement.normalize() } else { self.facing.to_vec() };
            self.start_dodge(world, dir, 0.2, 235.0, true);
            return true;
        }
        false
    }

    /// Tapping a mode key cycles modes; holding it opens a wheel (the world slows nearly to a stop) where a direction
    /// picks the mode: Wind up, Fire right, Water down, Thunder left.
    fn update_mode_switch(&mut self, world: &mut World) {
        let modes = world.state.owned_modes();
        let mode = Self::mode(world);
        let mut target = None;
        for (action, step) in [(GameAction::NextMode, 1), (GameAction::PrevMode, -1)] {
            if input::pressed(action) {
                self.wheel_hold = 0.0;
                self.wheel_step = step;
                self.wheel_pick = None;
            }
        }
        if self.wheel_hold >= 0.0 {
            if input::held(GameAction::NextMode) || input::held(GameAction::PrevMode) {
                self.wheel_hold += world.frame_delta;
                if self.wheel_hold >= WHEEL_DELAY && modes.len() > 1 {
                    self.wheel_open = true;
                    world.slow_motion(0.08, 0.1);
                    if let Some(pick) = wheel_direction(input::movement()).filter(|m| modes.contains(m)) {
                        if self.wheel_pick != Some(pick) {
                            audio::sfx(Sound::MenuMove);
                        }
                        self.wheel_pick = Some(pick);
                    }
                }
            } else {
                if self.wheel_open {
                    target = self.wheel_pick;
                } else if modes.len() > 1 {
                    target = Some(cycle(&modes, mode, self.wheel_step));
                }
                self.wheel_hold = -1.0;
                self.wheel_open = false;
            }
        }
        let scroll = input::wheel_steps();
        if scroll != 0 && modes.len() > 1 {
            target = Some(cycle(&modes, mode, scroll));
        }
        let direct = [GameAction::Mode1, GameAction::Mode2, GameAction::Mode3, GameAction::Mode4];
        for (i, action) in direct.iter().enumerate() {
            if input::pressed(*action) && modes.contains(&MODES[i]) {
                target = Some(MODES[i]);
            }
        }
        if let Some(m) = target
            && m != mode
        {
            self.switch_mode(world, m);
        }
    }

    pub fn switch_mode(&mut self, world: &mut World, mode: Element) {
        world.state.mode = mode;
        self.mode_flash = 0.35;
        audio::play(Sound::ChargeLevel, 0.5, 0.2);
        Particle::burst(world, self.center(), mode.glow(), 14, 50.0, 0.4, true);
        world.add(FloatingText::new(self.center() - Vec2::new(0.0, 14.0), mode.mode_label(), mode.glow()));
    }

    // ---- Melee --------------------------------------------------------------------------------------------------

    fn start_swing(&mut self, world: &mut World) {
        self.throw_held = -1.0;
        self.attack_time = 0.0;
        self.combo_buffered = false;
        self.swing_hit.clear();
        self.combo_reset = 0.0;
        self.facing = Direction::from_vec(self.aim, self.facing);
        audio::play(Sound::Swing, 1.0, self.combo_step as f32 * 0.12);
        let finisher = self.combo_step == 2;
        world.add(SlashEffect::new(
            self.center(),
            self.aim,
            Self::mode(world),
            if finisher { 30.0 } else { 23.0 },
            finisher,
            self.combo_step % 2 == 1,
        ));
    }

    fn update_swing(&mut self, world: &mut World, dt: f32) {
        self.attack_time += dt;
        let length = SWING_LENGTH[self.combo_step];
        let finisher = self.combo_step == 2;
        let lunge_end = if finisher { 0.13 } else { 0.09 };
        if self.attack_time < lunge_end {
            let delta = self.aim * if finisher { 120.0 } else { 45.0 } * dt;
            self.move_by(world, delta, false, true);
        }

        let (active_start, active_end) = if finisher { (0.07, 0.2) } else { (0.04, 0.14) };
        let reach = if Self::supports(world, Group::Attack).cleave { 1.3 } else { 1.0 };
        if self.attack_time >= active_start && self.attack_time <= active_end {
            let radius = if finisher { 30.0 } else { 23.0 } * reach;
            for (id, faction, hurtbox) in world.damageables() {
                if faction == Faction::Player
                    || self.swing_hit.contains(&id)
                    || !self.in_arc(&hurtbox, radius, if finisher { 105.0 } else { 80.0 } * reach)
                {
                    continue;
                }
                self.swing_hit.push(id);
                let (kind, poise, status, knock) =
                    if finisher { (HitKind::Finisher, 3.0, 1.4, 210.0) } else { (HitKind::Melee, 1.0, 0.8, 90.0) };
                let power = if finisher { 1.6 * world.state.mods().finisher } else { 1.0 };
                let hit = self.make_hit(world, power, kind, None, poise, status, knock);
                self.strike_target(world, id, hit);
            }
        }

        if self.attack_time >= active_start && self.attack_time - world.frame_delta < active_start {
            self.mow_grass(world, if finisher { 30.0 } else { 23.0 });
        }

        if self.attack_time < length {
            return;
        }
        self.attack_time = -1.0;
        if self.combo_buffered {
            self.combo_step = (self.combo_step + 1) % 3;
            self.start_swing(world);
        } else {
            self.combo_step = if finisher { 0 } else { self.combo_step + 1 };
            self.combo_reset = 0.35;
        }
    }

    fn mow_grass(&mut self, world: &mut World, radius: f32) {
        let (tx, ty) = self.tile_pos();
        for y in -2..=2 {
            for x in -2..=2 {
                let center = tile_center(tx + x, ty + y);
                let to = center - self.center();
                if to.length() <= radius + 6.0 && (to.length() < 8.0 || to.normalize().dot(self.aim) > 0.35) {
                    world.chem_cut(tx + x, ty + y);
                }
            }
        }
    }

    /// Whether a hurtbox lies within `radius` of the player and inside the aimed arc.
    pub fn in_arc(&self, b: &Box2, radius: f32, half_angle_degrees: f32) -> bool {
        let c = self.center();
        let closest = Vec2::new(c.x.clamp(b.x, b.right()), c.y.clamp(b.y, b.bottom()));
        let distance = (closest - c).length();
        if distance > radius {
            return false;
        }
        if distance < 7.0 {
            return true;
        }
        let to_center = b.center() - c;
        to_center.normalize_or_zero().dot(self.aim) >= half_angle_degrees.to_radians().cos()
    }

    // ---- Throwing -----------------------------------------------------------------------------------------------

    fn update_throw(&mut self, world: &mut World, dt: f32) {
        if input::pressed(GameAction::Throw) && !self.guarding() {
            self.throw_held = 0.0;
            self.charged_sound_played = false;
        }
        if self.throw_held < 0.0 {
            return;
        }
        if input::held(GameAction::Throw) {
            self.throw_held += dt;
            if self.throw_charge() >= 1.0 && !self.charged_sound_played {
                self.charged_sound_played = true;
                audio::play(Sound::ChargeLevel, 0.8, 0.1);
            }
            return;
        }
        let charged = self.throw_charge() >= 1.0;
        self.throw_held = -1.0;
        if self.bolt_cooldown > 0.0 && !charged {
            return;
        }
        self.throw_bolt(world, charged);
    }

    fn throw_bolt(&mut self, world: &mut World, charged: bool) {
        self.bolt_cooldown = if charged { 0.3 } else { 0.16 };
        self.facing = Direction::from_vec(self.aim, self.facing);
        let bolt = self.make_bolt(world, self.aim, charged, -1.0);
        world.add(bolt);
        if Self::supports(world, Group::Ranged).multishot {
            for angle in [-0.22, 0.22] {
                let side = self.make_bolt(world, crate::core::geom::rotate(self.aim, angle), charged, -1.0);
                world.add(side);
            }
        }
        if charged {
            audio::play(if Self::mode(world) == Element::Fire { Sound::ShootFire } else { Sound::ShootWind }, 0.9, 0.1);
        } else {
            audio::play(Sound::ShootWind, 0.45, 0.6);
        }
    }

    pub fn make_bolt(&self, world: &World, dir: Vec2, charged: bool, power: f32) -> Projectile {
        let mode = Self::mode(world);
        let mut bolt = Projectile::new(
            Faction::Player,
            Self::stats(world),
            self.base.pos + dir * 6.0 - Vec2::new(0.0, 2.0),
            dir * if charged { 320.0 } else { 280.0 },
        );
        bolt.power = if power >= 0.0 {
            power
        } else if charged {
            1.4
        } else {
            0.6
        };
        bolt.element = mode;
        bolt.kind = if charged { HitKind::ChargedBolt } else { HitKind::Bolt };
        bolt.poise = if charged { 2.0 } else { 0.5 };
        bolt.status_power = if charged { 1.6 } else { 0.6 };
        bolt.knockback = if charged { 160.0 } else { 60.0 };
        bolt.z = 7.0;
        bolt.lifetime = if charged { 0.75 } else { 0.55 };
        bolt.piercing = charged;
        bolt.bounces = if charged { 2 } else { 1 } + world.state.mods().bounces;
        bolt.paints_path = charged && matches!(mode, Element::Water | Element::Fire);
        bolt.size = if charged { 10.0 } else { 7.0 };
        bolt.texture = art::bolt(mode, charged);
        bolt.rotate_to_velocity = true;
        bolt.trail_color = mode.glow();
        bolt.hook = ProjectileHook::PlayerShot;
        if power < 0.0 {
            let gems = Self::supports(world, Group::Ranged);
            bolt.power *= gems.damage;
            bolt.poise *= gems.poise;
            bolt.status_power *= gems.status;
            bolt.piercing |= gems.pierce;
        }
        bolt
    }

    /// Applies the special group's gems and the Echo scale to a skill's own projectile.
    pub fn scale_skill_shot(&self, world: &World, shot: &mut Projectile) {
        let gems = Self::supports(world, Group::Special);
        shot.power *= gems.damage * self.skill_scale;
        shot.poise *= gems.poise;
        shot.status_power *= gems.status;
        shot.paints_path |= gems.linger;
    }

    // ---- Dodging ------------------------------------------------------------------------------------------------

    pub fn start_dodge(&mut self, world: &mut World, dir: Vec2, length: f32, speed: f32, consume_charge: bool) {
        self.cancel_actions();
        self.dodging = true;
        self.near_missed = false;
        self.dodge_trail = false;
        self.dodge_shock = false;
        self.dodge_rammed.clear();
        self.dodge_dir = dir;
        self.dodge_time = 0.0;
        let wind = if Self::mode(world) == Element::Wind { 1.1 } else { 1.0 };
        let gale = if consume_charge && world.state.has("gale_dash") { 1.3 } else { 1.0 };
        self.dodge_length = length * wind * gale;
        self.dodge_speed = speed;
        self.dodge_invulnerable = length * 0.85;
        if consume_charge {
            self.dodge_charges -= 1.0;
            self.dodge_recharge = 0.45;
            if self.dodge_charges < 1.0 {
                self.dodge_lockout = 0.7;
            }
        }
        self.facing = Direction::from_vec(dir, self.facing);
        audio::play(Sound::ShootWind, 0.4, 0.9);
    }

    pub fn dodging(&self) -> bool {
        self.dodging
    }

    pub fn update_dodge(&mut self, world: &mut World, dt: f32) {
        self.dodge_time += dt;
        self.dodge_invulnerable -= dt;
        let gale = world.state.has("gale_dash");
        let delta = self.dodge_dir * self.dodge_speed * dt;
        self.move_by(world, delta, gale, true);
        if world.state.soul.as_deref() == Some("ember") {
            let (tx, ty) = self.tile_pos();
            world.chem_brush(tx, ty, Element::Fire, false);
        }
        if world.rng.next_f32() < 0.7 {
            world.add(Afterimage::new(self.base.pos - Vec2::new(0.0, self.z), self.facing, Self::mode(world)));
        }
        if self.dodge_trail {
            let (tx, ty) = self.tile_pos();
            if !world.entities_of::<FirePatch>().any(|f| f.base().tile_pos() == (tx, ty)) {
                world.add(FirePatch::new(self.base.pos, 2.5));
                world.chem_ignite(tx, ty);
            }
        }
        if self.dodge_shock {
            let center = self.center();
            let mut rammed = std::mem::take(&mut self.dodge_rammed);
            self.strike_area(world, center, 16.0, Some(&mut rammed), |p, w| {
                p.make_hit(w, 1.0, HitKind::Skill, Some(Element::Thunder), 1.5, 2.0, 120.0)
            });
            self.dodge_rammed = rammed;
            if world.rng.next_f32() < 0.5 {
                Particle::burst(world, center, Element::Thunder.glow(), 2, 30.0, 0.2, true);
            }
        }
        if self.dodge_time < self.dodge_length {
            return;
        }
        self.dodging = false;
        self.dodge_invulnerable = 0.0;
    }

    fn near_miss(&mut self, world: &mut World) {
        self.near_missed = true;
        self.trigger(world, Trigger::CloseCall, None);
        world.add_sp(30.0);
        world.slow_motion(0.35, 0.35);
        world.add(FloatingText::new(self.center() - Vec2::new(0.0, 16.0), "Close call!", palette::HIGHLIGHT));
        audio::play(Sound::ChargeLevel, 0.6, 0.4);
    }

    // ---- Guarding -----------------------------------------------------------------------------------------------

    fn update_guard(&mut self, dt: f32) {
        if input::held(GameAction::Guard) && !self.airborne() && self.throw_held < 0.0 {
            if self.guard_time < 0.0 {
                self.guard_time = 0.0;
                audio::play(Sound::Clink, 0.25, -0.4);
            } else {
                self.guard_time += dt;
            }
        } else {
            self.guard_time = -1.0;
        }
    }

    fn facing_hit(&self, from: Vec2) -> bool {
        let to = from - self.center();
        if to.length_squared() < 1.0 {
            return true;
        }
        to.normalize().dot(self.aim) > 85f32.to_radians().cos()
    }

    fn perfect_window(&self, world: &World) -> f32 {
        let mods = world.state.mods();
        let window = 0.15 + world.state.acc().perfect_window + mods.perfect_window;
        if mods.has("stillwater") { window * 2.0 } else { window }
    }

    fn block(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        let perfect = !pacts::active(&world.state, "brittle")
            && self.guard_time >= 0.0
            && self.guard_time < self.perfect_window(world);
        if perfect {
            hit.result = DamageResult::blocked(true);
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
            return true;
        }

        let heavy = hit.power >= 1.5;
        if heavy {
            let mode = Self::mode(world);
            let result =
                combat::resolve(hit, world.state.stats(mode), combat::mode_resist(mode, hit.element), 0.3, &world.rng);
            self.apply_damage(world, result.amount, hit.element, false);
        }
        hit.result = DamageResult::blocked(false);
        audio::play(Sound::Clink, 0.8, -0.2);
        Particle::burst(world, self.center() + self.aim * 8.0, Rgba::rgb(220, 220, 255), 6, 40.0, 0.2, true);
        self.knock(hit.from, if heavy { 160.0 } else { 90.0 }, 0.08);
        if heavy {
            self.guard_time = -1.0;
            self.stun_time = 0.35;
        }
        if hit.projectile {
            hit.expire_owner = true;
        }
        true
    }

    fn counter(&mut self, world: &mut World, hit: &Hit) {
        let at = hit.owner.and_then(|id| world.get(id)).map(|e| e.base().hitbox().center()).unwrap_or(hit.from);
        self.aim = safe_dir(at - self.center(), self.aim);
        world.add(SlashEffect::new(self.center(), self.aim, Self::mode(world), 34.0, true, false));
        for (id, faction, hurtbox) in world.damageables() {
            if faction != Faction::Player && self.in_arc(&hurtbox, 40.0, 110.0) {
                let h = self.make_hit(world, 3.0, HitKind::Skill, None, 6.0, 2.0, 260.0);
                self.strike_target(world, id, h);
            }
        }
        world.camera.shake(0.2, 2.0);
    }

    // ---- Overload, SP and skills --------------------------------------------------------------------------------

    /// The highest level the equipped skill can reach right now: limited by unlocked levels and SP.
    fn skill_cap(world: &World) -> i32 {
        let gems = Self::supports(world, Group::Special);
        let pips = if world.state.mods().has("blood_art") {
            ((world.state.hp - 1) as f32 / Self::blood_cost(world)).floor() as i32
        } else {
            (world.state.sp / 100.0).floor() as i32
        };
        let affordable = (1..=skills::MAX_LEVEL).rev().find(|&l| gems.skill_cost(l) <= pips).unwrap_or(0);
        world.state.skill_level(Self::mode(world)).min(affordable)
    }

    /// Starts charging the equipped skill; returns false if there is nothing to charge.
    fn begin_skill_charge(&mut self, world: &mut World) -> bool {
        if world.state.equipped_skill(Self::mode(world)).is_none() {
            return false;
        }
        if Self::skill_cap(world) < 1 {
            audio::sfx(Sound::MenuCancel);
            world.add(FloatingText::new(self.center() - Vec2::new(0.0, 16.0), "Not enough SP", palette::TEXT_DIM));
            return false;
        }
        self.cancel_actions();
        self.skill_held = 0.0;
        self.skill_level_reached = 1;
        true
    }

    fn update_skill_charge(&mut self, world: &mut World) {
        self.skill_held += world.frame_delta;
        let cap = Self::skill_cap(world).max(1);
        let speed = Self::supports(world, Group::Special).charge_speed;
        let reached = skills::level_for_hold(self.skill_held * speed).min(cap);
        if reached > self.skill_level_reached {
            self.skill_level_reached = reached;
            audio::play(Sound::ChargeLevel, 0.8, 0.15 * reached as f32);
            Particle::burst(world, self.center(), Self::mode(world).glow(), 6 * reached, 50.0, 0.3, true);
        }
        if self.skill_held >= skills::TAP_WINDOW && cap > 1 {
            world.slow_motion(0.05, 0.3);
        }
        if !input::held(GameAction::Skill) || self.skill_held * speed >= skills::MAX_HOLD {
            let level = self.skill_level_reached;
            self.skill_held = -1.0;
            self.fire_skill(world, level);
        }
    }

    fn fire_skill(&mut self, world: &mut World, level: i32) {
        let Some(skill) = world.state.equipped_skill(Self::mode(world)) else { return };
        let level = level.min(Self::skill_cap(world));
        if level < 1 {
            return;
        }
        self.cancel_actions();
        let gems = Self::supports(world, Group::Special);
        let pips = gems.skill_cost(level);
        if world.state.mods().has("blood_art") {
            let cost = (Self::blood_cost(world) * pips as f32).round() as i32;
            world.state.hp = (world.state.hp - cost).max(1);
            world.add(DamageNumber::hurt(self.center(), cost, Element::None, false));
        } else {
            world.state.sp -= pips as f32 * 100.0;
        }
        self.skill_scale = 1.0;
        self.echo = gems.echo.then_some((skill, level, 0.45));
        self.skill = Some(Box::new(SkillAction::create(skill, level)));
        self.last_skill = Some((skill, level));
        world.add(SkillBanner::new(self.base.pos, skill, level));
        if level >= skills::MAX_LEVEL {
            self.trigger(world, Trigger::GreatSkill, None);
        }
    }

    /// Life spent per skill level under the Blood Art keystone.
    fn blood_cost(world: &World) -> f32 {
        world.state.max_hp() as f32 * 0.08
    }

    /// The Echo soul performs the last skill again, at the same level, for free.
    pub fn replay_last_skill(&mut self, world: &mut World) -> bool {
        let Some((skill, level)) = self.last_skill else { return false };
        if !self.can_act() || self.airborne() {
            return false;
        }
        self.cancel_actions();
        self.skill = Some(Box::new(SkillAction::create(skill, level)));
        world.add(SkillBanner::new(self.base.pos, skill, level));
        true
    }

    /// Wards the player from all harm for a while (Sanctum).
    pub fn ward(&mut self, seconds: f32) {
        self.ward_time = seconds;
    }

    pub fn begin_aegis(&mut self, seconds: f32) {
        self.aegis_time = seconds;
    }

    pub fn begin_reflect(&mut self, seconds: f32) {
        self.reflect_time = seconds;
    }

    /// An ice shell that absorbs `charges` hits; with `nova`, each break also chills everything nearby.
    pub fn begin_mirror(&mut self, seconds: f32, charges: i32, nova: bool) {
        self.mirror_time = seconds;
        self.mirror_charges = charges;
        self.mirror_nova = nova;
    }

    pub fn begin_static(&mut self, seconds: f32, radius: f32, power: f32) {
        self.static_time = seconds;
        self.static_radius = radius;
        self.static_power = power;
    }

    pub fn hide(&mut self, seconds: f32) {
        self.hidden_time = seconds;
    }

    fn art_absorbs(&mut self, world: &mut World, hit: &Hit) -> bool {
        if self.mirror_time <= 0.0 || hit.kind == HitKind::Environment {
            return false;
        }
        self.mirror_charges -= 1;
        if self.mirror_charges <= 0 {
            self.mirror_time = 0.0;
        }
        if self.mirror_nova {
            let center = self.center();
            self.strike_area(world, center, 48.0, None, |p, w| {
                p.make_hit(w, 0.8, HitKind::Skill, Some(Element::Water), 1.5, 3.0, 160.0)
            });
            world.add(Shockwave::new(self.base.pos, 48.0, Element::Water.glow()));
        }
        audio::play(Sound::Break, 0.7, 0.5);
        Particle::burst(world, self.center(), Rgba::rgb(200, 240, 255), 24, 80.0, 0.5, true);
        if let Some(owner) = hit.owner {
            let counter = self.make_hit(world, 0.8, HitKind::Skill, Some(Element::Water), 1.0, 4.0, 120.0);
            let mut counter = counter;
            world.hit(owner, &mut counter);
        }
        self.invulnerable = 0.6;
        true
    }

    // ---- Shared helpers -----------------------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn make_hit(
        &self,
        world: &World,
        power: f32,
        kind: HitKind,
        element: Option<Element>,
        poise: f32,
        status: f32,
        knock: f32,
    ) -> Hit {
        let (power, poise, status) = match Group::of(kind) {
            Some(group) => {
                let gems = Self::supports(world, group);
                let scale = if kind == HitKind::Skill { self.skill_scale } else { 1.0 };
                (power * gems.damage_for(kind) * scale, poise * gems.poise, status * gems.status)
            }
            None => (power, poise, status),
        };
        Hit {
            power,
            element: element.unwrap_or(Self::mode(world)),
            kind,
            poise,
            status_power: status,
            knockback: knock,
            from: self.center(),
            owner: Some(PLAYER_ID),
            aether: self.aether_active(),
            ..Hit::new(Faction::Player, Self::stats(world))
        }
    }

    pub fn strike_target(&mut self, world: &mut World, target: EntityId, mut hit: Hit) {
        let faction = world.get(target).and_then(|e| e.faction()).unwrap_or(Faction::Neutral);
        let at = world.get(target).map(|e| e.base().pos);
        if world.hit(target, &mut hit) {
            self.on_hit_dealt(world, faction, &hit);
            if let Some(at) = at
                && hit.kind == HitKind::Skill
                && hit.element != Element::None
                && Self::supports(world, Group::Special).linger
            {
                let t = TILE as f32;
                world.chem_apply((at.x / t).floor() as i32, ((at.y - 3.0) / t).floor() as i32, hit.element, true);
            }
        }
    }

    /// Hits every enemy (and puzzle object) within `radius` of a point; `already` skips (and collects) repeat targets.
    pub fn strike_area(
        &mut self,
        world: &mut World,
        center: Vec2,
        radius: f32,
        mut already: Option<&mut Vec<EntityId>>,
        make: impl Fn(&Player, &World) -> Hit,
    ) -> i32 {
        let mut count = 0;
        for (id, faction, b) in world.damageables() {
            if faction == Faction::Player || already.as_ref().is_some_and(|a| a.contains(&id)) {
                continue;
            }
            let closest = Vec2::new(center.x.clamp(b.x, b.right()), center.y.clamp(b.y, b.bottom()));
            if closest.distance(center) > radius {
                continue;
            }
            if let Some(a) = already.as_mut() {
                a.push(id);
            }
            let mut hit = make(self, world);
            hit.from = center;
            hit.owner = Some(PLAYER_ID);
            self.strike_target(world, id, hit);
            count += 1;
        }
        count
    }

    pub fn on_hit_dealt(&mut self, world: &mut World, target: Faction, hit: &Hit) {
        if target != Faction::Enemy || hit.result.amount <= 0 {
            return;
        }
        let sp = match hit.kind {
            HitKind::Melee => 9.0,
            HitKind::Finisher => 15.0,
            HitKind::Bolt => 5.0,
            HitKind::ChargedBolt => 12.0,
            HitKind::Skill => 3.0,
            _ => 2.0,
        };
        let gems = Group::of(hit.kind).map(|g| Self::supports(world, g)).unwrap_or_default();
        world.add_sp(sp * gems.sp_gain);
        if gems.leech > 0.0 {
            let amount = (hit.result.amount as f32 * gems.leech).round() as i32;
            if amount > 0 {
                world.heal_player(amount);
            }
        }
        self.feed_aether(world, hit);
    }

    /// The Meteor Art's landing: a fiery burst where the shot comes down.
    pub fn meteor_burst(&mut self, world: &mut World, at: Vec2) {
        self.strike_area(world, at - Vec2::new(0.0, 4.0), 30.0, None, |p, w| {
            p.make_hit(w, 2.3, HitKind::Skill, Some(Element::Fire), 4.0, 2.5, 200.0)
        });
        ignite_around(world, at, 1);
        world.add(Shockwave::new(at, 30.0, Element::Fire.glow()));
        world.camera.shake(0.2, 2.0);
        audio::play(Sound::Break, 0.8, 0.2);
    }

    /// The aimed point on the ground, at most `max_distance` away.
    pub fn aim_point(&self, world: &World, max_distance: f32) -> Vec2 {
        if input::mouse_aiming() && input::direct_aim().is_none() {
            let mouse = world.camera.screen_to_world(input::mouse_screen()) + Vec2::new(0.0, 8.0);
            let to = mouse - self.base.pos;
            return if to.length() <= max_distance {
                mouse
            } else {
                self.base.pos + to.normalize_or_zero() * max_distance
            };
        }
        let mut best: Option<Vec2> = None;
        let mut best_score = f32::MAX;
        for enemy in world.entities_of::<super::enemy::Enemy>() {
            if !enemy.targetable() {
                continue;
            }
            let to = enemy.base().pos - self.base.pos;
            let d = to.length();
            if d > max_distance || d < 1.0 || (to / d).dot(self.aim) < 0.7 {
                continue;
            }
            if d < best_score {
                best_score = d;
                best = Some(enemy.base().pos);
            }
        }
        best.unwrap_or(self.base.pos + self.aim * max_distance * 0.7)
    }

    pub fn cancel_actions(&mut self) {
        self.attack_time = -1.0;
        self.combo_buffered = false;
        self.throw_held = -1.0;
        self.guard_time = -1.0;
        self.skill = None;
        self.skill_held = -1.0;
        if let Some(hook) = self.hook.take() {
            self.released_hook = Some(hook);
        }
        self.dodging = false;
        if self.zipping {
            self.zipping = false;
            self.z = 0.0;
        }
    }

    /// Lets go of a hook dropped by a cancelled action; the hook is a separate entity, so this waits for the world.
    fn flush_released_hook(&mut self, world: &mut World) {
        if let Some(id) = self.released_hook.take() {
            world.with_as::<TetherHook, _>(id, |h, w| h.release(w));
        }
    }

    pub fn hook_done(&mut self) {
        self.hook = None;
    }

    pub fn start_zip(&mut self, destination: Vec2) {
        self.zipping = true;
        self.zip_target = destination;
        self.zip_time = 0.0;
    }

    fn update_zip(&mut self, world: &mut World, dt: f32) {
        self.zip_time += dt;
        let to = self.zip_target - self.base.pos;
        let distance = to.length();
        self.z = 6.0;
        if distance < 5.0 || self.zip_time > 1.2 {
            self.end_zip(world);
            return;
        }
        let moved = self.move_by(world, to / distance * distance.min(340.0 * dt), true, false);
        if moved.length_squared() < 0.01 {
            self.end_zip(world);
        }
    }

    fn end_zip(&mut self, world: &mut World) {
        self.zipping = false;
        self.z = 0.0;
        self.invulnerable = self.invulnerable.max(0.2);
        if self.terrain_underfoot(world).is_hazard() {
            self.start_fall(world);
        }
    }

    fn interact_probe(&self) -> Box2 {
        let center = Vec2::new(self.base.pos.x, self.base.pos.y - 5.0) + self.facing.to_vec() * 12.0;
        Box2::from_center(center, 12.0, 14.0)
    }

    // ---- Drawing ------------------------------------------------------------------------------------------------

    fn current_pose(&self) -> (HeroPose, i32) {
        if self.is_dead() || self.knock_time > 0.0 || self.stun_time > 0.0 {
            return (HeroPose::Hurt, 0);
        }
        if self.zipping || self.jump_time >= 0.0 {
            return (HeroPose::Jump, 0);
        }
        if self.dodging {
            return (HeroPose::Dodge, 0);
        }
        if self.attack_time >= 0.0 {
            let t = self.attack_time / SWING_LENGTH[self.combo_step];
            return (
                HeroPose::Slash,
                if t < 0.25 {
                    0
                } else if t < 0.65 {
                    1
                } else {
                    2
                },
            );
        }
        if self.skill.is_some() {
            return (HeroPose::Slash, 1);
        }
        if self.charging_skill() {
            return (HeroPose::Guard, 0);
        }
        if self.hook.is_some() {
            return (HeroPose::Throw, 1);
        }
        if self.guarding() {
            return (HeroPose::Guard, 0);
        }
        if self.aiming() {
            return (HeroPose::Throw, 0);
        }
        if self.bolt_cooldown > 0.06 {
            return (HeroPose::Throw, 1);
        }
        if self.moving {
            return (HeroPose::Run, (self.walk_time * 11.0) as i32 % 6);
        }
        (HeroPose::Idle, (self.time * 1.6) as i32 % 2)
    }

    fn swing_angle(&self) -> f32 {
        if self.attack_time < 0.0 {
            return 0.0;
        }
        let t = (self.attack_time / SWING_LENGTH[self.combo_step] * 1.4).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        let sweep = if self.combo_step == 2 { 2.4 } else { 1.9 };
        let (mut start, mut end) = (-sweep / 2.0, sweep / 2.0);
        if self.combo_step == 1 {
            std::mem::swap(&mut start, &mut end);
        }
        start + (end - start) * eased
    }

    fn draw_sword(&self, world: &World, batch: &mut Batch, lift: Vec2, swing: f32) {
        let angle = self.aim.y.atan2(self.aim.x) + swing;
        let dir = Vec2::new(angle.cos(), angle.sin());
        let hand = self.center() + lift + Vec2::new(0.0, 2.0) + dir * 3.0;
        batch.draw_ex(
            art::blade(Self::mode(world)),
            snap(hand),
            None,
            Rgba::WHITE,
            angle,
            Vec2::new(1.0, 3.5),
            Vec2::ONE,
            false,
            false,
        );
    }

    fn draw_shield(&self, world: &World, batch: &mut Batch, lift: Vec2) {
        let perfect = self.guard_time >= 0.0 && self.guard_time < self.perfect_window(world);
        let at = snap(self.center() + lift + self.aim * 7.0 + Vec2::new(0.0, 2.0));
        let color = if perfect { Rgba::WHITE.lerp(palette::HIGHLIGHT, 0.6) } else { Rgba::WHITE };
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

    fn draw_charge(&self, world: &World, batch: &mut Batch, lift: Vec2) {
        let charge = self.throw_charge();
        let center = self.center() + lift + self.aim * 6.0;
        let glow = Self::mode(world).glow();
        let t = world.time as f32;
        let color = if charge >= 1.0 {
            glow.lerp(Rgba::WHITE, 0.5 + (t * 20.0).sin() * 0.3)
        } else {
            glow.mul(0.4 + charge * 0.6)
        };
        let sparks = 3 + (charge * 5.0) as i32;
        for i in 0..sparks {
            let a = t * (6.0 + charge * 8.0) + i as f32 * TAU / sparks as f32;
            let r = 6.0 - charge * 2.0;
            batch.draw(props::spark(), snap(center + Vec2::new(a.cos(), a.sin()) * r - Vec2::ONE), color);
        }
    }

    /// Pips circling the hero while a skill charges: one per level it can reach, lit as each is reached.
    fn draw_skill_charge(&self, world: &World, batch: &mut Batch, lift: Vec2) {
        let glow = Self::mode(world).glow();
        let cap = Self::skill_cap(world).max(1);
        let center = self.center() + lift;
        let spin = world.time as f32 * 4.0;
        for i in 0..cap {
            let a = spin + i as f32 * TAU / cap as f32;
            let lit = i < self.skill_level_reached;
            let color = if lit { glow.lerp(Rgba::WHITE, 0.35) } else { glow.mul(0.35) };
            let at = snap(center + Vec2::new(a.cos() * 14.0, a.sin() * 9.0));
            let size = if lit { 3.0 } else { 2.0 };
            batch.fill_rect(at.x - 1.0, at.y - 1.0, size + 2.0, size + 2.0, palette::OUTLINE);
            batch.fill_rect(at.x, at.y, size, size, color);
        }
    }

    fn draw_orbit(&self, world: &World, batch: &mut Batch, lift: Vec2, color: Rgba, radius: f32, remaining: f32) {
        if remaining < 1.0 && (remaining * 10.0) as i32 % 2 == 0 {
            return;
        }
        let center = self.center() + lift;
        for i in 0..6 {
            let a = world.time as f32 * 7.0 + i as f32 * TAU / 6.0;
            batch.draw(
                props::spark(),
                snap(center + Vec2::new(a.cos() * radius, a.sin() * radius * 0.6) - Vec2::ONE),
                color,
            );
        }
    }
}

fn cycle(modes: &[Element], from: Element, step: i32) -> Element {
    let index = modes.iter().position(|&m| m == from).unwrap_or(0) as i32;
    let n = modes.len() as i32;
    modes[(index + step).rem_euclid(n) as usize]
}

/// The quick-change wheel's mode in a stick direction; opposite elements face each other.
pub fn wheel_direction(dir: Vec2) -> Option<Element> {
    if dir.length() < 0.5 {
        None
    } else if dir.y.abs() > dir.x.abs() {
        Some(if dir.y < 0.0 { Element::Wind } else { Element::Water })
    } else {
        Some(if dir.x > 0.0 { Element::Fire } else { Element::Thunder })
    }
}

pub fn ignite_around(world: &mut World, at: Vec2, radius: i32) {
    let t = TILE as f32;
    let (cx, cy) = ((at.x / t).floor() as i32, ((at.y - 3.0) / t).floor() as i32);
    for y in -radius..=radius {
        for x in -radius..=radius {
            world.chem_ignite(cx + x, cy + y);
        }
    }
}

impl Entity for Player {
    entity_base!();

    fn light(&self, world: &World) -> Option<(f32, Rgba)> {
        let mode = Self::mode(world);
        Some((if mode == Element::None { 56.0 } else { 64.0 }, Rgba::rgb(255, 230, 190).lerp(mode.glow(), 0.4)))
    }

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Player)
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 5.0, self.base.pos.y - 14.0 - self.z, 10.0, 14.0)
    }

    fn added(&mut self, world: &mut World) {
        self.last_safe = self.base.pos;
        self.dodge_charges = world.state.dodge_charges() as f32;
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.update_player(world, dt);
        self.flush_released_hook(world);
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        let connected = self.take_hit_inner(world, hit);
        self.flush_released_hook(world);
        connected
    }

    fn grounded(&self) -> bool {
        !self.airborne()
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let fall = if self.fall_time >= 0.0 { self.fall_time / FALL_DURATION } else { 0.0 };
        if fall < 0.15 || self.fall_time < 0.0 {
            batch.draw(
                props::shadow(12),
                snap(self.base.pos - Vec2::new(6.0, 3.0)),
                Rgba::WHITE.mul(1.0 - self.z / 30.0),
            );
        }

        let blink = self.invulnerable > 0.0
            && self.knock_time <= 0.0
            && (self.invulnerable * 20.0) as i32 % 2 == 0
            && !self.is_dead();
        let lift = Vec2::new(0.0, -self.z.round());
        let (pose, frame) = self.current_pose();
        let swing = self.swing_angle();

        let sword_behind = self.attack_time >= 0.0 && self.aim.y < -0.3;
        if sword_behind {
            self.draw_sword(world, batch, lift, swing);
        }
        if self.guarding() && self.aim.y < -0.3 {
            self.draw_shield(world, batch, lift);
        }

        if !blink {
            let mut tint = if self.is_dead() {
                Rgba::WHITE.lerp(Rgba::RED, self.death_time.min(1.0))
            } else {
                self.status.tint().unwrap_or(Rgba::WHITE)
            };
            if self.hidden() {
                tint = tint.mul(0.35);
            }
            tint = tint.mul(1.0 - fall);
            let feet = self.base.pos + lift + Vec2::new(0.0, fall * 6.0);
            let mode = Self::mode(world);
            if self.mode_flash > 0.0 {
                for offset in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)] {
                    hero_sprite::draw(
                        batch,
                        feet + offset,
                        self.facing,
                        pose,
                        frame,
                        mode.glow().mul(self.mode_flash / 0.35),
                        true,
                        1.0,
                        0.0,
                    );
                }
            }
            let rotation = if self.is_dead() { (self.death_time * 3.0).min(1.0) * FRAC_PI_2 } else { 0.0 };
            hero_sprite::draw(batch, feet, self.facing, pose, frame, tint, false, 1.0 - fall * 0.7, rotation);
            if self.aether_active() {
                let pulse = 0.2 + 0.15 * (self.time as f32 * 8.0).sin().abs();
                hero_sprite::draw(
                    batch,
                    feet,
                    self.facing,
                    pose,
                    frame,
                    Element::Aether.glow().mul(pulse),
                    true,
                    1.0,
                    0.0,
                );
            }
        }

        if !sword_behind && self.attack_time >= 0.0 {
            self.draw_sword(world, batch, lift, swing);
        }
        if self.guarding() && self.aim.y >= -0.3 {
            self.draw_shield(world, batch, lift);
        }
        if self.aiming() {
            self.draw_charge(world, batch, lift);
        }
        if self.reflect_time > 0.0 {
            self.draw_orbit(world, batch, lift, Element::Wind.light(), 16.0, self.reflect_time);
        }
        if self.static_time > 0.0 {
            self.draw_orbit(world, batch, lift, Element::Thunder.light(), 20.0, self.static_time);
        }
        if self.mirror_time > 0.0 {
            batch.draw(
                props::barrier_bubble(),
                snap(self.center() + lift - Vec2::new(14.0, 14.0)),
                Rgba::rgb(200, 240, 255).mul(0.8),
            );
        }
        if self.ward_time > 0.0 {
            self.draw_orbit(world, batch, lift, Element::Aether.light(), 18.0, self.ward_time);
        }
        if self.charging_skill() {
            self.draw_skill_charge(world, batch, lift);
        }

        let max = world.state.dodge_charges();
        if self.dodge_charges < max as f32 && !self.is_dead() {
            for i in 0..max {
                let fill = (self.dodge_charges - i as f32).clamp(0.0, 1.0);
                let at = snap(self.base.pos + Vec2::new((i as f32 - (max - 1) as f32 / 2.0) * 5.0 - 1.0, 3.0));
                batch.fill_rect(at.x, at.y, 3.0, 2.0, palette::OUTLINE.mul(0.7));
                if fill >= 1.0 {
                    let color = if self.dodge_lockout > 0.0 { palette::LIFE } else { Rgba::rgb(150, 230, 255) };
                    batch.fill_rect(at.x, at.y, 3.0, 2.0, color);
                }
            }
        }
    }
}
