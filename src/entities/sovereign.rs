//! Sovereign Vael, the final boss. In the first form he is shielded by four elemental orbs that each break only to
//! their own element. Once they fall he ascends into a winged form that teleports, fires beams and calls down rings.

use bevy::math::Vec2;

use super::boss::{self, BossBrain, BossCore};
use super::effects::{DamageNumber, Particle, Puff};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use super::projectile::Projectile;
use crate::art::{bosses, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, safe_normalize, snap};
use crate::core::gfx::{Batch, FrameStrip};
use crate::entity_base;
use crate::gameplay::combat::{self, CombatStats, Faction, Hit};
use crate::gameplay::element::Element;
use crate::gameplay::game_data::data;
use crate::scenes::world::World;

const ORBS: [(Element, Rgba); 4] = [
    (Element::Wind, Rgba::rgb(120, 230, 200)),
    (Element::Fire, Rgba::rgb(255, 120, 40)),
    (Element::Water, Rgba::rgb(80, 160, 255)),
    (Element::Thunder, Rgba::rgb(250, 230, 80)),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Shielded,
    Ascending,
    Hover,
    FadeOut,
    FadeIn,
    Beam,
    Burst,
    Summon,
}

pub struct SovereignBrain {
    anchors: [Vec2; 5],
    orbs: Vec<EntityId>,
    orb_alive: [bool; 4],
    phase: Phase,
    timer: f32,
    fade: f32,
    orb_angle: f32,
    cycle: i32,
    beam_shots: i32,
    last_orb: Element,
    aether_warned: bool,
}

impl SovereignBrain {
    pub fn new(enemy: &mut Enemy) -> SovereignBrain {
        let pos = enemy.base.pos;
        SovereignBrain {
            anchors: [
                pos,
                pos + Vec2::new(-80.0, 24.0),
                pos + Vec2::new(80.0, 24.0),
                pos + Vec2::new(-56.0, 64.0),
                pos + Vec2::new(56.0, 64.0),
            ],
            orbs: Vec::new(),
            orb_alive: [true; 4],
            phase: Phase::Shielded,
            timer: 3.0,
            fade: 1.0,
            orb_angle: 0.0,
            cycle: 0,
            beam_shots: 0,
            last_orb: Element::Aether,
            aether_warned: false,
        }
    }

    fn first_form() -> FrameStrip {
        FrameStrip::new(bosses::sovereign(), 32, 40)
    }

    fn second_form() -> FrameStrip {
        FrameStrip::new(bosses::sovereign_ascended(), 48, 48)
    }

    fn ascended(&self) -> bool {
        !matches!(self.phase, Phase::Shielded | Phase::Ascending)
    }

    fn aether_form(&self, enemy: &Enemy) -> bool {
        self.ascended() && enemy.raw_health_fraction() < 0.35
    }

    fn orbs_left(&self) -> usize {
        self.orb_alive.iter().filter(|&&alive| alive).count()
    }

    fn phase_shot(&self) -> &'static str {
        match self.last_orb {
            Element::Fire => "fire",
            Element::Water => "ice",
            Element::Thunder => "spark",
            Element::Wind => "gust",
            _ => "aether",
        }
    }

    fn next(&mut self, phase: Phase, time: f32) {
        self.phase = phase;
        self.timer = time;
    }
}

impl BossBrain for SovereignBrain {
    fn added(&mut self, enemy: &mut Enemy, world: &mut World) {
        let hp = (enemy.max_hp / 12).max(40);
        for (index, (element, color)) in ORBS.into_iter().enumerate() {
            let orb = ElementOrb::new(enemy.base.id, index, element, color, enemy.stats(), hp);
            self.orbs.push(world.add(orb));
        }
    }

    fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        self.timer -= dt;
        self.orb_angle += dt * (0.8 + (4 - self.orbs_left()) as f32 * 0.25);
        let time = enemy.boss.as_ref().map_or(0.0, |b| b.time);
        enemy.z = 6.0 + (time * 2.0).sin() * 2.0;
        let enraged = self.ascended() && enemy.raw_health_fraction() < 0.5;
        let player = world.hero.pos;

        match self.phase {
            Phase::Shielded => {
                if self.orbs_left() == 0 {
                    self.next(Phase::Ascending, 2.2);
                    world.flash(Rgba::WHITE, 0.6);
                    world.camera.shake(1.5, 2.0);
                    audio::sfx(Sound::BossRoar);
                    world.show_message("Sovereign Vael: Enough! Behold what the Aether made of me!");
                } else if self.timer <= 0.0 {
                    boss::ring(enemy, world, "dark", enemy.base.pos - Vec2::new(0.0, 16.0), 10, time, 0.8, 1.0);
                    audio::play(Sound::ShootFire, 0.6, -0.4);
                    self.timer = 3.2;
                }
            }
            Phase::Ascending => {
                if world.rng.next_f32() < 0.5 {
                    let x = world.rng.range(-20, 21) as f32;
                    let y = world.rng.range(0, 40) as f32;
                    let at = enemy.base.pos + Vec2::new(x, -y);
                    world.add(Particle::new(at, Vec2::new(0.0, -40.0), Rgba::WHITE, 0.6).spark());
                }
                if self.timer <= 0.0 {
                    self.next(Phase::Hover, 1.0);
                }
            }
            Phase::Hover => {
                let to_player = player - enemy.base.pos;
                if to_player.length() > 30.0 {
                    enemy.move_by(world, safe_normalize(to_player) * 22.0 * dt, true);
                }
                if self.timer <= 0.0 {
                    self.next(Phase::FadeOut, 0.35);
                }
            }
            Phase::FadeOut => {
                self.fade = (self.timer / 0.35).max(0.0);
                if self.timer <= 0.0 {
                    let choices: Vec<Vec2> =
                        self.anchors.iter().copied().filter(|a| a.distance(player) > 48.0).collect();
                    enemy.base.pos = if choices.is_empty() {
                        self.anchors[0]
                    } else {
                        choices[world.rng.below(choices.len() as i32) as usize]
                    };
                    self.next(Phase::FadeIn, 0.35);
                }
            }
            Phase::FadeIn => {
                self.fade = 1.0 - (self.timer / 0.35).max(0.0);
                if self.timer <= 0.0 {
                    self.fade = 1.0;
                    self.cycle += 1;
                    let next = if enraged && self.cycle % 3 == 0 {
                        Phase::Summon
                    } else if self.cycle % 2 == 0 {
                        Phase::Burst
                    } else {
                        Phase::Beam
                    };
                    self.next(next, 0.5);
                    self.beam_shots = if enraged { 10 } else { 7 };
                }
            }
            Phase::Beam => {
                if self.timer <= 0.0 {
                    let kind = if self.aether_form(enemy) { "aether" } else { self.phase_shot() };
                    let from = enemy.base.pos - Vec2::new(0.0, 20.0);
                    boss::shoot(enemy, world, kind, from, player - enemy.base.pos, 1.6, 1.0);
                    audio::play(Sound::ShootWind, 0.5, 0.5);
                    self.timer = 0.12;
                    self.beam_shots -= 1;
                    if self.beam_shots <= 0 {
                        self.next(Phase::Hover, 1.2);
                    }
                }
            }
            Phase::Burst => {
                if self.timer <= 0.0 {
                    let from = enemy.base.pos - Vec2::new(0.0, 20.0);
                    let count = if self.aether_form(enemy) { 16 } else { 12 };
                    boss::ring(enemy, world, self.phase_shot(), from, count, time, 1.0, 1.0);
                    boss::ring(enemy, world, "dark", from, 12, time + std::f32::consts::PI / 12.0, 0.7, 1.0);
                    audio::sfx(Sound::ShootFire);
                    self.next(Phase::Hover, 1.4);
                }
            }
            Phase::Summon => {
                if self.timer <= 0.0 {
                    if let Some(knight) = data().enemies.get("darkknight")
                        && world.entities_of::<Enemy>().filter(|e| e.def.id == "darkknight").count() < 2
                    {
                        world.add(Enemy::new(knight, enemy.base.pos + Vec2::new(-28.0, 12.0), None));
                        world.add(Enemy::new(knight, enemy.base.pos + Vec2::new(28.0, 12.0), None));
                    }
                    audio::play(Sound::BossRoar, 0.7, 0.0);
                    self.next(Phase::Hover, 1.0);
                }
            }
        }
    }

    fn draw(&self, enemy: &Enemy, boss: &BossCore, world: &World, batch: &mut Batch) {
        let fade = self.fade * boss.death_fade();
        if fade <= 0.02 {
            return;
        }
        let time = boss.time;
        let pos = enemy.base.pos;
        let feet = pos - Vec2::new(0.0, enemy.z.round());
        batch.draw(props::shadow(26), snap(pos - Vec2::new(13.0, 4.0)), Rgba::WHITE.mul(fade));
        let flash = (enemy.flashing() || boss.dying()) && (time * 20.0) as i32 % 2 == 0;
        if self.ascended() {
            let strip = Self::second_form();
            let frame = (time * 4.0) as i32 % 2;
            let aura = Rgba::rgb(200, 190, 255).mul(0.3 * fade);
            for offset in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0)] {
                strip.draw(batch, feet + offset, frame, aura, false, true, 1.0);
            }
            strip.draw(batch, feet, frame, boss.tint().mul(fade), false, flash, 1.0);
        } else {
            let shimmer = self.phase == Phase::Ascending && (time * 16.0) as i32 % 2 == 0;
            let frame = (time * 2.0) as i32 % 2;
            let flip = world.hero.pos.x < pos.x;
            Self::first_form().draw(batch, feet, frame, boss.tint().mul(fade), flip, flash || shimmer, 1.0);
            if self.phase == Phase::Shielded {
                batch.draw(props::barrier_bubble(), snap(feet - Vec2::new(14.0, 34.0)), Rgba::WHITE.mul(0.8));
            }
        }
    }

    fn hurtbox(&self, enemy: &Enemy) -> Box2 {
        let pos = enemy.base.pos;
        if self.ascended() {
            Box2::new(pos.x - 12.0, pos.y - 40.0 - enemy.z, 24.0, 38.0)
        } else {
            Box2::new(pos.x - 9.0, pos.y - 34.0 - enemy.z, 18.0, 32.0)
        }
    }

    fn tangible(&self) -> bool {
        matches!(self.phase, Phase::Hover | Phase::Beam | Phase::Burst | Phase::Summon)
    }

    fn death_color(&self) -> Rgba {
        Rgba::rgb(230, 220, 255)
    }

    fn health_fraction(&self, enemy: &Enemy) -> Option<f32> {
        Some(if self.ascended() { enemy.raw_health_fraction() } else { self.orbs_left() as f32 / 4.0 })
    }

    fn before_hit(&mut self, enemy: &mut Enemy, world: &mut World, hit: &mut Hit) -> Option<bool> {
        let dying = enemy.boss.as_ref().is_some_and(|b| b.dying());
        if !self.aether_form(enemy) || !hit.is_player() || hit.element == Element::Aether || dying {
            return None;
        }
        audio::play(Sound::Clink, 0.8, 0.5);
        world.add_resonance(6.0);
        if !self.aether_warned {
            self.aether_warned = true;
            world.show_message(
                "Sovereign Vael: The elements bow to me now. Only the whole Aether could reach me... and the Concord took that to its grave! (Your blade drinks in the light he sheds. Fill it and call the Aether.)",
            );
        }
        Some(true)
    }

    fn on_die(&mut self, _enemy: &mut Enemy, world: &mut World) -> bool {
        for &id in &self.orbs {
            world.with_entity(id, |e, _| e.base_mut().removed = true);
        }
        false
    }

    fn part_anchor(&self, index: usize, enemy: &Enemy) -> Vec2 {
        let a = self.orb_angle + index as f32 * std::f32::consts::TAU / 4.0;
        enemy.base.pos + Vec2::new(a.cos() * 44.0, a.sin() * 26.0 - 4.0)
    }

    fn part_broken(&mut self, index: usize, _enemy: &mut Enemy, _world: &mut World) {
        self.orb_alive[index] = false;
        self.last_orb = ORBS[index].0;
    }
}

/// One of the four orbs shielding Vael; only its own element can break it.
pub struct ElementOrb {
    base: EntityBase,
    owner: EntityId,
    index: usize,
    element: Element,
    color: Rgba,
    stats: CombatStats,
    hp: i32,
    flash: f32,
    fire_timer: f32,
}

impl ElementOrb {
    pub fn new(
        owner: EntityId,
        index: usize,
        element: Element,
        color: Rgba,
        stats: CombatStats,
        hp: i32,
    ) -> ElementOrb {
        ElementOrb {
            base: EntityBase::default(),
            owner,
            index,
            element,
            color,
            stats,
            hp,
            flash: 0.0,
            fire_timer: 2.0 + element.index() as f32 * 0.7,
        }
    }

    fn follow(&mut self, world: &World) -> Option<bool> {
        let owner = world.get_as::<Enemy>(self.owner)?;
        let core = owner.boss.as_ref()?;
        if let Some(brain) = core.brain() {
            self.base.pos = brain.part_anchor(self.index, owner);
        }
        Some(core.dying())
    }
}

impl Entity for ElementOrb {
    entity_base!();

    fn sort_y(&self) -> f32 {
        self.base.pos.y + 1.0
    }

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Enemy)
    }

    fn hurtbox(&self) -> Box2 {
        Box2::from_center(self.base.pos - Vec2::new(0.0, 7.0), 14.0, 14.0)
    }

    fn added(&mut self, world: &mut World) {
        self.follow(world);
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        let Some(owner_dying) = self.follow(world) else {
            self.base.removed = true;
            return;
        };
        self.flash = (self.flash - dt).max(0.0);
        self.fire_timer -= dt;
        if self.fire_timer <= 0.0 && !owner_dying {
            self.fire_timer = 3.6;
            let kind = match self.element {
                Element::Fire => "fire",
                Element::Water => "ice",
                Element::Thunder => "spark",
                _ => "aether",
            };
            let dir = safe_normalize(world.hero.pos - self.base.pos);
            world.add(Projectile::enemy_shot(kind, self.base.pos - Vec2::new(0.0, 2.0), dir, self.stats, 1.0, 1.0));
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if hit.source != Faction::Player {
            return false;
        }
        let center = self.hurtbox().center();
        if hit.element != self.element && hit.element != Element::Aether {
            audio::sfx(Sound::Clink);
            Particle::burst(world, center, Rgba::WHITE, 4, 40.0, 0.2, true);
            return true;
        }
        let mut stats = self.stats;
        stats.def *= 0.5;
        let result = combat::resolve(hit, stats, 1.0, 1.0, &world.rng);
        hit.result = result;
        self.hp -= result.amount;
        world.add(DamageNumber::new(center - Vec2::new(0.0, 10.0), result.amount, self.element, result.crit));
        self.flash = 0.15;
        audio::sfx(Sound::Hit);
        if self.hp <= 0 {
            self.base.removed = true;
            let index = self.index;
            world.with_as::<Enemy, _>(self.owner, |e, w| {
                boss::with_brain(e, |brain, e| brain.part_broken(index, e, w));
            });
            audio::sfx(Sound::Break);
            world.add(Puff::new(center, self.color));
            Particle::burst(world, center, self.color, 20, 90.0, 0.6, true);
        }
        true
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let texture = bosses::element_orb(if self.flash > 0.0 { Rgba::WHITE } else { self.color });
        let bob = ((world.time as f32 * 4.0 + self.element.index() as f32).sin() * 1.5).round();
        batch.draw(texture, snap(self.base.pos - Vec2::new(7.0, 14.0 + bob)), Rgba::WHITE);
    }
}
