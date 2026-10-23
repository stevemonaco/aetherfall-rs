//! The Ember Warlock. While its staff gem holds, it blinks between anchor points; every attack is preceded by a visible
//! flare. Shatter the gem (a part) and it can no longer teleport, only stalk. It drowns easily in water.

use bevy::math::Vec2;

use super::boss::BossCore;
use super::combat_effects::{GroundMarker, LineMarker};
use super::effects::{FloatingText, Particle};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase};
use super::projectile::Projectile;
use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{IRect, safe_normalize, snap};
use crate::core::gfx::{Batch, FrameStrip};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::gameplay::game_data::data;
use crate::scenes::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Intro,
    Hover,
    FadeOut,
    FadeIn,
    RingWindup,
    Volley,
    Pillars,
    Summon,
    Recover,
    Stagger,
}

pub struct WarlockState {
    anchors: [Vec2; 5],
    step: Step,
    timer: f32,
    fade: f32,
    cycle: i32,
    volley_shots: i32,
    grounded: bool,
}

impl WarlockState {
    pub fn new(pos: Vec2) -> WarlockState {
        WarlockState {
            anchors: [
                pos,
                pos + Vec2::new(-64.0, 16.0),
                pos + Vec2::new(64.0, 16.0),
                pos + Vec2::new(-40.0, 48.0),
                pos + Vec2::new(40.0, 48.0),
            ],
            step: Step::Intro,
            timer: 1.6,
            fade: 1.0,
            cycle: 0,
            volley_shots: 0,
            grounded: false,
        }
    }

    pub fn tangible(&self) -> bool {
        !matches!(self.step, Step::FadeOut | Step::FadeIn | Step::Intro)
    }

    pub fn fade(&self) -> f32 {
        self.fade
    }

    fn strip() -> FrameStrip {
        FrameStrip::new(props::warlock(), 32, 40)
    }

    fn enraged(enemy: &Enemy) -> bool {
        enemy.health_fraction() < 0.5
    }

    fn next(&mut self, step: Step, time: f32) {
        self.step = step;
        self.timer = time;
    }

    pub fn gem_broken(&mut self, enemy: &mut Enemy, world: &mut World) {
        self.grounded = true;
        self.next(Step::Stagger, 3.0);
        self.fade = 1.0;
        world.add(FloatingText::new(enemy.base.pos - Vec2::new(0.0, 48.0), "It can't teleport now!", palette::HIGHLIGHT));
    }

    fn shoot(enemy: &Enemy, world: &mut World, from: Vec2, direction: Vec2, speed_scale: f32) {
        world.add(Projectile::enemy_shot("fire", from, safe_normalize(direction), enemy.stats(), 1.0, speed_scale));
    }

    pub fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.timer -= dt;
        let time = enemy.boss.as_ref().map(|b| b.time).unwrap_or(0.0);
        enemy.z = if self.grounded { 0.0 } else { 4.0 + (time * 2.2).sin() * 2.0 };
        let player = world.hero.pos;

        match self.step {
            Step::Intro => {
                if self.timer <= 0.0 {
                    self.next(Step::Hover, 0.8);
                }
            }
            Step::Hover => {
                let to_player = player - enemy.base.pos;
                if to_player.length() > 30.0 {
                    let speed = if self.grounded { 34.0 } else { 14.0 };
                    enemy.move_by(world, to_player.normalize() * speed * dt, !self.grounded);
                }
                if self.timer <= 0.0 {
                    if self.grounded {
                        self.choose_attack(enemy, world);
                    } else {
                        self.next(Step::FadeOut, 0.4);
                    }
                }
            }
            Step::FadeOut => {
                self.fade = self.timer / 0.4;
                if self.timer <= 0.0 {
                    let choices: Vec<Vec2> = self.anchors.iter().copied().filter(|a| a.distance(player) > 48.0).collect();
                    enemy.base.pos =
                        if choices.is_empty() { self.anchors[0] } else { choices[world.rng.below(choices.len() as i32) as usize] };
                    self.next(Step::FadeIn, 0.4);
                }
            }
            Step::FadeIn => {
                self.fade = 1.0 - self.timer / 0.4;
                if self.timer <= 0.0 {
                    self.fade = 1.0;
                    self.choose_attack(enemy, world);
                }
            }
            Step::RingWindup => {
                if world.rng.next_f32() < 0.6 {
                    let at = enemy.base.pos - Vec2::new(world.rng.range(-14, 15) as f32, 20.0 + world.rng.below(16) as f32);
                    world.add(Particle::new(at, Vec2::new(0.0, -30.0), Rgba::rgb(255, 160, 60), 0.4));
                }
                if self.timer <= 0.0 {
                    let count = if Self::enraged(enemy) { 14 } else { 10 };
                    let offset = world.rng.next_f32();
                    let from = enemy.base.pos - Vec2::new(0.0, 6.0);
                    for i in 0..count {
                        let a = offset + i as f32 * std::f32::consts::TAU / count as f32;
                        Self::shoot(enemy, world, from, Vec2::new(a.cos(), a.sin()), 1.0);
                    }
                    audio::sfx(Sound::ShootFire);
                    world.camera.shake(0.15, 1.0);
                    self.next(Step::Recover, 1.3);
                }
            }
            Step::Volley => {
                if self.timer <= 0.0 {
                    let from = enemy.base.pos - Vec2::new(0.0, 14.0);
                    let dir = player - Vec2::new(0.0, 6.0) - from;
                    Self::shoot(enemy, world, from, dir, 1.1);
                    audio::play(Sound::ShootFire, 0.7, 0.0);
                    self.timer = 0.32;
                    self.volley_shots -= 1;
                    if self.volley_shots <= 0 {
                        self.next(Step::Recover, 1.1);
                    } else {
                        world.add(LineMarker::new(from, from + safe_normalize(dir) * 90.0, 6.0, 0.3, Rgba::rgb(255, 120, 50)));
                    }
                }
            }
            Step::Pillars => {
                if self.timer <= 0.0 {
                    self.next(Step::Recover, 1.4);
                }
            }
            Step::Summon => {
                if self.timer <= 0.0 {
                    if let Some(wisp) = data().enemies.get("wisp")
                        && world.entities_of::<Enemy>().filter(|e| e.def.id == "wisp").count() < 3
                    {
                        let level = Some(enemy.level - 1);
                        world.add(Enemy::new(wisp, enemy.base.pos + Vec2::new(-20.0, 4.0), level));
                        world.add(Enemy::new(wisp, enemy.base.pos + Vec2::new(20.0, 4.0), level));
                    }
                    audio::play(Sound::BossRoar, 0.6, 0.0);
                    self.next(Step::Recover, 1.0);
                }
            }
            Step::Recover | Step::Stagger => {
                if self.timer <= 0.0 {
                    self.next(Step::Hover, if self.grounded { 0.8 } else { 1.2 });
                }
            }
        }
    }

    fn choose_attack(&mut self, enemy: &mut Enemy, world: &mut World) {
        self.cycle += 1;
        let enraged = Self::enraged(enemy);
        if enraged && self.cycle % 4 == 0 {
            self.next(Step::Summon, 0.9);
            return;
        }
        match self.cycle % 3 {
            0 => {
                self.next(Step::RingWindup, 0.9);
                world.add(GroundMarker::new(enemy.base.pos, 70.0, 0.9, Rgba::rgb(255, 120, 40)));
                audio::play(Sound::ChargeLevel, 0.7, -0.4);
            }
            1 => {
                self.volley_shots = if enraged { 5 } else { 3 };
                self.next(Step::Volley, 0.5);
                world.add(LineMarker::new(enemy.base.pos - Vec2::new(0.0, 14.0), world.hero.pos, 6.0, 0.5, Rgba::rgb(255, 120, 50)));
            }
            _ => {
                let count = if enraged { 5 } else { 3 };
                for i in 0..count {
                    let jitter = if i == 0 {
                        Vec2::ZERO
                    } else {
                        Vec2::new(world.rng.range(-50, 51) as f32, world.rng.range(-36, 37) as f32)
                    };
                    let spot = world.hero.pos + jitter;
                    let hit = Hit {
                        power: 1.3,
                        element: Element::Fire,
                        from: spot,
                        owner: Some(enemy.base.id),
                        kind: HitKind::Attack,
                        knockback: 180.0,
                        ..Hit::new(Faction::Enemy, enemy.stats())
                    };
                    world.add(EmberPillar::new(spot, 1.0 + i as f32 * 0.12, hit));
                }
                audio::play(Sound::ChargeLevel, 0.8, -0.6);
                self.next(Step::Pillars, 1.2);
            }
        }
    }

    pub fn draw(&self, enemy: &Enemy, boss: &BossCore, world: &World, batch: &mut Batch) {
        let fade = self.fade * boss.death_fade();
        if fade <= 0.02 {
            return;
        }
        let time = boss.time;
        let flare = if self.step == Step::RingWindup { 1.0 - self.timer / 0.9 } else { 0.0 };
        let mix = if flare > 0.0 { flare } else { ((time * 6.0).sin() + 1.0) / 2.0 };
        let aura = Rgba::rgb(255, 90, 30).lerp(Rgba::rgb(255, 230, 120), mix).mul((0.35 + flare * 0.5) * fade);
        let frame = (time * 5.0) as i32 % 2;
        let feet = enemy.base.pos - Vec2::new(0.0, enemy.z.round());
        let strip = Self::strip();
        batch.draw(props::shadow(24), snap(enemy.base.pos - Vec2::new(12.0, 4.0)), Rgba::WHITE.mul(fade));
        for offset in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0)] {
            strip.draw(batch, feet + offset * (1.0 + flare), frame, aura, false, true, 1.0);
        }
        let staggered = self.step == Step::Stagger && (time * 10.0) as i32 % 2 == 0;
        let flash = (enemy.flashing() || boss.dying() || staggered) && (time * 20.0) as i32 % 2 == 0;
        strip.draw(batch, feet, frame, boss.tint().mul(fade), world.hero.pos.x < enemy.base.pos.x, flash, 1.0);
    }
}

/// A ring of embers on the ground that erupts into a pillar of fire when its time runs out.
pub struct EmberPillar {
    base: EntityBase,
    delay: f32,
    hit: Hit,
    age: f32,
    erupted: bool,
}

impl EmberPillar {
    pub fn new(at: Vec2, delay: f32, hit: Hit) -> EmberPillar {
        EmberPillar { base: EntityBase::at(at), delay, hit, age: 0.0, erupted: false }
    }
}

impl Entity for EmberPillar {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        self.erupted.then_some((40.0, Rgba::rgb(255, 150, 60)))
    }

    fn added(&mut self, world: &mut World) {
        world.add(GroundMarker::new(self.base.pos, 18.0, self.delay, Rgba::rgb(255, 110, 40)));
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if !self.erupted && self.age >= self.delay {
            self.erupted = true;
            audio::play(Sound::ShootFire, 0.6, -0.2);
            if !world.hero.airborne && world.hero.pos.distance(self.base.pos) < 20.0 {
                let mut hit = self.hit.clone();
                world.hit_player(&mut hit);
                if hit.parried
                    && let Some(owner) = hit.owner
                {
                    world.with_as::<Enemy, _>(owner, |e, w| e.parried(w));
                }
            }
            let (tx, ty) = self.base.tile_pos();
            world.chem_ignite(tx, ty);
            for _ in 0..14 {
                let at = self.base.pos + Vec2::new(world.rng.range(-6, 7) as f32, 0.0);
                let vel = Vec2::new(world.rng.range(-10, 11) as f32, -60.0 - world.rng.below(60) as f32);
                let color = if world.rng.next_f32() < 0.5 { Rgba::rgb(255, 200, 70) } else { Rgba::rgb(255, 100, 40) };
                world.add(Particle::new(at, vel, color, 0.6));
            }
        }
        if self.age > self.delay + 0.5 {
            self.base.removed = true;
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        if !self.erupted {
            return;
        }
        let frame = (self.age * 14.0) as i32 % 4;
        let rise = ((self.age - self.delay) * 6.0).min(1.0);
        let src = IRect::new(frame * 16, 0, 16, 22);
        let at = snap(self.base.pos - Vec2::new(8.0, 30.0 * rise));
        batch.draw_ex(crate::art::combat::flames(), at, Some(src), Rgba::WHITE, 0.0, Vec2::ZERO, Vec2::new(1.0, 1.4), false, false);
    }
}
