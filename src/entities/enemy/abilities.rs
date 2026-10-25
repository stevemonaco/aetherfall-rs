//! The less common enemy attacks (volleys, mortars, eruptions, hooks, whirls and support casts) and the traits that
//! act every frame.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::affixes::Detonation;
use super::{Enemy, SHIELD_HP, attack_color, ray_end};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{rotate, safe_normalize};
use crate::entities::combat_effects::{Beam, GroundMarker, Shockwave};
use crate::entities::effects::{FloatingText, Particle};
use crate::entities::entity::{Entity, EntityId};
use crate::entities::projectile::Projectile;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::AttackDef;
use crate::scenes::world::World;

/// Seconds a mortar shell spends in the air.
pub(super) const MORTAR_FLIGHT: f32 = 0.8;
const WHIRL_TIME: f32 = 1.2;
const WHIRL_TICK: f32 = 0.3;
const SUPPORT_COLOR: Rgba = Rgba::rgb(140, 255, 170);
const WARD_TARGETS: usize = 2;

/// Where each shell of a mortar lands: the first on the target, the rest in a ring around it.
pub(super) fn mortar_spots(attack: &AttackDef, target: Vec2) -> Vec<Vec2> {
    let ring = (attack.count - 1).max(1) as f32;
    (0..attack.count)
        .map(
            |i| {
                if i == 0 {
                    target
                } else {
                    target + rotate(Vec2::new(attack.radius * 1.6, 0.0), i as f32 * TAU / ring)
                }
            },
        )
        .collect()
}

impl Enemy {
    pub(super) fn telegraph_mortar(&self, world: &mut World, attack: &AttackDef, target: Vec2) {
        for spot in mortar_spots(attack, target) {
            world.add(GroundMarker::new(spot, attack.radius, attack.windup + MORTAR_FLIGHT, attack_color(attack)));
        }
    }

    pub(super) fn mortar(&self, world: &mut World, attack: &AttackDef, from: Vec2) {
        for spot in mortar_spots(attack, self.locked_target) {
            let mut shell =
                Projectile::enemy_shot(&attack.projectile, from, Vec2::ZERO, self.stats(), attack.power, 1.0);
            shell.velocity = (spot - from) / MORTAR_FLIGHT;
            shell.lifetime = MORTAR_FLIGHT;
            shell.lob_height = 40.0;
            shell.size = attack.radius * 0.8;
            shell.ignores_walls = true;
            shell.reflectable = false;
            world.add(shell);
        }
        audio::play(Sound::ShootFire, 0.5, -0.3);
    }

    /// One shot of a burst, re-aimed at the target each time.
    pub(super) fn burst_shot(&mut self, world: &mut World, attack: &AttackDef, from: Vec2) {
        if let Some(target) = world.enemy_target(self.base.pos) {
            self.locked_dir = safe_normalize(target - self.base.pos);
        }
        let wobble = (world.rng.next_f32() - 0.5) * attack.spread;
        let dir = rotate(self.locked_dir, wobble);
        world.add(Projectile::enemy_shot(&attack.projectile, from, dir, self.stats(), attack.power, attack.speed));
        audio::play(Sound::ShootWind, 0.35, 0.5);
    }

    /// One turn of a spiral: two opposed shots, a little further round each time.
    pub(super) fn spiral_shot(&self, world: &mut World, attack: &AttackDef, from: Vec2, tick: i32) {
        let angle = self.locked_dir.y.atan2(self.locked_dir.x) + tick as f32 * 0.45;
        for arm in [0.0, std::f32::consts::PI] {
            let dir = Vec2::new((angle + arm).cos(), (angle + arm).sin());
            world.add(Projectile::enemy_shot(&attack.projectile, from, dir, self.stats(), attack.power, attack.speed));
        }
        audio::play(Sound::ShootWind, 0.3, 0.7);
    }

    /// Spins toward the target, striking everything close on each beat; returns whether the whirl is over.
    pub(super) fn whirl(&mut self, world: &mut World, attack: &AttackDef, dt: f32) -> bool {
        if let Some(target) = world.enemy_target(self.base.pos) {
            let dir = safe_normalize(target - self.base.pos);
            let flying = self.is_flying();
            let speed = self.move_speed() * 1.3;
            self.move_by(world, dir * speed * dt, flying);
        }
        if self.phase_time >= self.act_ticks as f32 * WHIRL_TICK {
            self.act_ticks += 1;
            world.add(Shockwave::new(self.base.pos, attack.radius, attack_color(attack)));
            audio::play(Sound::Swing, 0.6, 0.2);
            if !world.hero.airborne && world.hero.pos.distance(self.base.pos) < attack.radius + 4.0 {
                let mut hit = self.make_attack_hit(attack, self.base.pos);
                self.strike_player(world, &mut hit);
            }
        }
        self.phase_time >= WHIRL_TIME
    }

    /// A line that drags the target in if it connects.
    pub(super) fn hook(&mut self, world: &mut World, attack: &AttackDef) {
        let start = self.base.pos - Vec2::new(0.0, 4.0);
        let end = ray_end(world, start, self.locked_dir, attack.distance);
        world.add(Beam::new(start - Vec2::new(0.0, 2.0), end - Vec2::new(0.0, 2.0), attack_color(attack)));
        audio::play(Sound::Swing, 0.8, -0.4);
        let player = world.hero.pos;
        let along = (player - start).dot(self.locked_dir).clamp(0.0, start.distance(end));
        if (start + self.locked_dir * along).distance(player - Vec2::new(0.0, 4.0)) >= 10.0 {
            return;
        }
        let mut hit = self.make_attack_hit(attack, player + self.locked_dir * 24.0);
        hit.knockback = 300.0;
        self.strike_player(world, &mut hit);
    }

    /// A row of blasts racing out along the aim.
    pub(super) fn eruption(&self, world: &mut World, attack: &AttackDef) {
        let element = Element::parse(attack.element.as_deref());
        let step = attack.radius * 1.4;
        for i in 0..attack.count {
            let at = self.base.pos + self.locked_dir * step * (i + 1) as f32;
            let t = crate::world::map::TILE as f32;
            if world.map.blocks_walking((at.x / t).floor() as i32, ((at.y - 3.0) / t).floor() as i32) {
                break;
            }
            world.add(Detonation::new(at, self.stats(), element, attack.radius, 0.2 + i as f32 * 0.12, attack.power));
        }
        audio::play(Sound::Break, 0.5, -0.4);
    }

    /// Whether a support cast would do anything: a hurt ally to mend, or an unshielded one to ward.
    pub(super) fn support_needed(&self, world: &World, attack: &AttackDef) -> bool {
        world.entities_of::<Enemy>().any(|e| {
            e.base.id != self.base.id
                && e.boss.is_none()
                && e.hp > 0
                && e.base.pos.distance(self.base.pos) < attack.radius
                && match attack.kind.as_str() {
                    "mend" => e.hp < e.max_hp * 3 / 4,
                    _ => !e.shield,
                }
        })
    }

    /// Mend heals every ally in reach; Ward shields only the nearest few, so a warder can't armour a whole crowd.
    pub(super) fn support(&self, world: &mut World, attack: &AttackDef) {
        let me = self.base.pos;
        let ward = attack.kind == "ward";
        let mut targets: Vec<(EntityId, f32)> = world
            .entities_of::<Enemy>()
            .filter(|e| e.base.id != self.base.id && e.boss.is_none() && e.hp > 0 && !(ward && e.shield))
            .map(|e| (e.base.id, e.base.pos.distance(me)))
            .filter(|&(_, d)| d < attack.radius)
            .collect();
        if ward {
            targets.sort_by(|a, b| a.1.total_cmp(&b.1));
            targets.truncate(WARD_TARGETS);
        }
        for (id, _) in targets {
            world.with_as::<Enemy, _>(id, |e, w| {
                if ward {
                    e.shield = true;
                    e.shield_hp = SHIELD_HP;
                } else {
                    e.hp = (e.hp + (e.max_hp as f32 * 0.25 * attack.power) as i32).min(e.max_hp);
                    e.hp_bar_time = 2.5;
                }
                Particle::burst(w, e.hurtbox().center(), SUPPORT_COLOR, 8, 30.0, 0.4, true);
            });
        }
        world.add(Shockwave::new(me, attack.radius, SUPPORT_COLOR));
        audio::play(Sound::ChargeLevel, 0.6, 0.4);
    }

    pub(super) fn telegraph_support(&self, world: &mut World, attack: &AttackDef) {
        Particle::burst(world, self.hurtbox().center(), SUPPORT_COLOR, 8, 30.0, attack.windup, false);
    }

    /// Traits that act every frame: regeneration out of combat and enraging at half life.
    pub(super) fn update_traits(&mut self, world: &mut World, dt: f32) {
        if self.def.has_trait("regen") && self.hp < self.max_hp && self.hp_bar_time <= 0.0 {
            self.regen += self.max_hp as f32 * 0.04 * dt;
            let whole = self.regen as i32;
            self.hp = (self.hp + whole).min(self.max_hp);
            self.regen -= whole as f32;
        }
        if self.def.has_trait("enrage") && !self.enraged && self.hp * 2 < self.max_hp {
            self.enraged = true;
            let at = self.base.pos - Vec2::new(0.0, self.base.height + 14.0);
            world.add(FloatingText::new(at, "ENRAGED", Rgba::rgb(255, 80, 60)));
        }
    }

    /// A Volatile trait's parting blast.
    pub(super) fn trait_death(&self, world: &mut World) {
        if let Some(element) = self.def.trait_value("volatile") {
            let element = Element::parse(Some(element));
            world.add(Detonation::new(self.base.pos, self.stats(), element, 26.0, 0.8, 1.4));
        }
    }
}
