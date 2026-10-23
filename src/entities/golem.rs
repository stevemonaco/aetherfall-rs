//! The Storm Golem. Its left arm drives the ground slam, its right arm calls the lightning; break an arm and that attack
//! is gone. The storm core is armoured until both arms fall or water seizes it up. It charges anyone who keeps away.

use bevy::math::Vec2;

use super::boss::{self, BossBrain, BossCore};
use super::combat_effects::{GroundMarker, LightningStrike, LineMarker, Shockwave};
use super::effects::{FloatingText, Particle};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use crate::art::{bosses, combat, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, safe_normalize, snap};
use crate::core::gfx::{Batch, FrameStrip};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::chemistry::conductive;

const LEFT_ARM: usize = 0;
const RIGHT_ARM: usize = 1;
const CORE: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Intro,
    Walk,
    SlamWindup,
    Strikes,
    ChargeWindup,
    Charge,
    Recover,
    Seized,
}

pub struct GolemBrain {
    step: Step,
    timer: f32,
    water: f32,
    charge_dir: Vec2,
    cycle: i32,
    core: Option<EntityId>,
    broken: [bool; 3],
}

impl GolemBrain {
    pub fn new(enemy: &mut Enemy) -> GolemBrain {
        enemy.base.width = 24.0;
        enemy.base.height = 12.0;
        GolemBrain {
            step: Step::Intro,
            timer: 1.4,
            water: 0.0,
            charge_dir: Vec2::ZERO,
            cycle: 0,
            core: None,
            broken: [false; 3],
        }
    }

    fn strip() -> FrameStrip {
        FrameStrip::new(bosses::golem(), 40, 44)
    }

    fn core_exposed(&self) -> bool {
        self.step == Step::Seized || self.broken[LEFT_ARM] && self.broken[RIGHT_ARM]
    }

    fn enraged(enemy: &Enemy) -> bool {
        enemy.health_fraction() < 0.5
    }

    fn facing(enemy: &Enemy, world: &World) -> bool {
        world.hero.pos.x < enemy.base.pos.x
    }

    fn next(&mut self, step: Step, time: f32) {
        self.step = step;
        self.timer = time;
    }

    fn think_step(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.timer -= dt;
        if let Some(core) = self.core {
            boss::set_part(world, core, !self.core_exposed(), false);
        }
        let to_player = world.hero.pos - enemy.base.pos;
        let time = enemy.boss.as_ref().map_or(0.0, |b| b.time);

        match self.step {
            Step::Intro => {
                if self.timer <= 0.0 {
                    self.next(Step::Walk, 1.6);
                }
            }
            Step::Walk => {
                let speed = if Self::enraged(enemy) { 26.0 } else { 18.0 };
                enemy.move_by(world, safe_normalize(to_player) * speed * dt, false);
                if self.timer <= 0.0 {
                    self.choose_attack(enemy, world, to_player);
                }
            }
            Step::SlamWindup => {
                if self.timer <= 0.0 {
                    let center = enemy.base.pos + Vec2::new(if Self::facing(enemy, world) { -20.0 } else { 20.0 }, 2.0);
                    world.add(Shockwave::new(center, 38.0, Rgba::rgb(200, 210, 255)));
                    world.camera.shake(0.35, 3.0);
                    audio::sfx(Sound::Break);
                    if !world.hero.airborne && world.hero.pos.distance(center) < 40.0 {
                        boss::hit_player(enemy, world, boss::boss_hit(enemy, 1.6, Element::None, center, false));
                    }
                    if Self::enraged(enemy) {
                        boss::ring(enemy, world, "spark", enemy.base.pos - Vec2::new(0.0, 4.0), 12, time, 0.9, 1.0);
                    }
                    self.next(Step::Recover, 1.2);
                }
            }
            Step::Strikes => {
                if self.timer <= 0.0 {
                    self.next(Step::Recover, 1.0);
                }
            }
            Step::ChargeWindup => {
                if self.timer <= 0.0 {
                    self.next(Step::Charge, 1.2);
                    audio::play(Sound::BossRoar, 0.6, 0.2);
                }
            }
            Step::Charge => {
                let moved = enemy.move_by(world, self.charge_dir * 150.0 * dt, false);
                if world.hero.pos.distance(enemy.base.pos - Vec2::new(0.0, 10.0)) < 22.0 {
                    let hit = boss::boss_hit(enemy, 1.5, Element::None, enemy.base.pos, false);
                    boss::hit_player(enemy, world, hit);
                }
                if moved.length_squared() < 0.01 {
                    world.camera.shake(0.3, 2.0);
                    audio::play(Sound::Break, 0.7, 0.0);
                    self.next(Step::Seized, 2.0);
                } else if self.timer <= 0.0 {
                    self.next(Step::Recover, 0.9);
                }
                // Spawned here rather than in draw, which cannot add entities.
                if self.step == Step::Charge && world.rng.next_f32() < 0.5 {
                    let at = enemy.base.pos + Vec2::new(world.rng.range(-10, 11) as f32, -2.0);
                    world.add(Particle::new(at, -self.charge_dir * 30.0, Rgba::rgb(200, 190, 170), 0.4));
                }
            }
            Step::Seized => {
                if world.rng.next_f32() < 0.4 {
                    let at = enemy.base.pos + Vec2::new(world.rng.range(-12, 13) as f32, -30.0);
                    world.add(Particle::new(at, Vec2::new(0.0, -30.0), Rgba::rgb(220, 230, 255), 0.6));
                }
                if self.timer <= 0.0 {
                    self.next(Step::Walk, 1.2);
                }
            }
            Step::Recover => {
                if self.timer <= 0.0 {
                    self.next(Step::Walk, if Self::enraged(enemy) { 1.0 } else { 1.6 });
                }
            }
        }
    }

    fn choose_attack(&mut self, enemy: &mut Enemy, world: &mut World, to_player: Vec2) {
        self.cycle += 1;
        let enraged = Self::enraged(enemy);
        let from = enemy.base.pos - Vec2::new(0.0, 6.0);
        if to_player.length() > 100.0 {
            self.charge_dir = safe_normalize(to_player);
            self.next(Step::ChargeWindup, 0.8);
            world.add(LineMarker::new(from, from + self.charge_dir * 200.0, 22.0, 0.8, Rgba::rgb(255, 90, 70)));
            return;
        }
        let (slam, strikes) = (!self.broken[LEFT_ARM], !self.broken[RIGHT_ARM]);
        if slam && (!strikes || self.cycle % 2 == 0) {
            self.next(Step::SlamWindup, 0.8);
            let center = enemy.base.pos + Vec2::new(if Self::facing(enemy, world) { -20.0 } else { 20.0 }, 2.0);
            world.add(GroundMarker::new(center, 38.0, 0.8, Rgba::rgb(200, 210, 255)));
            audio::play(Sound::ChargeLevel, 0.6, -0.6);
        } else if strikes {
            let target = world.hero.pos;
            world.add(StrikeMarker::new(target, 0.9, boss::boss_hit(enemy, 1.4, Element::Thunder, target, false)));
            for i in 0..(if enraged { 4 } else { 2 }) {
                let at = target + Vec2::new(world.rng.range(-48, 49) as f32, world.rng.range(-32, 33) as f32);
                let hit = boss::boss_hit(enemy, 1.4, Element::Thunder, target, false);
                world.add(StrikeMarker::new(at, 0.9 + i as f32 * 0.15, hit));
            }
            audio::play(Sound::ChargeLevel, 0.8, -0.5);
            self.next(Step::Strikes, 1.3);
        } else {
            self.charge_dir = safe_normalize(to_player);
            self.next(Step::ChargeWindup, 0.7);
            world.add(LineMarker::new(from, from + self.charge_dir * 200.0, 22.0, 0.7, Rgba::rgb(255, 90, 70)));
        }
    }
}

impl BossBrain for GolemBrain {
    fn added(&mut self, enemy: &mut Enemy, world: &mut World) {
        let size = Vec2::new(12.0, 14.0);
        let left = combat::boss_gem(Rgba::rgb(150, 170, 255));
        boss::add_part(enemy, world, LEFT_ARM, "Left arm", 0.12, left, size, Some("storm_glass"));
        let right = combat::boss_gem(Rgba::rgb(255, 235, 120));
        boss::add_part(enemy, world, RIGHT_ARM, "Right arm", 0.12, right, size, Some("storm_glass"));
        let gem = combat::boss_gem(Rgba::rgb(120, 220, 255));
        let core =
            boss::add_part(enemy, world, CORE, "Storm core", 0.2, gem, Vec2::new(12.0, 12.0), Some("frost_core"));
        boss::set_part(world, core, true, false);
        self.core = Some(core);
    }

    fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.think_step(enemy, world, dt);
    }

    fn draw(&self, enemy: &Enemy, boss: &BossCore, world: &World, batch: &mut Batch) {
        let fade = boss.death_fade();
        let time = boss.time;
        batch.draw(props::shadow(30), snap(enemy.base.pos - Vec2::new(15.0, 5.0)), Rgba::WHITE.mul(fade));
        let windup = matches!(self.step, Step::SlamWindup | Step::ChargeWindup);
        let frame = if windup { 1 } else { 0 };
        let bob = if self.step == Step::Walk {
            Vec2::new(0.0, -(((time * 6.0).sin().abs() * 2.0) as i32) as f32)
        } else {
            Vec2::ZERO
        };
        let shake = if windup { Vec2::new(world.rng.range(-1, 2) as f32, 0.0) } else { Vec2::ZERO };
        let flash = (enemy.flashing() || boss.dying()) && (time * 20.0) as i32 % 2 == 0;
        let tint = if self.step == Step::Seized { Rgba::rgb(170, 200, 255) } else { boss.tint() };
        let facing = Self::facing(enemy, world);
        Self::strip().draw(batch, enemy.base.pos + bob + shake, frame, tint.mul(fade), facing, flash, 1.0);
    }

    fn hurtbox(&self, enemy: &Enemy) -> Box2 {
        Box2::new(enemy.base.pos.x - 14.0, enemy.base.pos.y - 40.0, 28.0, 40.0)
    }

    fn tangible(&self) -> bool {
        self.step != Step::Intro
    }

    fn death_color(&self) -> Rgba {
        Rgba::rgb(180, 200, 255)
    }

    fn before_hit(&mut self, enemy: &mut Enemy, world: &mut World, hit: &mut Hit) -> Option<bool> {
        if hit.is_player() && hit.element == Element::Water && self.step != Step::Seized {
            self.water +=
                if matches!(hit.kind, HitKind::ChargedBolt | HitKind::Art | HitKind::Finisher) { 3.0 } else { 1.0 };
            if self.water >= 10.0 {
                self.water = 0.0;
                self.next(Step::Seized, 4.0);
                audio::play(Sound::Burn, 1.0, -0.3);
                let at = enemy.base.pos - Vec2::new(0.0, 56.0);
                world.add(FloatingText::new(at, "SEIZED", Rgba::rgb(160, 220, 255)));
            }
        }
        if !self.core_exposed() && hit.is_player() {
            hit.power *= 0.5;
            hit.knockback = 0.0;
        }
        None
    }

    fn part_anchor(&self, index: usize, enemy: &Enemy) -> Vec2 {
        enemy.base.pos
            + match index {
                LEFT_ARM => Vec2::new(-17.0, -22.0),
                RIGHT_ARM => Vec2::new(17.0, -22.0),
                _ => Vec2::new(0.0, -26.0),
            }
    }

    fn part_broken(&mut self, index: usize, enemy: &mut Enemy, world: &mut World) {
        self.broken[index] = true;
        if index == CORE {
            self.next(Step::Seized, 4.0);
            world.add(FloatingText::new(
                enemy.base.pos - Vec2::new(0.0, 56.0),
                "The storm gutters out!",
                palette::HIGHLIGHT,
            ));
        } else {
            self.next(Step::Recover, 2.0);
        }
    }
}

/// A glowing ring on the ground that calls down lightning when its timer runs out.
pub struct StrikeMarker {
    base: EntityBase,
    delay: f32,
    hit: Hit,
    age: f32,
}

impl StrikeMarker {
    const RADIUS: f32 = 14.0;

    pub fn new(at: Vec2, delay: f32, hit: Hit) -> StrikeMarker {
        StrikeMarker { base: EntityBase::at(at), delay, hit, age: 0.0 }
    }
}

impl Entity for StrikeMarker {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if self.age < self.delay {
            return;
        }
        self.base.removed = true;
        world.add(LightningStrike::new(self.base.pos));
        audio::play(Sound::Clink, 1.0, -0.7);
        world.camera.shake(0.12, 1.0);
        if world.hero.pos.distance(self.base.pos) < Self::RADIUS + 4.0 {
            let mut hit = self.hit.clone();
            world.hit_player(&mut hit);
            if hit.parried
                && let Some(owner) = hit.owner
            {
                world.with_as::<Enemy, _>(owner, |e, w| e.parried(w));
            }
        }
        let (tx, ty) = self.base.tile_pos();
        if conductive(world.map.at(tx, ty)) {
            world.chem_electrify(tx, ty, None);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let t = self.age / self.delay;
        let blink = (self.age * (6.0 + t * 14.0)) as i32 % 2;
        let color = Rgba::rgb(255, 240, 120).lerp(Rgba::WHITE, t).mul(0.4 + 0.6 * blink as f32);
        let pos = self.base.pos;
        let mut ring = |radius: f32, step: f32, color: Rgba| {
            let mut a = 0.0f32;
            while a < std::f32::consts::TAU {
                batch.fill_rect(pos.x + a.cos() * radius, pos.y + a.sin() * radius * 0.5, 1.0, 1.0, color);
                a += step;
            }
        };
        ring(Self::RADIUS, 0.2, color);
        ring(Self::RADIUS * t, 0.35, color.mul(0.7));
    }
}
