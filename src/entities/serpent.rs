//! The Tidal Serpent: hides beneath the arena's pools and surfaces in one after a ripple warning, then spits and slams
//! before diving. Its fins are parts; without them it lingers on the surface. Lightning in its pool shocks it out.

use bevy::math::Vec2;

use super::boss::{self, BossBrain, BossCore};
use super::combat_effects::{GroundMarker, LineMarker, Shockwave};
use super::effects::{FloatingText, Particle};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use crate::art::{bosses, combat};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, IRect, safe_normalize, snap};
use crate::core::gfx::{self, Batch};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Submerged,
    Rising,
    Surfaced,
    Sinking,
    Shocked,
}

pub struct SerpentBrain {
    pools: [Vec2; 5],
    step: Step,
    timer: f32,
    rise: f32,
    surface_time: f32,
    shock_cooldown: f32,
    pool: usize,
    attack: i32,
    fins: [Option<EntityId>; 2],
    broken: [bool; 2],
}

impl SerpentBrain {
    pub fn new(enemy: &mut Enemy) -> SerpentBrain {
        let pos = enemy.base.pos;
        SerpentBrain {
            pools: [
                pos,
                pos + Vec2::new(-80.0, 24.0),
                pos + Vec2::new(80.0, 24.0),
                pos + Vec2::new(-40.0, 56.0),
                pos + Vec2::new(40.0, 56.0),
            ],
            step: Step::Submerged,
            timer: 1.5,
            rise: 0.0,
            surface_time: 0.0,
            shock_cooldown: 0.0,
            pool: 0,
            attack: 0,
            fins: [None; 2],
            broken: [false; 2],
        }
    }

    fn finless(&self) -> bool {
        self.broken[0] && self.broken[1]
    }

    fn enraged(enemy: &Enemy) -> bool {
        enemy.health_fraction() < 0.5
    }

    fn sync_fins(&self, world: &mut World) {
        let hidden = self.step == Step::Submerged || self.rise < 0.6;
        for fin in self.fins.into_iter().flatten() {
            boss::set_part(world, fin, false, hidden);
        }
    }

    fn think_step(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.timer -= dt;
        self.shock_cooldown = (self.shock_cooldown - dt).max(0.0);
        match self.step {
            Step::Submerged => {
                if self.timer <= 0.0 {
                    self.step = Step::Rising;
                    self.timer = 0.35;
                    audio::play(Sound::Burn, 0.8, -0.4);
                    Particle::burst(world, enemy.base.pos, Rgba::rgb(200, 240, 255), 14, 60.0, 0.5, false);
                }
            }
            Step::Rising => {
                self.rise = 1.0 - self.timer.max(0.0) / 0.35;
                if self.timer <= 0.0 {
                    self.step = Step::Surfaced;
                    self.surface_time =
                        if Self::enraged(enemy) { 2.6 } else { 3.2 } + if self.finless() { 2.5 } else { 0.0 };
                    self.timer = self.surface_time;
                    self.attack = 0;
                }
            }
            Step::Surfaced => self.surfaced(enemy, world),
            Step::Shocked => {
                if world.rng.next_f32() < 0.3 {
                    let jitter = Vec2::new(world.rng.range(-10, 11) as f32, world.rng.range(-14, 15) as f32);
                    let at = self.hurtbox(enemy).center() + jitter;
                    world.add(Particle::new(at, Vec2::ZERO, Rgba::rgb(255, 245, 160), 0.3).spark());
                }
                if self.timer <= 0.0 {
                    self.step = Step::Sinking;
                    self.timer = 0.35;
                }
            }
            Step::Sinking => {
                self.rise = self.timer.max(0.0) / 0.35;
                if self.timer <= 0.0 {
                    let mut next = world.rng.below(self.pools.len() as i32) as usize;
                    while next == self.pool {
                        next = world.rng.below(self.pools.len() as i32) as usize;
                    }
                    self.pool = next;
                    enemy.base.pos = self.pools[next];
                    self.step = Step::Submerged;
                    self.timer = if Self::enraged(enemy) { 0.9 } else { 1.3 };
                }
            }
        }
        self.sync_fins(world);
    }

    fn surfaced(&mut self, enemy: &mut Enemy, world: &mut World) {
        let elapsed = self.surface_time - self.timer;
        let player = world.hero.pos;
        let mouth = enemy.base.pos - Vec2::new(0.0, 30.0);
        let enraged = Self::enraged(enemy);
        if self.attack == 0 && elapsed > 0.2 {
            self.attack = 1;
            let to = mouth + safe_normalize(player - mouth) * 110.0;
            world.add(LineMarker::new(mouth, to, 10.0, 0.45, Rgba::rgb(120, 200, 255)));
        } else if self.attack == 1 && elapsed > 0.65 {
            self.attack = 2;
            let spread = if enraged { 2 } else { 1 };
            for i in -spread..=spread {
                let dir = Vec2::from_angle(i as f32 * 0.22).rotate(player - mouth);
                boss::shoot(enemy, world, "bubble", mouth, dir, 1.0, 1.0);
            }
            audio::play(Sound::ShootWind, 0.7, -0.3);
        } else if self.attack == 2 && elapsed > 1.4 {
            self.attack = 3;
            let hit = boss::boss_hit(enemy, 1.5, Element::Water, player, false);
            world.add(TailSlam::new(player, 0.9, hit, enemy.base.id));
            audio::play(Sound::Burn, 0.7, -0.5);
        } else if self.attack == 3 && elapsed > 2.2 && enraged {
            self.attack = 4;
            let time = enemy.boss.as_ref().map_or(0.0, |b| b.time);
            boss::ring(enemy, world, "bubble", enemy.base.pos - Vec2::new(0.0, 24.0), 12, time, 0.9, 1.0);
            audio::play(Sound::ShootWind, 0.7, -0.5);
        }
        if self.timer <= 0.0 {
            self.step = Step::Sinking;
            self.timer = if self.finless() { 0.8 } else { 0.35 };
        }
    }
}

impl BossBrain for SerpentBrain {
    fn added(&mut self, enemy: &mut Enemy, world: &mut World) {
        let size = Vec2::new(10.0, 12.0);
        let left =
            boss::add_part(enemy, world, 0, "Left fin", 0.12, combat::serpent_fin(false), size, Some("tide_pearl"));
        let right =
            boss::add_part(enemy, world, 1, "Right fin", 0.12, combat::serpent_fin(true), size, Some("tide_pearl"));
        self.fins = [Some(left), Some(right)];
        self.sync_fins(world);
    }

    fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.think_step(enemy, world, dt);
    }

    fn draw(&self, enemy: &Enemy, boss: &BossCore, _world: &World, batch: &mut Batch) {
        let time = boss.time;
        let pos = enemy.base.pos;
        if self.step == Step::Submerged && !boss.dying() {
            if self.timer < 1.0 {
                let src = IRect::new((time * 8.0) as i32 % 4 * 16, 0, 16, 12);
                batch.draw_src(bosses::ripple(), snap(pos - Vec2::new(8.0, 8.0)), Some(src), Rgba::WHITE);
            }
            return;
        }
        let rise = if boss.dying() { boss.death_fade() } else { self.rise };
        if rise <= 0.02 {
            return;
        }
        let visible = (44.0 * rise) as i32;
        let frame = if self.step == Step::Surfaced && (time * 3.0) as i32 % 2 == 0 { 1 } else { 0 };
        let shocked = self.step == Step::Shocked && (time * 16.0) as i32 % 2 == 0;
        let flash = (enemy.flashing() || boss.dying() || shocked) && (time * 20.0) as i32 % 2 == 0;
        let texture = if flash { gfx::silhouette(bosses::serpent()) } else { bosses::serpent() };
        let tint = if self.step == Step::Shocked { Rgba::rgb(255, 250, 190) } else { boss.tint() };
        let at = snap(pos - Vec2::new(16.0, (visible - 2) as f32));
        batch.draw_src(texture, at, Some(IRect::new(frame * 32, 0, 32, visible)), tint);
        let ripple = IRect::new((time * 6.0) as i32 % 2 * 16, 0, 16, 12);
        batch.draw_src(bosses::ripple(), snap(pos - Vec2::new(8.0, 6.0)), Some(ripple), Rgba::WHITE.mul(0.8));
    }

    fn hurtbox(&self, enemy: &Enemy) -> Box2 {
        let pos = enemy.base.pos;
        Box2::new(pos.x - 10.0, pos.y - 6.0 - 36.0 * self.rise, 20.0, 36.0 * self.rise + 2.0)
    }

    fn tangible(&self) -> bool {
        matches!(self.step, Step::Surfaced | Step::Rising | Step::Shocked)
    }

    fn fade(&self) -> f32 {
        if self.step == Step::Submerged { 0.0 } else { self.rise }
    }

    fn death_color(&self) -> Rgba {
        Rgba::rgb(160, 220, 255)
    }

    fn touches_ground(&self) -> bool {
        true
    }

    fn before_hit(&mut self, enemy: &mut Enemy, world: &mut World, hit: &mut Hit) -> Option<bool> {
        let dying = enemy.boss.as_ref().is_some_and(|b| b.dying());
        // Electricity running through the pools reaches the serpent even while it hides beneath them.
        if hit.kind == HitKind::Environment && hit.element == Element::Thunder && self.shock_cooldown <= 0.0 && !dying {
            self.shock_cooldown = 3.0;
            let amount = ((enemy.max_hp as f32 * 0.06) as i32).max(1);
            enemy.damage(world, amount, Element::Thunder, false, " SHOCK");
            audio::play(Sound::BossRoar, 0.7, 0.6);
            world.camera.shake(0.3, 2.0);
            self.step = Step::Shocked;
            self.timer = 2.4;
            self.rise = 1.0;
            self.sync_fins(world);
            return Some(true);
        }
        if self.step == Step::Shocked && hit.is_player() {
            hit.power *= 1.3;
            hit.poise *= 2.0;
            hit.knockback = 0.0;
        }
        if self.finless() && hit.is_player() {
            hit.power *= 1.3;
            hit.knockback = 0.0;
        }
        None
    }

    fn part_anchor(&self, index: usize, enemy: &Enemy) -> Vec2 {
        let x = if index == 0 { 13.0 } else { -13.0 };
        enemy.base.pos - Vec2::new(x, 22.0 * self.rise + 6.0)
    }

    fn part_broken(&mut self, index: usize, enemy: &mut Enemy, world: &mut World) {
        self.broken[index] = true;
        if self.finless() {
            world.add(FloatingText::new(
                enemy.base.pos - Vec2::new(0.0, 50.0),
                "It can barely dive!",
                palette::HIGHLIGHT,
            ));
        }
    }
}

/// A tail crashing down where the player stood, after a splash ring warns of it.
pub struct TailSlam {
    base: EntityBase,
    delay: f32,
    hit: Hit,
    owner: EntityId,
    age: f32,
}

impl TailSlam {
    pub fn new(at: Vec2, delay: f32, hit: Hit, owner: EntityId) -> TailSlam {
        TailSlam { base: EntityBase::at(at), delay, hit, owner, age: 0.0 }
    }
}

impl Entity for TailSlam {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn added(&mut self, world: &mut World) {
        world.add(GroundMarker::new(self.base.pos, 26.0, self.delay, Rgba::rgb(120, 190, 255)));
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if self.age < self.delay {
            return;
        }
        self.base.removed = true;
        let owner_dying = world.get_as::<Enemy>(self.owner).is_none_or(|e| e.boss.as_ref().is_none_or(|b| b.dying()));
        if owner_dying {
            return;
        }
        let pos = self.base.pos;
        world.add(Shockwave::new(pos, 26.0, Rgba::rgb(170, 220, 255)));
        Particle::burst(world, pos, Rgba::rgb(180, 230, 255), 20, 90.0, 0.5, false);
        world.camera.shake(0.2, 2.0);
        audio::play(Sound::Break, 0.7, -0.2);
        if !world.hero.airborne && world.hero.pos.distance(pos) < 28.0 {
            let mut hit = self.hit.clone();
            world.hit_player(&mut hit);
            if hit.parried {
                world.with_as::<Enemy, _>(self.owner, |e, w| e.parried(w));
            }
        }
    }

    fn draw(&self, _world: &World, _batch: &mut Batch) {}
}
