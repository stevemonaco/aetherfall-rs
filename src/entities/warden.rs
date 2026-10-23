//! The Concord Warden, the Bastion's last automaton. It wraps itself in an elemental shield that cycles through the
//! elements of its four emitters; only the shield's opposite element cracks it, and breaking an emitter (a part)
//! takes its element out of the cycle. With all four gone the shield fails for good.

use bevy::math::Vec2;

use super::boss::{self, BossBrain, BossCore};
use super::combat_effects::{Beam, GroundMarker, LineMarker, Shockwave};
use super::effects::{FloatingText, Particle};
use super::enemy::Enemy;
use super::golem::StrikeMarker;
use crate::art::{combat as art, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, IRect, rotate, safe_normalize, snap};
use crate::core::gfx::{self, Batch};
use crate::core::palette;
use crate::gameplay::combat::Hit;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::data;
use crate::scenes::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Intro,
    Stalk,
    BeamWindup,
    Salvo,
    Slam,
    Summon,
    Recover,
    Cracked,
}

const EMITTERS: [(Element, Vec2); 4] = [
    (Element::Fire, Vec2::new(-30.0, -10.0)),
    (Element::Water, Vec2::new(30.0, -10.0)),
    (Element::Wind, Vec2::new(-22.0, -46.0)),
    (Element::Thunder, Vec2::new(22.0, -46.0)),
];

const SLAM_COLOR: Rgba = Rgba::rgb(220, 220, 255);

pub struct WardenBrain {
    /// The elements whose emitters still stand, in layout order.
    live: Vec<Element>,
    step: Step,
    timer: f32,
    shield_timer: f32,
    beam_angle: f32,
    shield: Element,
    shield_up: bool,
    cycle: i32,
    beam_dir: Vec2,
}

impl WardenBrain {
    pub fn new(enemy: &mut Enemy) -> WardenBrain {
        enemy.base.width = 22.0;
        enemy.base.height = 12.0;
        WardenBrain {
            live: Vec::new(),
            step: Step::Intro,
            timer: 1.5,
            shield_timer: 0.0,
            beam_angle: 0.0,
            shield: Element::Fire,
            shield_up: true,
            cycle: 0,
            beam_dir: Vec2::ZERO,
        }
    }

    fn enraged(enemy: &Enemy) -> bool {
        enemy.health_fraction() < 0.4
    }

    fn next(&mut self, step: Step, time: f32) {
        self.step = step;
        self.timer = time;
    }

    fn raise_shield(&mut self, enemy: &Enemy) {
        if self.live.is_empty() {
            self.shield_up = false;
            return;
        }
        let index = self.live.iter().position(|&e| e == self.shield).map_or(0, |i| i + 1);
        self.shield = self.live[index % self.live.len()];
        self.shield_up = true;
        self.shield_timer = if Self::enraged(enemy) { 7.0 } else { 10.0 };
        audio::play(Sound::ChargeLevel, 0.7, 0.2);
    }

    fn think_step(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.timer -= dt;
        if !self.shield_up && !self.live.is_empty() {
            self.shield_timer -= dt;
            if self.shield_timer <= 0.0 && self.step != Step::Cracked {
                self.raise_shield(enemy);
            }
        } else if self.shield_up {
            self.shield_timer -= dt;
            if self.shield_timer <= 0.0 {
                self.raise_shield(enemy);
            }
        }

        let to = world.hero.pos - enemy.base.pos;
        let enraged = Self::enraged(enemy);
        match self.step {
            Step::Intro => {
                if self.timer <= 0.0 {
                    self.next(Step::Stalk, 1.2);
                }
            }
            Step::Stalk => {
                if to.length() > 50.0 {
                    enemy.move_by(world, safe_normalize(to) * if enraged { 28.0 } else { 20.0 } * dt, false);
                }
                if self.timer <= 0.0 {
                    self.choose(enemy, world, to);
                }
            }
            Step::BeamWindup => {
                self.beam_dir = rotate(self.beam_dir, self.beam_angle * dt);
                if self.timer <= 0.0 {
                    let from = enemy.base.pos - Vec2::new(0.0, 30.0);
                    world.add(Beam::new(from, from + self.beam_dir * 260.0, self.shield.glow()));
                    let along = (world.hero.center - from).dot(self.beam_dir).clamp(0.0, 260.0);
                    if (from + self.beam_dir * along).distance(world.hero.center) < 12.0 {
                        let at = from + self.beam_dir * (along - 12.0).max(0.0);
                        boss::hit_player(enemy, world, boss::boss_hit(enemy, 1.6, self.shield, at, false));
                    }
                    audio::play(Sound::ChargeLevel, 1.0, -0.6);
                    world.camera.shake(0.2, 2.0);
                    self.next(Step::Recover, 0.9);
                }
            }
            Step::Salvo => {
                if self.timer <= 0.0 {
                    self.next(Step::Recover, 1.0);
                }
            }
            Step::Slam => {
                if self.timer <= 0.0 {
                    world.add(Shockwave::new(enemy.base.pos, 46.0, SLAM_COLOR));
                    world.camera.shake(0.35, 3.0);
                    audio::sfx(Sound::Break);
                    if !world.hero.airborne && to.length() < 48.0 {
                        let hit = boss::boss_hit(enemy, 1.8, Element::None, enemy.base.pos, false);
                        boss::hit_player(enemy, world, hit);
                    }
                    self.next(Step::Recover, 1.2);
                }
            }
            Step::Summon => {
                if self.timer <= 0.0 {
                    if let Some(drone) = data().enemies.get("drone")
                        && world.entities_of::<Enemy>().filter(|e| e.def.id == "drone").count() < 3
                    {
                        for dx in [-30.0, 30.0] {
                            world.add(Enemy::new(drone, enemy.base.pos + Vec2::new(dx, 10.0), Some(enemy.level - 2)));
                        }
                    }
                    audio::play(Sound::BossRoar, 0.6, 0.5);
                    self.next(Step::Recover, 1.0);
                }
            }
            Step::Cracked => {
                if world.rng.next_f32() < 0.3 {
                    let jitter = Vec2::new(world.rng.range(-12, 13) as f32, world.rng.range(-16, 17) as f32);
                    let at = self.hurtbox(enemy).center() + jitter;
                    world.add(Particle::new(at, Vec2::ZERO, Rgba::WHITE, 0.3).spark());
                }
                if self.timer <= 0.0 {
                    self.next(Step::Stalk, 1.0);
                }
            }
            Step::Recover => {
                if self.timer <= 0.0 {
                    self.next(Step::Stalk, if enraged { 0.8 } else { 1.3 });
                }
            }
        }
    }

    fn choose(&mut self, enemy: &Enemy, world: &mut World, to: Vec2) {
        self.cycle += 1;
        let enraged = Self::enraged(enemy);
        if to.length() < 46.0 {
            self.next(Step::Slam, 0.9);
            world.add(GroundMarker::new(enemy.base.pos, 46.0, 0.9, SLAM_COLOR));
            return;
        }
        if enraged && self.cycle % 5 == 0 {
            self.next(Step::Summon, 0.8);
            return;
        }
        if self.cycle % 2 == 0 {
            self.beam_dir = safe_normalize(to);
            self.beam_angle = if world.rng.next_f32() < 0.5 { -1.0 } else { 1.0 } * if enraged { 0.5 } else { 0.3 };
            let from = enemy.base.pos - Vec2::new(0.0, 30.0);
            world.add(LineMarker::new(from, from + self.beam_dir * 260.0, 10.0, 1.0, self.shield.glow()));
            self.next(Step::BeamWindup, 1.0);
            audio::play(Sound::ChargeLevel, 0.8, -0.8);
            return;
        }
        let count = if enraged { 6 } else { 4 };
        for i in 0..count {
            let jitter = if i == 0 {
                Vec2::ZERO
            } else {
                Vec2::new(world.rng.range(-60, 61) as f32, world.rng.range(-40, 41) as f32)
            };
            let spot = world.hero.pos + jitter;
            world.add(StrikeMarker::new(
                spot,
                1.0 + i as f32 * 0.15,
                boss::boss_hit(enemy, 1.3, self.shield, spot, false),
            ));
        }
        audio::play(Sound::ShootFire, 0.7, 0.3);
        self.next(Step::Salvo, 1.4);
    }
}

impl BossBrain for WardenBrain {
    fn added(&mut self, enemy: &mut Enemy, world: &mut World) {
        for (index, (element, _)) in EMITTERS.iter().enumerate() {
            let name = format!("{} emitter", element.label());
            let gem = art::boss_gem(element.glow());
            boss::add_part(enemy, world, index, &name, 0.07, gem, Vec2::new(11.0, 11.0), Some("concord_alloy"));
            self.live.push(*element);
        }
        self.raise_shield(enemy);
    }

    fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.think_step(enemy, world, dt);
    }

    fn draw(&self, enemy: &Enemy, boss: &BossCore, world: &World, batch: &mut Batch) {
        let fade = boss.death_fade();
        let time = boss.time;
        let pos = enemy.base.pos;
        batch.draw(props::shadow(30), snap(pos - Vec2::new(15.0, 5.0)), Rgba::WHITE.mul(fade));
        let texture = art::warden();
        let frame = if matches!(self.step, Step::BeamWindup | Step::Slam) { 1 } else { 0 };
        let flash = (enemy.flashing() || boss.dying()) && (time * 20.0) as i32 % 2 == 0;
        let texture = if flash { gfx::silhouette(texture) } else { texture };
        let bob = if self.step == Step::Stalk { -((time * 5.0).sin().abs() * 1.5).trunc() } else { 0.0 };
        let tint = if self.step == Step::Cracked { Rgba::rgb(200, 200, 230) } else { boss.tint() };
        batch.draw_ex(
            texture,
            snap(pos + Vec2::new(0.0, bob)),
            Some(IRect::new(frame * 40, 0, 40, 50)),
            tint.mul(fade),
            0.0,
            Vec2::new(20.0, 49.0),
            Vec2::ONE,
            world.hero.pos.x < pos.x,
            false,
        );
        if self.shield_up && !boss.dying() {
            let pulse = 0.35 + 0.1 * (time * 6.0).sin();
            let at = snap(pos - Vec2::new(0.0, 24.0));
            let color = self.shield.glow().mul(pulse);
            batch.draw_ex(
                props::barrier_bubble(),
                at,
                None,
                color,
                0.0,
                Vec2::splat(14.0),
                Vec2::splat(1.9),
                false,
                false,
            );
        }
    }

    fn hurtbox(&self, enemy: &Enemy) -> Box2 {
        Box2::new(enemy.base.pos.x - 13.0, enemy.base.pos.y - 44.0, 26.0, 44.0)
    }

    fn death_color(&self) -> Rgba {
        Rgba::rgb(220, 230, 255)
    }

    fn before_hit(&mut self, enemy: &mut Enemy, world: &mut World, hit: &mut Hit) -> Option<bool> {
        if !self.shield_up || !hit.is_player() {
            return None;
        }
        let center = self.hurtbox(enemy).center();
        if hit.element == self.shield.opposite() || hit.element == Element::Aether {
            self.shield_up = false;
            self.shield_timer = 5.0;
            self.next(Step::Cracked, 3.0);
            audio::play(Sound::Break, 1.0, 0.3);
            world.hit_stop(0.1);
            world.add(FloatingText::new(enemy.base.pos - Vec2::new(0.0, 60.0), "SHIELD BROKEN", hit.element.glow()));
            Particle::burst(world, center, self.shield.glow(), 30, 110.0, 0.6, true);
            return None;
        }
        audio::play(Sound::Clink, 0.8, 0.0);
        Particle::burst(world, center, self.shield.glow(), 4, 40.0, 0.2, true);
        // Every projectile is reflectable, so any shot that strikes the shield fizzles.
        if hit.projectile {
            hit.expire_owner = true;
        }
        Some(true)
    }

    fn part_anchor(&self, index: usize, enemy: &Enemy) -> Vec2 {
        let (element, offset) = EMITTERS[index];
        let time = enemy.boss.as_ref().map_or(0.0, |b| b.time);
        enemy.base.pos + offset + Vec2::new(0.0, (time * 2.0 + element as i32 as f32).sin() * 2.0)
    }

    fn part_broken(&mut self, index: usize, enemy: &mut Enemy, world: &mut World) {
        let element = EMITTERS[index].0;
        self.live.retain(|&e| e != element);
        if self.shield == element || self.live.is_empty() {
            self.shield_up = false;
            self.shield_timer = 2.0;
        }
        if self.live.is_empty() {
            world.add(FloatingText::new(
                enemy.base.pos - Vec2::new(0.0, 64.0),
                "Its shield fails!",
                palette::HIGHLIGHT,
            ));
        }
    }
}
