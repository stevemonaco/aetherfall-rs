//! The Magma Wyrm, Cinderfall's optional boss. It swims through the caldera's lava and surfaces at its vents. Its crust
//! plates shrug off blows until Water cools them; a cooled wyrm stays up, steaming, exposed and with breakable plates.

use bevy::math::Vec2;

use super::boss::{self, BossBrain, BossCore};
use super::combat_effects::LineMarker;
use super::effects::{FloatingText, Particle};
use super::enemy::Enemy;
use super::entity::EntityId;
use super::hazards::SteamCloud;
use super::warlock::EmberPillar;
use crate::art::bosses as art;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, IRect, rotate, safe_normalize, snap};
use crate::core::gfx::{Batch, FrameStrip};
use crate::core::palette;
use crate::gameplay::combat::Hit;
use crate::gameplay::element::Element;
use crate::scenes::world::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Submerged,
    Rising,
    Surfaced,
    Sinking,
    Cooled,
}

const PLATE_OFFSETS: [Vec2; 3] = [Vec2::new(-9.0, 30.0), Vec2::new(9.0, 22.0), Vec2::new(-2.0, 12.0)];

pub struct WyrmBrain {
    vents: [Vec2; 5],
    /// Each crust plate's part and whether it has broken.
    plates: Vec<(EntityId, bool)>,
    step: Step,
    timer: f32,
    rise: f32,
    surface_time: f32,
    chill: f32,
    vent: usize,
    attack: i32,
}

impl WyrmBrain {
    pub fn new(enemy: &mut Enemy) -> WyrmBrain {
        let pos = enemy.base.pos;
        WyrmBrain {
            vents: [
                pos,
                pos + Vec2::new(-88.0, 30.0),
                pos + Vec2::new(88.0, 30.0),
                pos + Vec2::new(-48.0, 70.0),
                pos + Vec2::new(48.0, 70.0),
            ],
            plates: Vec::new(),
            step: Step::Submerged,
            timer: 1.5,
            rise: 0.0,
            surface_time: 0.0,
            chill: 0.0,
            vent: 0,
            attack: 0,
        }
    }

    fn strip() -> FrameStrip {
        FrameStrip::new(art::wyrm(), 32, 44)
    }

    fn armored(&self) -> bool {
        self.plates.iter().any(|&(_, broken)| !broken)
    }

    fn enraged(enemy: &Enemy) -> bool {
        enemy.health_fraction() < 0.5
    }

    fn sync_plates(&self, world: &mut World) {
        let hidden = self.step == Step::Submerged || self.rise < 0.6;
        for &(plate, _) in &self.plates {
            boss::set_part(world, plate, self.step != Step::Cooled, hidden);
        }
    }

    fn cool(&mut self, enemy: &Enemy, world: &mut World) {
        self.chill = 0.0;
        self.step = Step::Cooled;
        self.timer = 4.5;
        self.rise = 1.0;
        audio::play(Sound::Burn, 1.0, -0.5);
        world.add(SteamCloud::new(enemy.base.pos - Vec2::new(0.0, 10.0), false));
        world.add(FloatingText::new(enemy.base.pos - Vec2::new(0.0, 56.0), "COOLED", Rgba::rgb(150, 220, 255)));
        world.camera.shake(0.25, 2.0);
        self.sync_plates(world);
    }

    fn think_step(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.timer -= dt;
        self.chill = (self.chill - dt * 0.3).max(0.0);
        match self.step {
            Step::Submerged => {
                if self.timer <= 0.0 {
                    self.step = Step::Rising;
                    self.timer = 0.4;
                    audio::play(Sound::Burn, 0.9, -0.6);
                    Particle::burst(world, enemy.base.pos, Rgba::rgb(255, 150, 60), 16, 70.0, 0.5, true);
                }
            }
            Step::Rising => {
                self.rise = 1.0 - self.timer.max(0.0) / 0.4;
                if self.timer <= 0.0 {
                    self.step = Step::Surfaced;
                    self.surface_time = if Self::enraged(enemy) { 3.4 } else { 4.0 };
                    self.timer = self.surface_time;
                    self.attack = 0;
                }
            }
            Step::Surfaced => self.surfaced(enemy, world),
            Step::Cooled => {
                if world.rng.next_f32() < 0.2 {
                    let at = self.hurtbox(enemy).center() + Vec2::new(world.rng.range(-10, 11) as f32, -10.0);
                    world.add(Particle::new(at, Vec2::new(0.0, -20.0), Rgba::rgb(220, 220, 230), 0.6));
                }
                if self.timer <= 0.0 {
                    self.step = Step::Sinking;
                    self.timer = 0.4;
                }
            }
            Step::Sinking => {
                self.rise = self.timer.max(0.0) / 0.4;
                if self.timer <= 0.0 {
                    let mut next = world.rng.below(self.vents.len() as i32) as usize;
                    while next == self.vent {
                        next = world.rng.below(self.vents.len() as i32) as usize;
                    }
                    self.vent = next;
                    enemy.base.pos = self.vents[next];
                    self.step = Step::Submerged;
                    self.timer = if Self::enraged(enemy) { 0.9 } else { 1.4 };
                }
            }
        }
        self.sync_plates(world);
    }

    fn surfaced(&mut self, enemy: &Enemy, world: &mut World) {
        let elapsed = self.surface_time - self.timer;
        let player = world.hero.pos;
        let mouth = enemy.base.pos - Vec2::new(0.0, 30.0);
        let enraged = Self::enraged(enemy);
        if self.attack == 0 && elapsed > 0.3 {
            self.attack = 1;
            let to = mouth + safe_normalize(player - mouth) * 120.0;
            world.add(LineMarker::new(mouth, to, 10.0, 0.5, Rgba::rgb(255, 140, 60)));
        } else if self.attack == 1 && elapsed > 0.8 {
            self.attack = 2;
            let spread = if enraged { 2 } else { 1 };
            for i in -spread..=spread {
                boss::shoot(enemy, world, "fire", mouth, rotate(player - mouth, i as f32 * 0.25), 1.0, 1.0);
            }
            audio::play(Sound::ShootFire, 0.8, -0.3);
        } else if self.attack == 2 && elapsed > 1.6 {
            self.attack = 3;
            let count = if enraged { 5 } else { 3 };
            for i in 0..count {
                let jitter = if i == 0 {
                    Vec2::ZERO
                } else {
                    Vec2::new(world.rng.range(-56, 57) as f32, world.rng.range(-36, 37) as f32)
                };
                let spot = player + jitter;
                let hit = boss::boss_hit(enemy, 1.3, Element::Fire, spot, false);
                world.add(EmberPillar::new(spot, 0.9 + i as f32 * 0.12, hit));
            }
        } else if self.attack == 3 && elapsed > 2.6 && enraged {
            self.attack = 4;
            let time = enemy.boss.as_ref().map_or(0.0, |b| b.time);
            boss::ring(enemy, world, "fire", enemy.base.pos - Vec2::new(0.0, 20.0), 10, time, 0.8, 1.0);
            audio::play(Sound::ShootFire, 0.8, -0.5);
        }
        if self.timer <= 0.0 {
            self.step = Step::Sinking;
            self.timer = 0.4;
        }
    }
}

impl BossBrain for WyrmBrain {
    fn added(&mut self, enemy: &mut Enemy, world: &mut World) {
        for index in 0..PLATE_OFFSETS.len() {
            let plate = art::wyrm_plate();
            let id = boss::add_part(
                enemy,
                world,
                index,
                "Crust plate",
                0.08,
                plate,
                Vec2::new(12.0, 9.0),
                Some("ember_shard"),
            );
            self.plates.push((id, false));
        }
        self.sync_plates(world);
    }

    fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.think_step(enemy, world, dt);
    }

    fn draw(&self, enemy: &Enemy, boss: &BossCore, _world: &World, batch: &mut Batch) {
        let time = boss.time;
        let pos = enemy.base.pos;
        if self.step == Step::Submerged && !boss.dying() {
            if self.timer < 1.1 {
                let src = IRect::new((time * 8.0) as i32 % 4 * 16, 0, 16, 12);
                batch.draw_src(art::ripple(), snap(pos - Vec2::new(8.0, 8.0)), Some(src), Rgba::rgb(255, 160, 80));
            }
            return;
        }
        let rise = if boss.dying() { boss.death_fade() } else { self.rise };
        if rise <= 0.02 {
            return;
        }
        let visible = (44.0 * rise) as i32;
        let frame = if self.step == Step::Surfaced && (time * 3.0) as i32 % 2 == 0 { 1 } else { 0 };
        let flash = (enemy.flashing() || boss.dying()) && (time * 20.0) as i32 % 2 == 0;
        let strip = Self::strip();
        let texture = if flash { strip.silhouette() } else { strip.texture };
        let tint = if self.step == Step::Cooled { Rgba::rgb(150, 150, 170) } else { boss.tint() };
        let at = snap(pos - Vec2::new(16.0, visible as f32 - 2.0));
        batch.draw_src(texture, at, Some(IRect::new(frame * 32, 0, 32, visible)), tint);
        let src = IRect::new((time * 6.0) as i32 % 2 * 16, 0, 16, 12);
        batch.draw_src(art::ripple(), snap(pos - Vec2::new(8.0, 6.0)), Some(src), Rgba::rgb(255, 170, 90).mul(0.8));
    }

    fn hurtbox(&self, enemy: &Enemy) -> Box2 {
        let pos = enemy.base.pos;
        Box2::new(pos.x - 10.0, pos.y - 6.0 - 36.0 * self.rise, 20.0, 36.0 * self.rise + 2.0)
    }

    fn tangible(&self) -> bool {
        matches!(self.step, Step::Surfaced | Step::Rising | Step::Cooled)
    }

    fn fade(&self) -> f32 {
        if self.step == Step::Submerged { 0.0 } else { self.rise }
    }

    fn death_color(&self) -> Rgba {
        Rgba::rgb(255, 160, 80)
    }

    fn touches_ground(&self) -> bool {
        true
    }

    fn light(&self, _enemy: &Enemy) -> Option<(f32, Rgba)> {
        (self.step != Step::Submerged).then_some((60.0 * self.rise, Rgba::rgb(255, 140, 60)))
    }

    fn before_hit(&mut self, enemy: &mut Enemy, world: &mut World, hit: &mut Hit) -> Option<bool> {
        if !hit.is_player() {
            return None;
        }
        if self.step == Step::Surfaced && (matches!(hit.element, Element::Water | Element::Aether) || hit.aether) {
            self.chill += hit.power;
            Particle::burst(world, self.hurtbox(enemy).center(), Rgba::rgb(230, 230, 240), 6, 40.0, 0.4, false);
            if self.chill >= 2.2 {
                self.cool(enemy, world);
            }
        }
        let armored = self.armored();
        let scale = if self.step == Step::Cooled {
            1.3
        } else if armored {
            0.35
        } else {
            1.0
        };
        if armored && self.step != Step::Cooled {
            audio::play(Sound::Clink, 0.6, -0.3);
        }
        hit.power *= scale;
        hit.knockback = 0.0;
        None
    }

    fn part_anchor(&self, index: usize, enemy: &Enemy) -> Vec2 {
        let offset = PLATE_OFFSETS[index];
        enemy.base.pos - Vec2::new(offset.x, offset.y * self.rise)
    }

    fn part_broken(&mut self, index: usize, enemy: &mut Enemy, world: &mut World) {
        if let Some(plate) = self.plates.get_mut(index) {
            plate.1 = true;
        }
        if !self.armored() {
            world.add(FloatingText::new(
                enemy.base.pos - Vec2::new(0.0, 50.0),
                "Its crust is gone!",
                palette::HIGHLIGHT,
            ));
        }
    }
}
