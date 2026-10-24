//! The First Concord, the Hollow's superboss: the original binding of the elements, rotted by the blight. It attunes to
//! one element at a time, raises four seals at two thirds, and in its last third only the Aether can touch it.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::boss::{self, BossBrain, BossCore};
use super::combat_effects::{Beam, GroundMarker, LineMarker};
use super::effects::{FloatingText, Particle};
use super::enemy::Enemy;
use super::entity::{EntityBase, EntityId};
use super::golem::StrikeMarker;
use super::warlock::EmberPillar;
use crate::art::{bosses, combat as combat_art, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, rotate, safe_normalize, snap};
use crate::core::gfx::{self, Batch};
use crate::gameplay::combat::Hit;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::data;
use crate::scenes::world::World;

const CYCLE: [Element; 4] = [Element::Fire, Element::Water, Element::Wind, Element::Thunder];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Attuned,
    Orbs,
    Unbound,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Drift,
    Windup,
    Beam,
    Recover,
    Shift,
}

pub struct ConcordBrain {
    orbs: Vec<(Element, EntityId)>,
    phase: Phase,
    step: Step,
    timer: f32,
    attune_timer: f32,
    beam_spin: f32,
    attune: usize,
    cycle: i32,
    beam_dir: Vec2,
}

impl ConcordBrain {
    pub fn new(enemy: &mut Enemy) -> ConcordBrain {
        enemy.base.width = 30.0;
        enemy.base.height = 12.0;
        ConcordBrain {
            orbs: Vec::new(),
            phase: Phase::Attuned,
            step: Step::Shift,
            timer: 2.0,
            attune_timer: 9.0,
            beam_spin: 0.0,
            attune: 0,
            cycle: 0,
            beam_dir: Vec2::ZERO,
        }
    }

    fn attuned(&self) -> Element {
        if self.phase == Phase::Unbound { Element::Aether } else { CYCLE[self.attune % CYCLE.len()] }
    }

    fn core(enemy: &Enemy) -> Vec2 {
        enemy.base.pos - Vec2::new(0.0, 26.0 + enemy.z)
    }

    fn time(enemy: &Enemy) -> f32 {
        enemy.boss.as_ref().map_or(0.0, |b| b.time)
    }

    fn enraged(&self, enemy: &Enemy) -> bool {
        self.phase == Phase::Unbound && enemy.raw_health_fraction() < 0.15
    }

    fn orb_anchor(index: usize, enemy: &Enemy) -> Vec2 {
        let a = Self::time(enemy) * 0.9 + index as f32 * TAU / 4.0;
        Self::core(enemy) + Vec2::new(a.cos() * 46.0, a.sin() * 30.0)
    }

    fn any_orb_left(&self, world: &World) -> bool {
        self.orbs.iter().any(|&(_, part)| !boss::part_broken(world, part))
    }

    fn next(&mut self, step: Step, time: f32) {
        self.step = step;
        self.timer = time;
    }

    fn attune_to(&mut self, enemy: &Enemy, world: &mut World, index: usize) {
        self.attune = index;
        self.attune_timer = 9.0;
        let (core, attuned) = (Self::core(enemy), self.attuned());
        audio::play(Sound::ChargeLevel, 0.8, 0.1);
        Particle::burst(world, core, attuned.glow(), 20, 80.0, 0.5, true);
        world.add(FloatingText::new(
            core - Vec2::new(0.0, 24.0),
            format!("Attuned: {}", attuned.label()),
            attuned.glow(),
        ));
    }

    fn raise_orbs(&mut self, enemy: &Enemy, world: &mut World) {
        self.phase = Phase::Orbs;
        self.next(Step::Shift, 2.0);
        world.flash(Rgba::WHITE, 0.4);
        world.camera.shake(1.0, 2.0);
        audio::sfx(Sound::BossRoar);
        world.show_message("The First Concord: WE BOUND YOU ONCE. WE WILL BIND YOU AGAIN.");
        for (index, element) in CYCLE.into_iter().enumerate() {
            let name = format!("{} seal", element.label());
            let art = bosses::element_orb(element.glow());
            let part =
                boss::add_part(enemy, world, index, &name, 0.06, art, Vec2::new(14.0, 14.0), Some("blight_core"));
            self.orbs.push((element, part));
        }
    }

    fn unbind(&mut self, world: &mut World) {
        self.phase = Phase::Unbound;
        self.next(Step::Shift, 2.5);
        world.flash(Rgba::rgb(240, 200, 255), 0.8);
        world.camera.shake(1.5, 3.0);
        audio::play(Sound::BossRoar, 1.0, -0.3);
        world.show_message("The First Concord: THE ELEMENTS ARE OURS NO LONGER. THEN LET ALL OF IT ROT.");
    }

    fn choose(&mut self, enemy: &mut Enemy, world: &mut World, to: Vec2) {
        self.cycle += 1;
        let (pos, core, attuned) = (enemy.base.pos, Self::core(enemy), self.attuned());
        if to.length() < 40.0 && self.phase != Phase::Unbound {
            world.add(GroundMarker::new(pos, 52.0, 0.9, attuned.glow()));
            world.add(StrikeMarker::new(pos, 0.9, boss::boss_hit(enemy, 1.6, attuned, pos, false)));
            self.next(Step::Windup, 1.0);
            return;
        }
        match self.phase {
            Phase::Attuned => self.element_attack(enemy, world),
            Phase::Orbs => {
                if self.cycle % 2 == 0 {
                    self.start_beam(enemy, world, to, 0.35);
                } else {
                    for (index, &(element, part)) in self.orbs.iter().enumerate() {
                        if !boss::part_broken(world, part) {
                            let from = Self::orb_anchor(index, enemy);
                            boss::shoot(enemy, world, shot_for(element), from, world.hero.center - core, 1.2, 1.0);
                        }
                    }
                }
                if self.step == Step::Drift {
                    self.next(Step::Windup, 0.8);
                }
            }
            Phase::Unbound => {
                let enraged = self.enraged(enemy);
                if self.cycle % 3 == 0 {
                    self.start_beam(enemy, world, to, if enraged { 0.7 } else { 0.5 });
                } else if self.cycle % 3 == 1 {
                    self.blight_burst(enemy, world);
                } else {
                    let time = Self::time(enemy);
                    boss::ring(enemy, world, "aether", core, if enraged { 18 } else { 14 }, time, 0.9, 1.0);
                    boss::ring(enemy, world, "dark", core, 10, time + 0.2, 0.7, 1.0);
                    audio::play(Sound::ShootFire, 0.8, -0.3);
                    self.next(Step::Windup, 1.0);
                }
            }
        }
    }

    fn element_attack(&mut self, enemy: &Enemy, world: &mut World) {
        let (player, center, core, time) = (world.hero.pos, world.hero.center, Self::core(enemy), Self::time(enemy));
        match self.attuned() {
            Element::Fire => {
                for i in 0..5 {
                    let jitter = if i == 0 {
                        Vec2::ZERO
                    } else {
                        let x = world.rng.range(-56, 57) as f32;
                        Vec2::new(x, world.rng.range(-40, 41) as f32)
                    };
                    let spot = player + jitter;
                    let hit = boss::boss_hit(enemy, 1.4, Element::Fire, spot, false);
                    world.add(EmberPillar::new(spot, 0.9 + i as f32 * 0.12, hit));
                }
            }
            Element::Water => {
                boss::ring(enemy, world, "ice", core, 14, time, 0.7, 1.0);
                boss::ring(enemy, world, "bubble", core, 7, time + 0.3, 0.5, 1.0);
            }
            Element::Wind => {
                for i in -2..=2 {
                    boss::shoot(enemy, world, "gust", core, rotate(center - core, i as f32 * 0.25), 1.3, 1.0);
                }
            }
            _ => {
                for y in -1i32..=1 {
                    for x in -1i32..=1 {
                        let spot = player + Vec2::new(x as f32 * 28.0, y as f32 * 24.0);
                        let delay = 1.0 + (x.abs() + y.abs()) as f32 * 0.2;
                        let hit = boss::boss_hit(enemy, 1.3, Element::Thunder, spot, false);
                        world.add(StrikeMarker::new(spot, delay, hit));
                    }
                }
            }
        }
        audio::play(Sound::ShootFire, 0.7, 0.2);
        self.next(Step::Windup, 1.4);
    }

    fn start_beam(&mut self, enemy: &Enemy, world: &mut World, to: Vec2, spin: f32) {
        let core = Self::core(enemy);
        self.beam_dir = safe_normalize(to);
        self.beam_spin = if world.rng.next_f32() < 0.5 { -spin } else { spin };
        world.add(LineMarker::new(core, core + self.beam_dir * 280.0, 10.0, 1.0, self.attuned().glow()));
        audio::play(Sound::ChargeLevel, 0.8, -0.8);
        self.next(Step::Beam, 1.0);
    }

    fn blight_burst(&mut self, enemy: &Enemy, world: &mut World) {
        let (tx, ty) = EntityBase::at(world.hero.pos).tile_pos();
        world.add(GroundMarker::new(world.hero.pos, 30.0, 0.8, Rgba::rgb(170, 80, 220)));
        for y in -1..=1 {
            for x in -1..=1 {
                world.chem_blight(tx + x, ty + y);
            }
        }
        let cap = if self.enraged(enemy) { 4 } else { 2 };
        if let Some(mote) = data().enemies.get("voidmote")
            && world.entities_of::<Enemy>().filter(|e| e.def.id == "voidmote").count() < cap
        {
            let at = enemy.base.pos + Vec2::new(world.rng.range(-40, 41) as f32, 16.0);
            world.add(Enemy::new(mote, at, Some(enemy.level - 4)));
        }
        audio::play(Sound::BossRoar, 0.5, 0.6);
        self.next(Step::Windup, 1.2);
    }
}

impl BossBrain for ConcordBrain {
    fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.timer -= dt;
        let time = Self::time(enemy);
        enemy.z = 6.0 + (time * 1.6).sin() * 2.0;
        let to = world.hero.pos - enemy.base.pos;

        if self.phase == Phase::Attuned {
            self.attune_timer -= dt;
            if self.attune_timer <= 0.0 && self.step != Step::Windup {
                self.attune_to(enemy, world, self.attune + 1);
            }
            if enemy.raw_health_fraction() < 0.66 {
                self.raise_orbs(enemy, world);
            }
        } else if self.phase == Phase::Orbs && !self.any_orb_left(world) {
            self.unbind(world);
        }

        match self.step {
            Step::Shift => {
                if self.timer <= 0.0 {
                    self.next(Step::Drift, 1.4);
                }
            }
            Step::Drift => {
                let home = enemy.boss.as_ref().map_or(enemy.base.pos, |b| b.home);
                let target = home + Vec2::new((time * 0.7).sin() * 50.0, (time * 0.45).sin() * 20.0);
                if target.distance(enemy.base.pos) > 2.0 {
                    enemy.move_by(world, safe_normalize(target - enemy.base.pos) * 26.0 * dt, true);
                }
                if self.timer <= 0.0 {
                    self.choose(enemy, world, to);
                }
            }
            Step::Windup => {
                if self.timer <= 0.0 {
                    self.next(Step::Recover, 0.8);
                }
            }
            Step::Beam => {
                self.beam_dir = rotate(self.beam_dir, self.beam_spin * dt);
                if self.timer <= 0.0 {
                    let (core, attuned, center) = (Self::core(enemy), self.attuned(), world.hero.center);
                    world.add(Beam::new(core, core + self.beam_dir * 280.0, attuned.glow()));
                    let along = (center - core).dot(self.beam_dir).clamp(0.0, 280.0);
                    if (core + self.beam_dir * along).distance(center) < 12.0 {
                        let from = core + self.beam_dir * (along - 12.0).max(0.0);
                        let hit = boss::boss_hit(enemy, 1.8, attuned, from, false);
                        boss::hit_player(enemy, world, hit);
                    }
                    audio::play(Sound::ChargeLevel, 1.0, -0.6);
                    world.camera.shake(0.25, 2.0);
                    self.next(Step::Recover, 0.9);
                }
            }
            Step::Recover => {
                if self.timer <= 0.0 {
                    let wait = if self.enraged(enemy) {
                        0.6
                    } else if self.phase == Phase::Unbound {
                        0.9
                    } else {
                        1.3
                    };
                    self.next(Step::Drift, wait);
                }
            }
        }
    }

    fn draw(&self, enemy: &Enemy, boss: &BossCore, _world: &World, batch: &mut Batch) {
        let fade = boss.death_fade();
        let time = boss.time;
        batch.draw(props::shadow(36), snap(enemy.base.pos - Vec2::new(18.0, 5.0)), Rgba::WHITE.mul(0.7 * fade));
        let flash = (enemy.flashing() || boss.dying()) && (time * 20.0) as i32 % 2 == 0;
        let core = snap(Self::core(enemy));
        let glow = self.attuned().glow();
        let pulse = 0.35 + 0.15 * (time * 5.0).sin();
        let bubble = props::barrier_bubble();
        batch.draw_ex(
            bubble,
            core,
            None,
            glow.mul(pulse * fade),
            0.0,
            Vec2::splat(14.0),
            Vec2::splat(1.4),
            false,
            false,
        );
        let ring = bosses::concord();
        let ring = if flash { gfx::silhouette(ring) } else { ring };
        let tilt = (time * 0.5).sin() * 0.15;
        batch.draw_ex(ring, core, None, boss.tint().mul(fade), tilt, Vec2::splat(28.0), Vec2::ONE, false, false);
        let gem = combat_art::boss_gem(if self.phase == Phase::Unbound { Rgba::rgb(200, 110, 255) } else { glow });
        let (w, h) = gfx::size(gem);
        let origin = Vec2::new(w as f32 / 2.0, h as f32 / 2.0);
        let scale = Vec2::splat(1.8 + 0.15 * (time * 6.0).sin());
        batch.draw_ex(gem, core, None, Rgba::WHITE.mul(fade), 0.0, origin, scale, false, false);
    }

    fn hurtbox(&self, enemy: &Enemy) -> Box2 {
        Box2::new(enemy.base.pos.x - 18.0, enemy.base.pos.y - 52.0, 36.0, 48.0)
    }

    fn death_color(&self) -> Rgba {
        Rgba::rgb(255, 220, 255)
    }

    fn death_duration(&self) -> f32 {
        3.5
    }

    fn before_hit(&mut self, enemy: &mut Enemy, world: &mut World, hit: &mut Hit) -> Option<bool> {
        if !hit.is_player() || enemy.boss.as_ref().is_some_and(|b| b.dying()) {
            return None;
        }
        let attuned = self.attuned();
        let sealed = match self.phase {
            Phase::Attuned => false,
            Phase::Orbs => self.any_orb_left(world),
            Phase::Unbound => !hit.aether,
        };
        if sealed {
            if self.phase == Phase::Unbound {
                world.feed_aether(6.0);
            }
            audio::play(Sound::Clink, 0.8, 0.4);
            Particle::burst(world, Self::core(enemy), attuned.glow(), 4, 40.0, 0.2, true);
            if hit.projectile {
                hit.expire_owner = true;
            }
            return Some(true);
        }
        let scale = if self.phase != Phase::Attuned {
            1.0
        } else if hit.element == attuned.opposite() || hit.aether {
            1.5
        } else if hit.element == attuned {
            0.1
        } else {
            0.5
        };
        if scale < 1.0 {
            audio::play(Sound::Clink, 0.5, -0.2);
        }
        hit.power *= scale;
        hit.knockback = 0.0;
        None
    }

    fn on_die(&mut self, enemy: &mut Enemy, _world: &mut World) -> bool {
        // Every phase must be fought; a huge hit early on leaves it at the next threshold instead of killing it.
        if self.phase == Phase::Unbound {
            return false;
        }
        enemy.hp = ((enemy.max_hp as f32 * 0.3) as i32).max(1);
        true
    }

    fn part_anchor(&self, index: usize, enemy: &Enemy) -> Vec2 {
        Self::orb_anchor(index, enemy)
    }
}

fn shot_for(element: Element) -> &'static str {
    match element {
        Element::Fire => "fire",
        Element::Water => "ice",
        Element::Thunder => "spark",
        Element::Wind => "gust",
        _ => "aether",
    }
}
