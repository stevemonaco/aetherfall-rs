//! A data-driven enemy. It wanders until it notices a target, then closes to a useful range and picks one of its
//! attacks: a visible windup (flash, ground marker), the attack itself, and a recovery window. Armoured enemies have a
//! break gauge; everything takes elemental ailments. Bosses are enemies with a [`BossCore`] that replaces thinking,
//! dying and drawing.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use bevy::math::Vec2;

use super::boss::{self, BossCore};
use super::combat_effects::{Beam, ClawEffect, Exclaim, GroundMarker, LightningArc, LineMarker, Shockwave};
use super::effects::{DamageNumber, FloatingText, Particle, Puff};
use super::entity::{Entity, EntityBase, EntityId, TetherResponse};
use super::hazards::GasCloud;
use super::obstacles::{ObstacleKind, TileObstacle};
use super::projectile::Projectile;
use crate::art::{combat as art, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::{Box2, rotate, safe_dir, safe_normalize, snap};
use crate::core::gfx::{self, Batch, FrameStrip};
use crate::core::noise;
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{self, CombatStats, DamageResult, Faction, Hit, HitKind};
use crate::gameplay::element::{BASIC, Element};
use crate::gameplay::game_data::{AttackDef, EnemyDef, data};
use crate::gameplay::mastery;
use crate::gameplay::pacts;
use crate::gameplay::status::{Ailment, StatusSet};
use crate::scenes::world::{KillInfo, World};
use crate::world::map::TILE;

/// Melee hits a shield takes before breaking; heavy hits count twice and plain bolts not at all.
const SHIELD_HP: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Idle,
    Chase,
    Windup,
    Act,
    Recover,
    Stagger,
    Broken,
    Reeled,
    Burrowed,
}

pub struct Enemy {
    pub base: EntityBase,
    pub def: &'static EnemyDef,
    pub level: i32,
    pub max_hp: i32,
    pub hp: i32,
    pub guard_max: f32,
    pub guard_value: f32,
    pub status: StatusSet,
    pub is_elite: bool,
    pub blighted: bool,
    pub z: f32,
    pub invulnerable: bool,
    pub boss: Option<Box<BossCore>>,
    cooldowns: HashMap<usize, f32>,
    home: Vec2,
    phase: Phase,
    attack: Option<usize>,
    move_dir: Vec2,
    locked_dir: Vec2,
    locked_target: Vec2,
    knock_velocity: Vec2,
    act_start: Vec2,
    phase_time: f32,
    think_timer: f32,
    knock_time: f32,
    flash: f32,
    anim_time: f32,
    spawn_fade: f32,
    contact_cooldown: f32,
    hp_bar_time: f32,
    sidestep: f32,
    hop_time: f32,
    act_done: bool,
    wall_slam: bool,
    shield: bool,
    shield_hp: f32,
    enraged: bool,
    aura: bool,
    marked: f32,
    revived: bool,
    last_skill: bool,
    last_element: Element,
    last_weak: bool,
    blight_mark: Element,
    aura_element: Element,
    facing_right: bool,
    alerted: bool,
    strip: FrameStrip,
}

impl Enemy {
    pub fn new(def: &'static EnemyDef, pos: Vec2, level: Option<i32>) -> Enemy {
        let level = level.unwrap_or(def.level);
        let max_hp = ((combat::enemy_hp(level) * def.hp) as i32).max(1);
        let aura = def.trait_value("aura");
        let strip = FrameStrip::new(gfx::external(&format!("external/Monsters/{}.png", def.sprite)), 16, 16);
        Enemy {
            base: EntityBase { pos, width: def.width(), height: def.height(), ..EntityBase::default() },
            def,
            level,
            max_hp,
            hp: max_hp,
            guard_max: def.guard,
            guard_value: def.guard,
            status: StatusSet::default(),
            is_elite: false,
            blighted: false,
            z: 0.0,
            invulnerable: false,
            boss: None,
            cooldowns: def.attacks.iter().enumerate().map(|(i, a)| (i, a.cooldown * 0.5)).collect(),
            home: pos,
            phase: Phase::Idle,
            attack: None,
            move_dir: Vec2::ZERO,
            locked_dir: Vec2::ZERO,
            locked_target: Vec2::ZERO,
            knock_velocity: Vec2::ZERO,
            act_start: Vec2::ZERO,
            phase_time: 0.0,
            think_timer: 0.0,
            knock_time: 0.0,
            flash: 0.0,
            anim_time: 0.0,
            spawn_fade: 0.4,
            contact_cooldown: 0.0,
            hp_bar_time: 0.0,
            sidestep: 0.0,
            hop_time: 0.0,
            act_done: false,
            wall_slam: false,
            shield: def.has_trait("shield"),
            shield_hp: SHIELD_HP,
            enraged: false,
            aura: aura.is_some(),
            marked: 0.0,
            revived: false,
            last_skill: false,
            last_element: Element::None,
            last_weak: false,
            blight_mark: Element::None,
            aura_element: Element::parse(aura),
            facing_right: false,
            alerted: false,
            strip,
        }
    }

    /// A rift elite: four times the life, a break gauge and an elemental aura that only its opposite strips.
    pub fn elite(def: &'static EnemyDef, at: Vec2, level: Option<i32>, world: &World) -> Enemy {
        let mut enemy = Enemy::new(def, at, Some(level.unwrap_or(def.level) + 2));
        enemy.max_hp *= 4;
        enemy.hp = enemy.max_hp;
        enemy.guard_max = (def.guard * 1.5).max(8.0);
        enemy.guard_value = enemy.guard_max;
        if !enemy.aura {
            enemy.aura = true;
            let weakest = strongest_resist(def);
            enemy.aura_element =
                if def.resist_to(weakest) > 1.0 { weakest.opposite() } else { BASIC[world.rng.below(4) as usize] };
        }
        enemy.is_elite = true;
        enemy
    }

    pub fn heavy(&self) -> bool {
        self.boss.is_some() || self.def.heavy
    }

    pub fn is_flying(&self) -> bool {
        self.def.flying && !self.frozen()
    }

    pub fn frozen(&self) -> bool {
        self.status.frozen > 0.0
    }

    pub fn is_broken(&self) -> bool {
        self.phase == Phase::Broken
    }

    pub fn targetable(&self) -> bool {
        !self.base.removed && self.hp > 0 && self.phase != Phase::Burrowed && self.spawn_fade <= 0.0
    }

    pub fn stats(&self) -> CombatStats {
        CombatStats::new(
            self.level,
            combat::enemy_atk(self.level) * self.def.attack,
            combat::enemy_def(self.level) * self.def.defense,
            combat::enemy_foc(self.level),
        )
    }

    pub fn flashing(&self) -> bool {
        self.flash > 0.0
    }

    fn deals_contact_damage(&self) -> bool {
        if let Some(boss) = &self.boss {
            return !boss.dying() && boss.tangible() && self.def.contact > 0.0;
        }
        self.def.contact > 0.0 && !self.frozen() && !matches!(self.phase, Phase::Broken | Phase::Burrowed)
    }

    fn time_scale(world: &World) -> f32 {
        world.enemy_time_scale()
    }

    fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_time = 0.0;
    }

    fn move_speed(&self) -> f32 {
        self.def.speed * self.status.speed_factor() * if self.enraged { 1.35 } else { 1.0 }
    }

    pub fn move_by(&mut self, world: &World, delta: Vec2, flying: bool) -> Vec2 {
        world.move_entity(&mut self.base, delta, flying, false, false)
    }

    fn update_status(&mut self, world: &mut World, dt: f32) {
        let (burn, pulse) = self.status.update(dt);
        if burn > 0.0 {
            let amount = ((burn * self.def.resist_to(Element::Fire)) as i32).max(1);
            self.damage(world, amount, Element::Fire, false, "");
            if self.def.has_trait("flammable") {
                let (tx, ty) = self.base.tile_pos();
                world.chem_ignite(tx, ty);
            }
        }
        if pulse {
            self.interrupt(0.2);
            Particle::burst(world, self.hurtbox().center(), Element::Thunder.glow(), 5, 30.0, 0.2, true);
            let chain = world.state.mods().has("chain_shock");
            let source = self.status.burn_source;
            let me = self.hurtbox().center();
            for id in world.ids_of::<Enemy>() {
                let Some(other) = world.get_as::<Enemy>(id) else { continue };
                let wet = other.status.wet() || other.def.has_trait("metal");
                if other.base.pos.distance(self.base.pos) > 44.0 || !(chain || wet) {
                    continue;
                }
                let target = other.hurtbox().center();
                world.add(LightningArc::new(me, target));
                let mut attacker = source;
                attacker.atk = attacker.atk.max(world.state.atk(Element::Thunder));
                let mut hit = Hit {
                    power: 0.3,
                    element: Element::Thunder,
                    kind: HitKind::Status,
                    status_power: 0.8,
                    from: me,
                    knockback: 20.0,
                    ..Hit::new(Faction::Player, attacker)
                };
                world.hit(id, &mut hit);
            }
        }
    }

    // ---- Thinking -----------------------------------------------------------------------------------------------

    fn think(&mut self, world: &mut World, dt: f32) {
        if self.boss.is_some() {
            self.boss_think(world, dt);
            return;
        }
        self.phase_time += dt;
        let target = world.enemy_target(self.base.pos);
        if let Some(t) = target {
            let to = t - self.base.pos;
            if !self.alerted && to.length() < self.def.sight {
                self.alert(world);
            }
            if self.alerted
                && (self.base.pos.distance(self.home) > self.def.leash * 1.6 || to.length() > self.def.sight * 2.5)
            {
                self.alerted = false;
            }
        } else {
            self.alerted = false;
        }

        match self.phase {
            Phase::Idle | Phase::Chase => {
                if let (true, Some(t)) = (self.alerted, target) {
                    self.chase(world, dt, t);
                } else {
                    self.wander(world, dt);
                }
            }
            Phase::Windup => self.update_windup(world, target),
            Phase::Act => self.update_act(world, dt),
            Phase::Recover => {
                let recover = self.attack.map(|i| self.def.attacks[i].recover).unwrap_or(0.4);
                if self.phase_time >= recover {
                    self.set_phase(Phase::Chase);
                }
            }
            Phase::Stagger => {
                if self.phase_time >= 0.35 {
                    self.set_phase(Phase::Chase);
                }
            }
            Phase::Broken => {
                if self.phase_time >= 3.0 {
                    self.guard_value = self.guard_max;
                    self.set_phase(Phase::Chase);
                }
            }
            Phase::Reeled => {
                let pull = world.hero.pos - self.base.pos;
                if pull.length() > 14.0 {
                    let flying = self.is_flying();
                    self.move_by(world, pull.normalize() * 260.0 * dt, flying);
                }
                if self.phase_time > 0.6 {
                    self.end_reel();
                }
            }
            Phase::Burrowed => self.update_burrow(world, dt, target),
        }
    }

    fn alert(&mut self, world: &mut World) {
        self.alerted = true;
        if self.phase == Phase::Idle {
            self.set_phase(Phase::Chase);
            world.add(Exclaim::new(self.base.id, self.base.pos, self.base.height));
        }
    }

    fn wander(&mut self, world: &mut World, dt: f32) {
        self.think_timer -= dt;
        if self.def.movement == "static" {
            return;
        }
        if self.think_timer <= 0.0 {
            self.move_dir = if world.rng.next_f32() < 0.35 { Vec2::ZERO } else { random_vector(world) };
            if self.base.pos.distance(self.home) > 48.0 {
                self.move_dir = safe_normalize(self.home - self.base.pos);
            }
            self.think_timer = 0.8 + world.rng.next_f32() * 1.4;
        }
        let speed = self.move_speed() * 0.5;
        self.locomote(world, dt, self.move_dir, speed);
    }

    fn chase(&mut self, world: &mut World, dt: f32, target: Vec2) {
        let to = target - self.base.pos;
        let distance = to.length();

        let mut order: Vec<usize> = (0..self.def.attacks.len()).collect();
        world.rng.shuffle(&mut order);
        for i in order {
            let attack = &self.def.attacks[i];
            if self.cooldowns[&i] > 0.0 || distance > attack.range || distance < attack.min_range {
                continue;
            }
            if world.rng.next_f32() > attack.weight * dt * 6.0 {
                continue;
            }
            self.begin_attack(world, i, target);
            return;
        }

        if self.def.movement == "burrow" && distance > 50.0 && world.rng.next_f32() < dt * 0.8 {
            self.set_phase(Phase::Burrowed);
            audio::play(Sound::Door, 0.5, -0.5);
            Particle::burst(world, self.base.pos, Rgba::rgb(150, 110, 70), 10, 40.0, 0.4, false);
            return;
        }

        let max_range = self.def.attacks.iter().map(|a| a.range).fold(0.0, f32::max);
        let preferred =
            if self.def.attacks.is_empty() { 0.0 } else { max_range * if self.ranged() { 0.75 } else { 0.5 } };
        let mut dir = safe_normalize(to);
        if self.sidestep > 0.0 {
            self.sidestep -= dt;
            dir = self.move_dir;
        } else if self.ranged() && distance < preferred * 0.6 {
            dir = -dir;
        } else if distance < preferred {
            let side = if noise::hash(self.base.pos.x as i32, 0, 5).is_multiple_of(2) { 1.0 } else { -1.0 };
            dir = Vec2::new(-dir.y, dir.x) * side * 0.6;
        }
        let speed = self.move_speed();
        self.locomote(world, dt, dir, speed);
    }

    fn ranged(&self) -> bool {
        !self.def.attacks.is_empty()
            && self
                .def
                .attacks
                .iter()
                .all(|a| matches!(a.kind.as_str(), "shoot" | "ring" | "beam" | "spores" | "summon"))
    }

    fn locomote(&mut self, world: &mut World, dt: f32, dir: Vec2, speed: f32) {
        if dir == Vec2::ZERO || self.def.movement == "static" {
            return;
        }
        if dir.x.abs() > 0.1 {
            self.facing_right = dir.x > 0.0;
        }
        match self.def.movement.as_str() {
            "hop" => {
                if self.hop_time > 0.0 {
                    self.hop_time -= dt;
                    self.z = ((1.0 - self.hop_time / 0.4) * PI).sin() * 7.0;
                    let delta = self.move_dir * speed * 3.0 * dt;
                    self.move_by(world, delta, false);
                    if self.hop_time <= 0.0 {
                        self.z = 0.0;
                    }
                } else {
                    self.think_timer -= dt;
                    if self.think_timer <= 0.0 {
                        self.move_dir = dir;
                        self.hop_time = 0.4;
                        self.think_timer = 0.4 + world.rng.next_f32() * 0.6;
                    }
                }
                return;
            }
            "fly" | "ghost" => self.z = 6.0 + (self.anim_time * 5.0).sin() * 2.0,
            _ => {}
        }
        self.move_dir = dir;
        let flying = self.is_flying();
        let moved = self.move_by(world, dir * speed * dt, flying);
        if moved.length_squared() < 0.0001 && self.sidestep <= 0.0 {
            let side = if world.rng.next_f32() < 0.5 { 1.0 } else { -1.0 };
            self.move_dir = Vec2::new(-dir.y, dir.x) * side;
            self.sidestep = 0.4;
        }
    }

    // ---- Attacks ------------------------------------------------------------------------------------------------

    fn attack_def(&self, i: usize) -> &'static AttackDef {
        &self.def.attacks[i]
    }

    fn begin_attack(&mut self, world: &mut World, i: usize, target: Vec2) {
        let attack = self.attack_def(i);
        self.attack = Some(i);
        self.act_done = false;
        self.locked_target = target;
        self.locked_dir = safe_normalize(target - self.base.pos);
        self.facing_right = self.locked_dir.x > 0.0;
        self.set_phase(Phase::Windup);
        let color = attack_color(attack);
        let mouth = self.base.pos - Vec2::new(0.0, 4.0);
        match attack.kind.as_str() {
            "slam" => {
                world.add(GroundMarker::new(self.base.pos, attack.radius, attack.windup, color));
            }
            "leap" => {
                world.add(GroundMarker::new(target, attack.radius, attack.windup + 0.5, color));
            }
            "charge" | "beam" => {
                let end = ray_end(world, mouth, self.locked_dir, attack.distance);
                let width = if attack.kind == "beam" { 8.0 } else { self.base.width };
                world.add(LineMarker::new(mouth, end, width, attack.windup, color));
            }
            "lunge" => {
                world.add(LineMarker::new(
                    mouth,
                    mouth + self.locked_dir * attack.distance,
                    self.base.width,
                    attack.windup,
                    color,
                ));
            }
            _ => {}
        }
    }

    fn windup_time(&self, world: &World, attack: &AttackDef) -> f32 {
        attack.windup * if pacts::active(&world.state, "haste") { 0.7 } else { 1.0 }
    }

    fn update_windup(&mut self, world: &mut World, target: Option<Vec2>) {
        let Some(i) = self.attack else {
            self.set_phase(Phase::Chase);
            return;
        };
        let attack = self.attack_def(i);
        let windup = self.windup_time(world, attack);
        // Ranged attacks keep tracking until the last moment; charges and lunges commit at the start.
        if let Some(t) = target
            && matches!(attack.kind.as_str(), "shoot" | "bite")
            && self.phase_time < windup - 0.15
        {
            self.locked_target = t;
            self.locked_dir = safe_normalize(t - self.base.pos);
        }
        if self.phase_time < windup {
            return;
        }
        self.cooldowns.insert(i, attack.cooldown);
        self.act_start = self.base.pos;
        self.set_phase(Phase::Act);
    }

    fn update_act(&mut self, world: &mut World, dt: f32) {
        let Some(i) = self.attack else {
            self.set_phase(Phase::Recover);
            return;
        };
        let attack = self.attack_def(i);
        match attack.kind.as_str() {
            "lunge" => {
                let duration = 0.22;
                let flying = self.is_flying();
                let delta = self.locked_dir * (attack.distance / duration) * attack.speed * dt;
                self.move_by(world, delta, flying);
                if !self.act_done && self.hit_player(world, self.hurtbox().inflate(3.0), attack) {
                    self.act_done = true;
                }
                if self.phase_time >= duration {
                    self.set_phase(Phase::Recover);
                }
            }
            "charge" => {
                let flying = self.is_flying();
                let delta = self.locked_dir * self.def.speed * 4.5 * attack.speed * dt;
                let moved = self.move_by(world, delta, flying);
                if !self.act_done && self.hit_player(world, self.hurtbox().inflate(3.0), attack) {
                    self.act_done = true;
                }
                if moved.length_squared() < 0.01 && self.phase_time > 0.05 {
                    world.camera.shake(0.15, 2.0);
                    audio::play(Sound::Break, 0.6, -0.3);
                    self.break_for(1.6);
                } else if self.act_start.distance(self.base.pos) >= attack.distance || self.phase_time > 2.0 {
                    self.set_phase(Phase::Recover);
                }
            }
            "leap" => {
                let duration = 0.5;
                let t = (self.phase_time / duration).min(1.0);
                self.base.pos = self.act_start.lerp(self.locked_target, t);
                self.z = (t * PI).sin() * 26.0;
                if t >= 1.0 {
                    self.z = 0.0;
                    let at = self.base.pos;
                    self.slam(world, attack, at);
                    self.set_phase(Phase::Recover);
                }
            }
            _ => {
                if !self.act_done {
                    self.act_done = true;
                    self.perform(world, attack);
                }
                if self.phase_time >= 0.15 {
                    self.set_phase(Phase::Recover);
                }
            }
        }
    }

    fn perform(&mut self, world: &mut World, attack: &'static AttackDef) {
        let from = self.base.pos - Vec2::new(0.0, self.base.height * self.def.scale / 2.0);
        let stats = self.stats();
        match attack.kind.as_str() {
            "bite" => {
                let mouth = self.base.pos - Vec2::new(0.0, 6.0);
                world.add(ClawEffect::new(mouth + self.locked_dir * 10.0, self.locked_dir, attack_color(attack)));
                let size = attack.radius * 1.2;
                let reach = Box2::from_center(mouth + self.locked_dir * (attack.radius * 0.6), size, size);
                self.hit_player(world, reach, attack);
                audio::play(Sound::Swing, 0.7, -0.3);
            }
            "shoot" => {
                for i in 0..attack.count {
                    let angle = if attack.count == 1 {
                        0.0
                    } else {
                        (i as f32 - (attack.count - 1) as f32 / 2.0) * attack.spread
                    };
                    let dir = rotate(self.locked_dir, angle);
                    world.add(Projectile::enemy_shot(&attack.projectile, from, dir, stats, attack.power, attack.speed));
                }
                let sound = if attack.projectile == "fire" { Sound::ShootFire } else { Sound::ShootWind };
                audio::play(sound, 0.5, 0.3);
            }
            "ring" => {
                let offset = world.rng.next_f32() * TAU;
                for i in 0..attack.count {
                    let a = offset + i as f32 * TAU / attack.count as f32;
                    world.add(Projectile::enemy_shot(
                        &attack.projectile,
                        from,
                        Vec2::new(a.cos(), a.sin()),
                        stats,
                        attack.power,
                        attack.speed,
                    ));
                }
                audio::play(Sound::ShootFire, 0.6, 0.2);
            }
            "slam" => {
                let at = self.base.pos;
                self.slam(world, attack, at);
            }
            "spores" => {
                for _ in 0..attack.count {
                    let at = self.base.pos + Vec2::new(world.rng.range(-8, 9) as f32, world.rng.range(-4, 5) as f32);
                    let drift = rotate(self.locked_dir, (world.rng.next_f32() - 0.5) * 0.8) * 30.0 * attack.speed;
                    world.add(GasCloud::new(at, stats, drift));
                }
                audio::play(Sound::Burn, 0.5, 0.4);
            }
            "summon" => {
                if let Some(minion) = attack.summon.as_ref().and_then(|s| data().enemies.get(s)) {
                    let alive = world.entities_of::<Enemy>().filter(|e| e.def.id == minion.id).count();
                    if alive < 4 {
                        for _ in 0..attack.count {
                            let at = self.base.pos
                                + Vec2::new(world.rng.range(-20, 21) as f32, world.rng.range(-8, 16) as f32);
                            world.add(Enemy::new(minion, at, Some(self.level)));
                        }
                    }
                }
                audio::play(Sound::BossRoar, 0.4, 0.4);
            }
            "beam" => {
                let start = self.base.pos - Vec2::new(0.0, 4.0);
                let end = ray_end(world, start, self.locked_dir, attack.distance);
                world.add(Beam::new(
                    self.base.pos - Vec2::new(0.0, 6.0),
                    end - Vec2::new(0.0, 2.0),
                    attack_color(attack),
                ));
                let player = world.hero.pos;
                let along = (player - start).dot(self.locked_dir).clamp(0.0, start.distance(end));
                if (start + self.locked_dir * along).distance(player - Vec2::new(0.0, 4.0)) < 10.0 {
                    let mut hit = self.make_attack_hit(attack, start + self.locked_dir * (along - 10.0).max(0.0));
                    self.strike_player(world, &mut hit);
                }
                audio::play(Sound::ChargeLevel, 0.8, -0.5);
            }
            _ => {}
        }
    }

    fn slam(&mut self, world: &mut World, attack: &AttackDef, at: Vec2) {
        world.add(Shockwave::new(at, attack.radius, attack_color(attack)));
        world.camera.shake(0.12, 1.0);
        audio::play(Sound::Break, 0.6, 0.2);
        if !world.hero.airborne && world.hero.pos.distance(at) < attack.radius + 4.0 {
            let mut hit = self.make_attack_hit(attack, at);
            self.strike_player(world, &mut hit);
        }
        if attack.element.is_some() {
            let element = Element::parse(attack.element.as_deref());
            let (tx, ty) = self.base.tile_pos();
            for y in -1..=1 {
                for x in -1..=1 {
                    world.chem_apply(tx + x, ty + y, element, false);
                }
            }
        }
    }

    fn hit_player(&mut self, world: &mut World, area: Box2, attack: &AttackDef) -> bool {
        if !world.hero.hurtbox.intersects(&area) {
            return false;
        }
        let mut hit = self.make_attack_hit(attack, self.base.hitbox().center());
        self.strike_player(world, &mut hit);
        true
    }

    /// Hits the player and reacts if they perfectly guarded it.
    pub fn strike_player(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        let connected = world.hit_player(hit);
        if hit.parried {
            self.parried(world);
        }
        connected
    }

    fn make_attack_hit(&self, attack: &AttackDef, from: Vec2) -> Hit {
        Hit {
            power: attack.power,
            element: Element::parse(attack.element.as_deref()),
            kind: HitKind::Attack,
            from,
            owner: Some(self.base.id),
            unblockable: attack.unblockable,
            knockback: 120.0 + attack.power * 40.0,
            ..Hit::new(Faction::Enemy, self.stats())
        }
    }

    fn touch_target(&mut self, world: &mut World) {
        if self.contact_cooldown > 0.0
            || self.hp <= 0
            || self.z >= 6.0
            || !world.hero.hurtbox.intersects(&self.hurtbox())
        {
            return;
        }
        self.contact_cooldown = 0.8;
        let mut hit = Hit {
            power: self.def.contact,
            from: self.base.hitbox().center(),
            owner: Some(self.base.id),
            kind: HitKind::Contact,
            knockback: 120.0,
            ..Hit::new(Faction::Enemy, self.stats())
        };
        self.strike_player(world, &mut hit);
    }

    fn update_burrow(&mut self, world: &mut World, dt: f32, target: Option<Vec2>) {
        let Some(t) = target else {
            self.surface(world);
            return;
        };
        let to = t - self.base.pos;
        if (self.phase_time > 0.4 && to.length() < 10.0) || self.phase_time > 3.0 {
            self.surface(world);
            if let Some(i) = self.def.attacks.iter().position(|a| a.kind == "slam") {
                let slam = self.attack_def(i);
                self.attack = Some(i);
                self.set_phase(Phase::Windup);
                world.add(GroundMarker::new(self.base.pos, slam.radius, slam.windup, attack_color(slam)));
            }
            return;
        }
        let delta = safe_normalize(to) * self.def.speed * 2.2 * dt;
        self.move_by(world, delta, false);
        if world.rng.next_f32() < 0.5 {
            let at = self.base.pos + Vec2::new(world.rng.range(-5, 6) as f32, 0.0);
            world.add(Particle::new(at, Vec2::new(0.0, -15.0), Rgba::rgb(140, 100, 60), 0.3));
        }
    }

    fn surface(&mut self, world: &mut World) {
        self.set_phase(Phase::Chase);
        Particle::burst(world, self.base.pos, Rgba::rgb(150, 110, 70), 14, 50.0, 0.4, false);
        audio::play(Sound::Break, 0.4, 0.4);
    }

    // ---- Taking hits --------------------------------------------------------------------------------------------

    fn take_hit_inner(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if self.hp <= 0
            || self.base.removed
            || hit.source == Faction::Enemy
            || self.spawn_fade > 0.2
            || self.phase == Phase::Burrowed
        {
            return false;
        }
        if let Some(result) = boss::with_brain(self, |brain, e| brain.before_hit(e, world, hit)).flatten() {
            return result;
        }
        if self.is_flying() && hit.kind == HitKind::Environment {
            return false;
        }

        let element = if hit.element == Element::Aether { self.best_element_against(world) } else { hit.element };
        self.alert(world);

        if self.aura && hit.kind != HitKind::Environment && element == self.aura_element.opposite() {
            self.aura = false;
            audio::play(Sound::Break, 0.8, 0.3);
            world.add(FloatingText::new(
                self.base.pos - Vec2::new(0.0, self.base.height + 14.0),
                "AURA BROKEN",
                element.glow(),
            ));
            self.break_for(2.0);
        }

        if self.shield
            && !matches!(hit.kind, HitKind::Environment | HitKind::Status)
            && self.phase != Phase::Broken
            && self.front_hit(world, hit.from)
        {
            let bane = self.shield_bane();
            let shattered = element == bane && bane != Element::None && hit.kind != HitKind::Tether;
            self.shield_hp -= match hit.kind {
                _ if shattered => SHIELD_HP,
                HitKind::Bolt | HitKind::Tether => 0.0,
                HitKind::Finisher | HitKind::ChargedBolt | HitKind::Skill => 2.0,
                _ => 1.0,
            };
            let at = self.hurtbox().center() + safe_dir(hit.from - self.base.pos, Vec2::Y) * 6.0;
            hit.result = DamageResult::blocked(false);
            if self.shield_hp > 0.0 {
                audio::play(Sound::Clink, 0.8, 0.0);
                Particle::burst(world, at, Rgba::WHITE, 5, 40.0, 0.2, true);
                self.knockback(hit.from, 50.0, 0.06);
                return true;
            }
            self.shield = false;
            audio::sfx(Sound::Break);
            let label = self.base.pos - Vec2::new(0.0, self.base.height + 14.0);
            if shattered {
                Particle::burst(world, at, bane.glow(), 14, 70.0, 0.4, true);
                world.add(FloatingText::new(label, "SHIELD SHATTERED", bane.glow()));
                self.break_for(3.0);
            } else {
                Particle::burst(world, at, Rgba::WHITE, 14, 70.0, 0.4, true);
                world.add(FloatingText::new(label, "SHIELD BROKEN", palette::HIGHLIGHT));
                self.interrupt(0.4);
            }
            return true;
        }

        if self.invulnerable {
            audio::sfx(Sound::Clink);
            Particle::burst(world, self.hurtbox().center(), Rgba::WHITE, 4, 50.0, 0.2, true);
            return true;
        }

        let element_mod = self.def.resist_to(element);
        let mut mods = if self.phase == Phase::Broken { 2.0 } else { 1.0 }
            * if self.aura { 0.3 } else { 1.0 }
            * if self.marked > 0.0 { 1.25 } else { 1.0 };
        let mut shatter = false;
        if self.frozen() && hit.power >= 0.5 && self.status.shatter() {
            shatter = true;
            mods *= if hit.kind == HitKind::Finisher { 2.0 } else { 1.5 };
        }
        let mut defender = self.stats();
        defender.def *= self.status.defense_factor();
        let mut probe = hit.clone();
        probe.element = element;
        let resolved = combat::resolve(&probe, defender, element_mod, mods, &world.rng);
        hit.result = resolved;
        self.hp_bar_time = 2.5;

        if element_mod <= 0.0 {
            audio::sfx(Sound::Clink);
            world.add(FloatingText::new(
                self.base.pos - Vec2::new(0.0, self.base.height + 8.0),
                "Immune",
                palette::TEXT_DIM,
            ));
            return true;
        }

        self.last_element = element;
        self.last_weak = hit.is_player() && element_mod > 1.0;
        self.last_skill = hit.kind == HitKind::Skill;
        self.damage(world, resolved.amount, element, resolved.crit, if element_mod > 1.0 { "!" } else { "" });
        self.flash = 0.12;
        audio::play(Sound::Hit, 1.0, if resolved.crit { -0.2 } else { 0.0 });
        world.hit_stop(if self.hp <= 0 {
            0.07
        } else if resolved.crit {
            0.05
        } else {
            0.03
        });
        let color = if element == Element::None { Rgba::WHITE } else { element.glow() };
        Particle::burst(world, self.hurtbox().center(), color, if resolved.crit { 10 } else { 5 }, 60.0, 0.25, true);
        if shatter {
            world.add(FloatingText::new(
                self.base.pos - Vec2::new(0.0, self.base.height + 14.0),
                "SHATTER",
                Rgba::rgb(200, 240, 255),
            ));
            Particle::burst(world, self.hurtbox().center(), Rgba::rgb(200, 240, 255), 18, 90.0, 0.5, true);
            audio::play(Sound::Break, 0.8, 0.5);
        }
        if self.hp <= 0 {
            self.die(world);
            return true;
        }

        self.apply_ailments(world, hit, element, element_mod);
        self.apply_poise(world, hit, element_mod);

        let flinch = !self.heavy() || self.phase == Phase::Broken;
        if flinch && hit.knockback > 0.0 && !self.frozen() {
            self.interrupt(0.3);
            let speed = hit.knockback * if self.heavy() { 0.4 } else { 1.0 };
            self.knockback(hit.from, speed, 0.12);
        }
        true
    }

    fn apply_ailments(&mut self, world: &mut World, hit: &Hit, element: Element, element_mod: f32) {
        let resonance = if hit.is_player() && world.state.resonates(element) { 1.3 } else { 1.0 };
        let mut trait_mod = match element {
            Element::Fire if self.def.has_trait("flammable") => 1.5,
            Element::Thunder if self.def.has_trait("metal") => 1.5,
            _ => 1.0,
        };
        if hit.is_player() {
            let soul = match (world.state.soul.as_deref(), element) {
                (Some("ember"), Element::Fire) => 1.2,
                (Some("stormling"), Element::Thunder) => 1.3,
                _ => 1.0,
            };
            trait_mod *= world.state.mods().buildup_for(element) * soul;
        }
        let buildup = 34.0
            * hit.status_power
            * (1.0 + hit.attacker.foc / 120.0)
            * element_mod.clamp(0.25, 1.5)
            * resonance
            * trait_mod;
        let was_burning = self.status.burn > 0.0;
        let can_freeze = !self.heavy() && self.boss.is_none();
        let ailment = self.status.add(element, buildup, hit.attacker, can_freeze);
        match ailment {
            Ailment::None => return,
            Ailment::Burn => {
                let (tx, ty) = self.base.tile_pos();
                world.chem_ignite(tx, ty);
            }
            Ailment::Gust => {
                self.interrupt(0.5);
                self.wall_slam = true;
                self.knockback(hit.from, if self.heavy() { 140.0 } else { 280.0 }, 0.25);
                if was_burning {
                    let me = self.base.pos;
                    for id in world.ids_of::<Enemy>() {
                        world.with_as::<Enemy, _>(id, |e, _| {
                            if e.base.pos.distance(me) < 44.0 {
                                e.status.apply(Element::Fire, hit.attacker, true);
                            }
                        });
                    }
                }
            }
            Ailment::Shock => self.interrupt(0.4),
            Ailment::Conduct => {
                self.interrupt(1.2);
                self.damage(world, (hit.result.amount / 2).max(1), Element::Thunder, false, " CONDUCT");
                Particle::burst(world, self.hurtbox().center(), Element::Thunder.glow(), 16, 70.0, 0.4, true);
            }
            Ailment::Chill if world.state.mods().has("deep_chill") => self.status.extend_chill(3.0),
            Ailment::Freeze => {
                self.interrupt(0.1);
                audio::play(Sound::Clink, 0.9, 0.7);
            }
            _ => {}
        }
        let at = self.base.pos - Vec2::new(0.0, self.base.height * self.def.scale + 18.0);
        world.add(FloatingText::new(at, ailment.label(), element.glow()));
    }

    fn apply_poise(&mut self, world: &mut World, hit: &Hit, element_mod: f32) {
        if self.guard_max <= 0.0 || self.phase == Phase::Broken {
            return;
        }
        self.guard_value -= hit.poise * if element_mod > 1.0 { 2.0 } else { 1.0 };
        if self.guard_value > 0.0 {
            return;
        }
        self.guard_value = 0.0;
        self.break_for(3.0);
        let at = self.base.pos - Vec2::new(0.0, self.base.height * self.def.scale + 18.0);
        world.add(FloatingText::new(at, "BREAK!", palette::HIGHLIGHT));
        audio::play(Sound::Break, 0.9, 0.2);
        world.hit_stop(0.08);
    }

    pub fn break_for(&mut self, seconds: f32) {
        self.set_phase(Phase::Broken);
        self.phase_time = 3.0 - seconds;
        self.attack = None;
    }

    /// The player perfectly guarded this enemy's attack.
    pub fn parried(&mut self, world: &mut World) {
        if self.guard_max > 0.0 {
            self.guard_value -= 6.0;
            if self.guard_value <= 0.0 {
                self.guard_value = 0.0;
                self.break_for(3.0);
                let at = self.base.pos - Vec2::new(0.0, self.base.height * self.def.scale + 18.0);
                world.add(FloatingText::new(at, "BREAK!", palette::HIGHLIGHT));
                return;
            }
        }
        if !self.heavy() || self.boss.is_none() {
            self.set_phase(Phase::Stagger);
            self.phase_time = -0.5;
        }
    }

    fn interrupt(&mut self, seconds: f32) {
        if matches!(self.phase, Phase::Broken | Phase::Burrowed) || self.boss.is_some() {
            return;
        }
        if self.heavy() && matches!(self.phase, Phase::Act | Phase::Windup) && seconds < 1.0 {
            return;
        }
        self.set_phase(Phase::Stagger);
        self.phase_time = 0.35 - seconds;
        self.hop_time = 0.0;
        if !self.is_flying() {
            self.z = 0.0;
        }
    }

    fn wall_slam_hit(&mut self, world: &mut World) {
        self.wall_slam = false;
        self.knock_time = 0.0;
        let doubled = if world.state.mods().has("gust_slam") { 2 } else { 1 };
        let amount = (self.max_hp / 12).max(1) * doubled;
        self.damage(world, amount, Element::Wind, false, " SLAM");
        world.camera.shake(0.12, 2.0);
        audio::play(Sound::Break, 0.6, 0.4);
        let me = self.hurtbox();
        let stats = self.stats();
        let rocks: Vec<EntityId> = world
            .entities_of::<TileObstacle>()
            .filter(|r| r.kind == ObstacleKind::Rock && r.hurtbox().inflate(6.0).intersects(&me))
            .map(|r| r.base().id)
            .collect();
        for id in rocks {
            let mut hit =
                Hit { power: 1.0, kind: HitKind::Environment, from: self.base.pos, ..Hit::new(Faction::Player, stats) };
            world.hit(id, &mut hit);
        }
    }

    fn front_hit(&self, world: &World, from: Vec2) -> bool {
        let facing = safe_normalize(world.hero.pos - self.base.pos);
        safe_normalize(from - self.base.pos).dot(facing) > 0.3
    }

    /// The element this enemy is weakest to, which also shatters its shield in one hit.
    fn shield_bane(&self) -> Element {
        BASIC
            .into_iter()
            .filter(|&e| self.def.resist_to(e) > 1.0)
            .max_by(|&a, &b| self.def.resist_to(a).total_cmp(&self.def.resist_to(b)))
            .unwrap_or(Element::None)
    }

    /// The element an Aether strike takes: this foe's weakest among the elements mastered well enough, else neutral.
    fn best_element_against(&self, world: &World) -> Element {
        let mut best = Element::None;
        let mut best_mod = -1.0;
        let lens = world.state.prism("prism_lens");
        for e in BASIC.into_iter().filter(|&e| lens || world.state.mastery_level(e) >= mastery::ADEPT) {
            let m = self.def.resist_to(e);
            if m > best_mod || (m == best_mod && world.rng.next_f32() < 0.5) {
                best = e;
                best_mod = m;
            }
        }
        best
    }

    pub fn damage(&mut self, world: &mut World, amount: i32, element: Element, crit: bool, suffix: &str) {
        self.hp -= amount;
        self.hp_bar_time = 2.5;
        let at = self.base.pos - Vec2::new(0.0, self.base.height * self.def.scale + 6.0 + self.z);
        world.add(DamageNumber::styled(at, amount, element, crit, false, false, suffix));
        if self.hp <= 0 && !self.base.removed {
            self.die(world);
        }
    }

    fn knockback(&mut self, from: Vec2, speed: f32, time: f32) {
        if self.boss.is_some() {
            return;
        }
        let away = self.base.hitbox().center() - from;
        self.knock_velocity = safe_normalize(away) * speed;
        self.knock_time = time;
    }

    fn revive(&mut self, world: &mut World) {
        self.revived = true;
        self.hp = (self.max_hp * 2 / 5).max(1);
        self.status.clear();
        self.set_phase(Phase::Stagger);
        self.phase_time = -0.9;
        audio::play(Sound::BossRoar, 0.5, 0.6);
        Particle::burst(world, self.hurtbox().center(), Rgba::rgb(170, 90, 220), 16, 60.0, 0.6, false);
        let at = self.base.pos - Vec2::new(0.0, self.base.height * self.def.scale + 18.0);
        world.add(FloatingText::new(at, "RISES AGAIN", Rgba::rgb(210, 150, 255)));
    }

    /// The hawk's mark: this enemy takes extra damage for a while.
    pub fn mark(&mut self, seconds: f32) {
        self.marked = self.marked.max(seconds);
    }

    /// Stuns the enemy (not bosses) for a moment.
    pub fn stun(&mut self, seconds: f32) {
        if self.boss.is_some() || self.hp <= 0 {
            return;
        }
        self.set_phase(Phase::Stagger);
        self.phase_time = 0.35 - seconds;
    }

    pub fn end_reel(&mut self) {
        if self.phase != Phase::Reeled {
            return;
        }
        self.set_phase(Phase::Stagger);
        self.phase_time = -0.3;
    }

    pub fn die(&mut self, world: &mut World) {
        if self.base.removed {
            return;
        }
        if self.boss.is_some() {
            if !boss::with_brain(self, |brain, e| brain.on_die(e, world)).unwrap_or(false) {
                self.boss_die(world);
            }
            return;
        }
        if self.blighted && !self.revived && !self.last_skill && self.last_element != self.blight_mark {
            self.revive(world);
            return;
        }
        self.base.removed = true;
        audio::sfx(Sound::EnemyDie);
        world.add(Puff::new(self.base.pos - Vec2::new(0.0, 6.0), Rgba::WHITE));
        Particle::burst(world, self.base.pos - Vec2::new(0.0, 6.0), Rgba::rgb(255, 240, 200), 10, 60.0, 0.4, false);
        if self.status.burn > 0.0 && world.state.mods().has("burn_spread") {
            let (me, source) = (self.base.pos, self.status.burn_source);
            for id in world.ids_of::<Enemy>() {
                world.with_as::<Enemy, _>(id, |e, _| {
                    if e.base.pos.distance(me) < 48.0 {
                        e.status.apply(Element::Fire, source, true);
                    }
                });
            }
        }
        if let Some(split) = self.def.trait_value("split") {
            let parts: Vec<&str> = split.split(':').collect();
            if let Some(child) = data().enemies.get(parts[0]) {
                let count = parts.get(1).and_then(|c| c.parse().ok()).unwrap_or(2);
                for i in 0..count {
                    let offset = Vec2::new((i as f32 - (count - 1) as f32 / 2.0) * 10.0, 0.0);
                    world.add(Enemy::new(child, self.base.pos + offset, Some(self.level)));
                }
            }
        }
        world.reward_kill(self.kill_info());
    }

    pub fn kill_info(&self) -> KillInfo {
        KillInfo {
            def: self.def,
            level: self.level,
            pos: self.base.pos,
            boss: self.boss.is_some(),
            blighted: self.blighted,
            element: self.last_element,
            weakness: self.last_weak,
        }
    }

    // ---- Bosses -------------------------------------------------------------------------------------------------

    fn boss_think(&mut self, world: &mut World, dt: f32) {
        let mut boss = self.boss.take().expect("boss");
        boss.time += dt;
        if !boss.dying() {
            self.invulnerable = !boss.tangible();
            let step = if boss.echo { dt * 1.25 } else { dt };
            self.boss = Some(boss);
            boss::with_brain(self, |brain, e| brain.think(e, world, step));
            return;
        }
        boss.dying_time += dt;
        if world.rng.next_f32() < 0.3 {
            let at =
                self.hurtbox().center() + Vec2::new(world.rng.range(-14, 15) as f32, world.rng.range(-14, 15) as f32);
            world.add(Puff::new(at, boss.death_color()));
            audio::play(Sound::Hit, 0.5, world.rng.next_f32() * 0.5 - 0.25);
        }
        let finished = boss.dying_time >= boss.death_duration();
        self.boss = Some(boss);
        if finished {
            self.boss_finish(world);
        }
    }

    fn boss_die(&mut self, world: &mut World) {
        let Some(boss) = self.boss.as_mut() else { return };
        if boss.dying() {
            return;
        }
        self.hp = 0;
        boss.dying_time = 0.0;
        let duration = boss.death_duration();
        self.invulnerable = true;
        world.camera.shake(duration, 2.0);
        world.slow_motion(0.8, 0.3);
        audio::sfx(Sound::BossDie);
        let me = self.base.id;
        for id in world.ids_of::<Enemy>() {
            if id != me {
                world.with_entity(id, |e, _| e.base_mut().removed = true);
            }
        }
        for id in world.ids_of::<Projectile>() {
            world.with_as::<Projectile, _>(id, |p, _| {
                if p.owner == Faction::Enemy {
                    p.base.removed = true;
                }
            });
        }
    }

    fn boss_finish(&mut self, world: &mut World) {
        let Some(boss) = self.boss.as_ref() else { return };
        let (flag, actions, color) = (boss.flag.clone(), boss.defeat_actions.clone(), boss.death_color());
        self.base.removed = true;
        world.boss = None;
        world.state.flags.insert(flag.clone());
        Particle::burst(world, self.hurtbox().center(), color, 40, 120.0, 0.9, true);
        world.reward_kill(self.kill_info());
        world.on_boss_defeated(&flag);
        world.run_actions(&actions);
    }

    /// Damage dealt to a part also wears down the boss itself.
    pub fn bleed_from_part(&mut self, world: &mut World, amount: i32) {
        if self.boss.as_ref().is_some_and(|b| b.dying()) || amount <= 0 {
            return;
        }
        self.hp -= amount;
        if self.hp <= 0 {
            self.die(world);
        }
    }

    pub fn health_fraction(&self) -> f32 {
        let own = || self.raw_health_fraction();
        self.boss.as_ref().and_then(|b| b.brain()).and_then(|b| b.health_fraction(self)).unwrap_or_else(own)
    }

    /// Life left as a fraction, ignoring any boss-specific measure.
    pub fn raw_health_fraction(&self) -> f32 {
        self.hp.max(0) as f32 / self.max_hp as f32
    }

    // ---- Drawing ------------------------------------------------------------------------------------------------

    fn anim_frame(&self) -> i32 {
        let rate = if self.def.movement == "fly" {
            8.0
        } else if self.phase == Phase::Chase {
            5.0
        } else {
            3.0
        };
        (self.anim_time * rate) as i32 % self.strip.frame_count
    }

    fn draw_enemy(&self, world: &World, batch: &mut Batch) {
        let scale = self.def.scale;
        if self.phase == Phase::Burrowed {
            batch.draw(props::shadow(10), snap(self.base.pos - Vec2::new(5.0, 2.0)), Rgba::rgb(120, 80, 50).mul(0.8));
            return;
        }
        let mut seen = 1.0;
        if let Some(cloak) = world.cloak_radius()
            && self.flash <= 0.0
        {
            seen = ((cloak - world.hero.pos.distance(self.base.pos)) / 16.0 + 1.0).clamp(0.0, 1.0);
            if seen <= 0.0 {
                if (self.anim_time * 3.0 + self.base.pos.x) as i32 % 4 == 0 {
                    let feet = self.base.pos - Vec2::new(0.0, self.z.round());
                    self.strip.draw(
                        batch,
                        feet,
                        self.anim_frame(),
                        Rgba::rgb(120, 90, 160).mul(0.12),
                        self.facing_right,
                        true,
                        scale,
                    );
                }
                return;
            }
        }
        let shadow_width = (self.base.width * scale) as i32 + 2;
        batch.draw(
            props::shadow(shadow_width),
            snap(self.base.pos - Vec2::new(shadow_width as f32 / 2.0, 2.0)),
            Rgba::WHITE.mul((1.0 - self.z / 30.0) * seen),
        );

        let feet = self.base.pos - Vec2::new(0.0, self.z.round());
        if self.aura {
            let glow = self.aura_element.glow().mul(0.35 + (self.anim_time * 6.0).sin() * 0.15);
            for offset in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)] {
                self.strip.draw(batch, feet + offset, self.anim_frame(), glow, self.facing_right, true, scale);
            }
        }

        let mut alpha = if self.def.movement == "ghost" { 0.65 + (self.anim_time * 6.0).sin() * 0.2 } else { 1.0 };
        if self.spawn_fade > 0.0 && (self.spawn_fade * 30.0) as i32 % 2 == 0 {
            alpha *= 0.3;
        }
        alpha *= seen;
        let telegraph = self.phase == Phase::Windup
            && self.attack.is_some_and(|i| self.phase_time > self.def.attacks[i].windup - 0.3)
            && (self.phase_time * 20.0) as i32 % 2 == 0;
        let tint = self.status.tint().unwrap_or(if self.enraged {
            Rgba::rgb(255, 140, 120)
        } else if self.blighted {
            if self.revived { Rgba::rgb(150, 110, 190) } else { Rgba::rgb(215, 180, 240) }
        } else {
            Rgba::WHITE
        });
        let flash = self.flash > 0.0 || telegraph;
        if self.phase == Phase::Windup {
            let tex = if flash { self.strip.silhouette() } else { self.strip.texture };
            let origin = Vec2::new(self.strip.frame_width as f32 / 2.0, self.strip.frame_height as f32);
            batch.draw_ex(
                tex,
                snap(feet),
                Some(self.strip.source(self.anim_frame())),
                tint.mul(alpha),
                0.0,
                origin,
                Vec2::new(1.1, 0.9) * scale,
                self.facing_right,
                false,
            );
        } else {
            self.strip.draw(batch, feet, self.anim_frame(), tint.mul(alpha), self.facing_right, flash, scale);
        }
        if telegraph {
            self.strip.draw(
                batch,
                feet,
                self.anim_frame(),
                Rgba::rgb(255, 80, 60).mul(0.5),
                self.facing_right,
                true,
                scale,
            );
        }

        if self.frozen() {
            batch.draw(art::ice_block(), snap(self.base.pos - Vec2::new(8.0, 18.0)), Rgba::WHITE.mul(0.75));
        }
        if self.shield {
            let dir = safe_normalize(world.hero.pos - self.base.pos);
            let at = self.base.pos - Vec2::new(4.0, self.base.height * scale / 2.0 + 8.0 + self.z) + dir * 7.0;
            batch.draw(art::enemy_shield(self.shield_bane()), snap(at), Rgba::WHITE);
        }
        if self.phase == Phase::Broken || (self.phase == Phase::Stagger && self.phase_time < -0.3) {
            self.draw_stars(batch);
        }
        if self.blighted && !self.revived && self.hp > 0 {
            let at = snap(self.base.pos - Vec2::new(3.0, self.base.height * scale + 14.0 + self.z));
            batch.draw_ex(
                art::boss_gem(self.blight_mark.glow()),
                at,
                None,
                Rgba::WHITE.mul(0.9),
                0.0,
                Vec2::ZERO,
                Vec2::splat(0.55),
                false,
                false,
            );
        }
        if self.marked > 0.0 {
            let at = snap(
                self.base.pos - Vec2::new(0.0, self.base.height * scale + 16.0 + self.z + (self.anim_time * 8.0).sin()),
            );
            font::draw_centered(batch, "v", at, Rgba::rgb(255, 120, 80), Some(palette::TEXT_SHADOW));
        }
        self.draw_bars(batch);
    }

    fn draw_stars(&self, batch: &mut Batch) {
        let center = self.base.pos - Vec2::new(0.0, self.base.height * self.def.scale + 6.0 + self.z);
        for i in 0..3 {
            let a = self.anim_time * 6.0 + i as f32 * TAU / 3.0;
            batch.draw(props::spark(), snap(center + Vec2::new(a.cos() * 7.0, a.sin() * 2.5)), palette::HIGHLIGHT);
        }
    }

    fn draw_bars(&self, batch: &mut Batch) {
        if self.hp_bar_time <= 0.0 || self.hp <= 0 || self.boss.is_some() {
            return;
        }
        let width = ((self.base.width * self.def.scale) as i32 + 4).max(14) as f32;
        let at = snap(self.base.pos + Vec2::new(-width / 2.0, 3.0));
        let alpha = (self.hp_bar_time * 2.0).min(1.0);
        let height = if self.guard_max > 0.0 { 6.0 } else { 4.0 };
        batch.fill_rect(at.x - 1.0, at.y - 1.0, width + 2.0, height, palette::OUTLINE.mul(alpha));
        batch.fill_rect(at.x, at.y, width * self.hp as f32 / self.max_hp as f32, 2.0, palette::LIFE.mul(alpha));
        if self.guard_max > 0.0 {
            let color = if self.phase == Phase::Broken { palette::HIGHLIGHT } else { Rgba::rgb(170, 190, 220) };
            batch.fill_rect(at.x, at.y + 3.0, width * self.guard_value / self.guard_max, 1.0, color.mul(alpha));
        }
        let mut bx = at.x;
        for e in BASIC {
            let b = self.status.buildup(e);
            if b <= 0.02 {
                continue;
            }
            batch.fill_rect(bx, at.y - 3.0, (6.0 * b).max(1.0), 1.0, e.glow().mul(alpha));
            bx += 7.0;
        }
    }
}

fn strongest_resist(def: &EnemyDef) -> Element {
    let mut best = BASIC[0];
    for e in BASIC {
        if def.resist_to(e) > def.resist_to(best) {
            best = e;
        }
    }
    best
}

fn attack_color(attack: &AttackDef) -> Rgba {
    if attack.element.is_some() { Element::parse(attack.element.as_deref()).glow() } else { Rgba::rgb(255, 70, 60) }
}

fn random_vector(world: &World) -> Vec2 {
    let a = world.rng.next_f32() * TAU;
    Vec2::new(a.cos(), a.sin())
}

pub fn ray_end(world: &World, start: Vec2, dir: Vec2, distance: f32) -> Vec2 {
    let mut end = start;
    let mut d = 0.0;
    while d < distance {
        let probe = start + dir * d;
        let t = TILE as f32;
        if world.map.blocks_flying((probe.x / t).floor() as i32, (probe.y / t).floor() as i32) {
            break;
        }
        end = probe;
        d += 4.0;
    }
    end
}

impl Entity for Enemy {
    entity_base!();

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Enemy)
    }

    fn hurtbox(&self) -> Box2 {
        if let Some(brain) = self.boss.as_ref().and_then(|b| b.brain()) {
            return brain.hurtbox(self);
        }
        let scale = self.def.scale;
        Box2::new(
            self.base.pos.x - self.base.width / 2.0 - 1.0,
            self.base.pos.y - self.base.height * scale - 3.0 - self.z,
            self.base.width + 2.0,
            self.base.height * scale + 3.0,
        )
    }

    fn grounded(&self) -> bool {
        !self.is_flying() || self.boss.as_ref().and_then(|b| b.brain()).is_some_and(|b| b.touches_ground())
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        self.boss.as_ref().and_then(|b| b.brain()).and_then(|b| b.light(self))
    }

    fn sort_y(&self) -> f32 {
        self.boss.as_ref().and_then(|b| b.brain()).and_then(|b| b.sort_y(self)).unwrap_or(self.base.pos.y)
    }

    fn tether_box(&self) -> Option<Box2> {
        Some(self.hurtbox())
    }

    fn on_tether(&mut self, world: &mut World) -> TetherResponse {
        if self.shield {
            self.shield = false;
            self.enraged = true;
            self.alert(world);
            self.interrupt(0.3);
            let at = self.base.pos - Vec2::new(0.0, self.base.height + 14.0);
            world.add(FloatingText::new(at, "ENRAGED", Rgba::rgb(255, 80, 60)));
            return TetherResponse::Ripped;
        }
        if self.heavy() {
            return TetherResponse::None;
        }
        self.set_phase(Phase::Reeled);
        TetherResponse::PullTarget
    }

    fn added(&mut self, world: &mut World) {
        self.think_timer = world.rng.next_f32() * 1.2;
        if world.map_blighted() && self.boss.is_none() {
            self.blighted = true;
            self.blight_mark = strongest_resist(self.def);
        }
        if pacts::active(&world.state, "ironhide") {
            self.max_hp = (self.max_hp as f32 * 1.5) as i32;
            self.hp = self.max_hp;
        }
        if self.boss.is_some() {
            boss::added(self, world);
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        let dt = dt * Self::time_scale(world);
        self.anim_time += dt * self.status.speed_factor();
        self.flash = (self.flash - dt).max(0.0);
        self.spawn_fade = (self.spawn_fade - dt).max(0.0);
        self.contact_cooldown = (self.contact_cooldown - dt).max(0.0);
        self.hp_bar_time = (self.hp_bar_time - dt).max(0.0);
        self.marked = (self.marked - dt).max(0.0);
        let cooldown_rate = if self.enraged { 1.5 } else { 1.0 };
        for c in self.cooldowns.values_mut() {
            *c = (*c - dt * cooldown_rate).max(0.0);
        }

        self.update_status(world, dt);
        if self.base.removed || (self.hp <= 0 && self.boss.is_none()) {
            return;
        }

        if self.frozen() {
            self.set_phase(Phase::Stagger);
            return;
        }

        if self.knock_time > 0.0 {
            self.knock_time -= dt;
            let expected = self.knock_velocity * dt;
            let flying = self.is_flying();
            let moved = self.move_by(world, expected, flying);
            if self.wall_slam && moved.length() < expected.length() * 0.4 && expected.length() > 1.5 {
                self.wall_slam_hit(world);
            }
            return;
        }

        self.think(world, dt);
        if self.deals_contact_damage() {
            self.touch_target(world);
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        self.take_hit_inner(world, hit)
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        if let Some(core) = &self.boss {
            if let Some(brain) = core.brain() {
                brain.draw(self, core, world, batch);
            }
        } else {
            self.draw_enemy(world, batch);
        }
    }
}
