//! The golden rift at the heart of a Trial: stepping near it opens one great breach that pours out escalating waves,
//! the last led by a Rift Champion. The breach holds the hero in: outside its edge their life drains, and if they
//! stay out too long it collapses and every wave must be fought again. Clearing it sets `golden:id`.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::effects::{FloatingText, Particle, Puff};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use crate::art::combat as art_combat;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::{self, Batch};
use crate::entity_base;
use crate::gameplay::game_data::data;
use crate::gameplay::trial_mods::TrialMod;
use crate::scenes::world::World;
use crate::world::map::{EntityData, tile_center};

const GOLD: Rgba = Rgba::rgb(255, 214, 110);
/// How far the breach reaches; leaving it starts the drain.
pub const RADIUS: f32 = 150.0;
const SPAWN_GAP: f32 = 0.1;
const WAVE_PAUSE: f32 = 2.5;
/// Share of max life lost each second outside the breach.
const DRAIN: f32 = 0.05;
/// Seconds outside before the breach collapses.
const COLLAPSE: f32 = 6.0;

/// One wave: plain foes, elites, and whether the champion leads it.
struct Wave {
    foes: i32,
    elites: i32,
    champion: bool,
}

const WAVES: [Wave; 4] = [
    Wave { foes: 8, elites: 0, champion: false },
    Wave { foes: 10, elites: 1, champion: false },
    Wave { foes: 12, elites: 2, champion: false },
    Wave { foes: 6, elites: 0, champion: true },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Dormant,
    Active,
    Done,
}

pub struct GoldenBreach {
    base: EntityBase,
    id: String,
    kinds: Vec<String>,
    level: i32,
    champion: String,
    state: State,
    wave: usize,
    /// Foes of the current wave still to pour out: (kind, elite, champion).
    queue: Vec<(String, bool, bool)>,
    spawned: Vec<EntityId>,
    spawn_timer: f32,
    pause: f32,
    outside: f32,
    drain: f32,
    time: f32,
}

impl GoldenBreach {
    pub fn new(data: &EntityData) -> GoldenBreach {
        GoldenBreach {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 8.0),
                width: 20.0,
                height: 12.0,
                ..EntityBase::default()
            },
            id: data.str_or("id", "golden").to_string(),
            kinds: data.str_or("kinds", "slime").split(',').map(|s| s.trim().to_string()).collect(),
            level: data.int("level", 1),
            champion: data.str_or("champion", "brute").to_string(),
            state: State::Dormant,
            wave: 0,
            queue: Vec::new(),
            spawned: Vec::new(),
            spawn_timer: 0.0,
            pause: 0.0,
            outside: 0.0,
            drain: 0.0,
            time: 0.0,
        }
    }

    pub fn flag(&self) -> String {
        format!("golden:{}", self.id)
    }

    /// The wave counter for the HUD: (wave, waves, foes left), while the breach is open.
    pub fn progress(&self) -> Option<(usize, usize, i32)> {
        (self.state == State::Active)
            .then(|| (self.wave + 1, WAVES.len(), self.queue.len() as i32 + self.spawned.len() as i32))
    }

    /// Seconds left before the breach collapses, while the hero is outside it.
    pub fn collapse_in(&self) -> Option<f32> {
        (self.state == State::Active && self.outside > 0.0).then_some(COLLAPSE - self.outside)
    }

    fn fill_wave(&mut self, world: &World) {
        let wave = &WAVES[self.wave];
        let pick = |w: &World| self.kinds[w.rng.below(self.kinds.len() as i32) as usize].clone();
        let mut queue = Vec::new();
        if wave.champion {
            queue.push((self.champion.clone(), false, true));
        }
        for _ in 0..wave.elites {
            queue.push((pick(world), true, false));
        }
        for i in 0..wave.foes {
            let tide = world.trial_mod(TrialMod::EliteTide) && i % 4 == 3;
            queue.push((pick(world), tide, false));
        }
        queue.reverse();
        self.queue = queue;
        self.spawn_timer = 0.4;
    }

    fn open(&mut self, world: &mut World) {
        self.state = State::Active;
        self.wave = 0;
        self.outside = 0.0;
        self.fill_wave(world);
        audio::play(Sound::BossRoar, 0.8, 0.2);
        world.camera.shake(0.5, 2.0);
        world.add(FloatingText::new(self.base.pos - Vec2::new(0.0, 44.0), "The golden breach opens!", GOLD));
        world.toast("Stay inside the breach until every wave falls.");
    }

    fn spawn_next(&mut self, world: &mut World) {
        let Some((kind, elite, champion)) = self.queue.pop() else { return };
        let Some(def) = data().enemies.get(&kind) else { return };
        let angle = world.rng.next_f32() * TAU;
        let reach = if champion { 0.0 } else { RADIUS * (0.35 + world.rng.next_f32() * 0.5) };
        let at = world.nearest_open(self.base.pos + Vec2::new(angle.cos() * reach, angle.sin() * reach * 0.7));
        let level = Some(self.level + self.wave as i32);
        let enemy = if champion {
            Enemy::champion(def, at, level, world, format!("champion:{}", self.id))
        } else if elite {
            Enemy::elite(def, at, level, world)
        } else {
            Enemy::new(def, at, level)
        };
        let id = world.add(enemy.hunting());
        self.spawned.push(id);
        world.add(Puff::new(at - Vec2::new(0.0, 6.0), GOLD));
        audio::play(Sound::Door, 0.4, 0.7);
    }

    /// Life drains outside the breach; staying out too long collapses it and sends every wave back.
    fn hold_in(&mut self, world: &mut World, dt: f32) {
        if world.hero.pos.distance(self.base.pos) <= RADIUS + 6.0 {
            self.outside = 0.0;
            return;
        }
        if self.outside == 0.0 {
            world.add(FloatingText::new(world.hero.pos - Vec2::new(0.0, 24.0), "The breach pulls at you!", GOLD));
        }
        self.outside += dt;
        self.drain += world.state.max_hp() as f32 * DRAIN * dt;
        let whole = self.drain as i32;
        if whole > 0 {
            self.drain -= whole as f32;
            world.state.hp = (world.state.hp - whole).max(1);
        }
        if self.outside >= COLLAPSE {
            self.collapse(world);
        }
    }

    fn collapse(&mut self, world: &mut World) {
        for id in self.spawned.drain(..) {
            world.with_entity(id, |e, _| e.base_mut().removed = true);
        }
        self.queue.clear();
        self.state = State::Dormant;
        self.outside = 0.0;
        audio::play(Sound::Break, 0.9, -0.3);
        world.camera.shake(0.4, 2.0);
        world.toast("The breach collapses. Step back in to face it again.");
    }

    fn finish(&mut self, world: &mut World) {
        self.state = State::Done;
        world.state.flags.insert(self.flag());
        world.flash(GOLD, 0.5);
        world.camera.shake(0.5, 2.0);
        world.hit_stop(0.15);
        audio::play(Sound::BossDie, 0.7, 0.3);
        Particle::burst(world, self.base.pos - Vec2::new(0.0, 14.0), GOLD, 50, 120.0, 1.0, true);
        world.toast("The golden breach is sealed. Its treasures are yours.");
    }
}

impl Entity for GoldenBreach {
    entity_base!();

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        (self.state != State::Done).then_some((90.0, GOLD))
    }

    fn added(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag()) {
            self.state = State::Done;
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        match self.state {
            State::Dormant if world.hero.pos.distance(self.base.pos) < RADIUS * 0.6 => self.open(world),
            State::Active => {
                self.spawned.retain(|&id| world.get_as::<Enemy>(id).is_some_and(|e| !e.base.removed && e.hp > 0));
                self.hold_in(world, dt);
                if self.state != State::Active {
                    return;
                }
                if !self.queue.is_empty() {
                    self.spawn_timer -= dt;
                    if self.spawn_timer <= 0.0 {
                        self.spawn_timer = SPAWN_GAP;
                        self.spawn_next(world);
                    }
                } else if self.spawned.is_empty() {
                    if self.wave + 1 >= WAVES.len() {
                        self.finish(world);
                    } else if self.pause <= 0.0 {
                        self.pause = WAVE_PAUSE;
                        let text = format!("Wave {} of {}", self.wave + 2, WAVES.len());
                        world.add(FloatingText::new(self.base.pos - Vec2::new(0.0, 44.0), text, GOLD));
                    } else {
                        self.pause -= dt;
                        if self.pause <= 0.0 {
                            self.wave += 1;
                            self.fill_wave(world);
                        }
                    }
                }
            }
            _ => {}
        }
        if self.state != State::Done && world.rng.next_f32() < 0.35 {
            let a = world.rng.next_f32() * TAU;
            let center = self.base.pos - Vec2::new(0.0, 14.0);
            let at = center + Vec2::new(a.cos(), a.sin() * 0.6) * 30.0;
            world.add(Particle::new(at, (center - at) * 1.5, GOLD, 0.6).spark());
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        if self.state == State::Active {
            let disc = art_combat::disc();
            let (dw, _) = gfx::size(disc);
            let scale = Vec2::new(RADIUS * 2.0 / dw as f32, RADIUS * 1.4 / dw as f32);
            batch.draw_ex(
                disc,
                self.base.pos,
                None,
                GOLD.mul(0.06),
                0.0,
                Vec2::splat(dw as f32 / 2.0),
                scale,
                false,
                false,
            );
            let ring = art_combat::ring();
            let (rw, _) = gfx::size(ring);
            let pulse = 0.6 + (self.time * 4.0).sin() * 0.25;
            let ring_scale = scale * dw as f32 / rw as f32;
            batch.draw_ex(
                ring,
                self.base.pos,
                None,
                GOLD.mul(pulse),
                0.0,
                Vec2::splat(rw as f32 / 2.0),
                ring_scale,
                false,
                false,
            );
        }
        if self.state == State::Done {
            return;
        }
        let center = self.base.pos - Vec2::new(0.0, 16.0);
        let frame = (self.time * 8.0) as i32 % 4;
        let size = if self.state == State::Active { 1.5 } else { 1.1 } * (1.0 + (self.time * 3.0).sin() * 0.06);
        batch.draw_ex(
            art_combat::trial_portal(),
            snap(center),
            Some(IRect::new(frame * 32, 0, 32, 40)),
            Rgba::WHITE,
            0.0,
            Vec2::new(16.0, 20.0),
            Vec2::splat(size),
            false,
            false,
        );
    }
}
