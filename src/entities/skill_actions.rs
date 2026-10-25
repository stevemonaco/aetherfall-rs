//! A skill in progress. It owns the player until its step reports it finished. Each skill reads its charged level
//! (1 to 3) to decide how big it is.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::combat_effects::{Afterimage, GroundMarker, LightningArc, LightningStrike, Shockwave, SlashEffect};
use super::effects::Particle;
use super::entity::{Entity, EntityId};
use super::hazards::{Cyclone, FirePatch};
use super::player::{Player, ignite_around};
use super::projectile::{Projectile, ProjectileHook, Volley};
use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::rotate;
use crate::gameplay::combat::{Faction, HitKind};
use crate::gameplay::element::Element;
use crate::gameplay::skills::SkillDef;
use crate::scenes::world::World;
use crate::world::map::TILE;

#[derive(Clone, Copy)]
enum Effect {
    Nothing,
    GaleSlash,
    Cyclone,
    Meteor,
    FlameAegis,
    Lance,
    IceMirror,
    ArcCleave,
    Static,
}

enum Kind {
    Instant { duration: f32, effect: Effect, started: bool },
    Dodge { length: f32, speed: f32, start: Effect, end: Effect, ram: Element, rammed: Vec<EntityId> },
    CinderRush { length: f32, hit: Vec<EntityId>, last_tile: (i32, i32), wave: i32, exploded: bool },
    FrostBloom { pulses: usize, done: usize },
    StormCall { targets: Vec<Vec2>, struck: usize },
}

pub struct SkillAction {
    kind: Kind,
    level: i32,
    t: f32,
}

fn instant(duration: f32, effect: Effect) -> Kind {
    Kind::Instant { duration, effect, started: false }
}

impl SkillAction {
    pub fn create(skill: &SkillDef, level: i32) -> SkillAction {
        use Effect::*;
        let lv = level.clamp(1, 3);
        let kind = match skill.id {
            "gale_slash" => instant(0.28, GaleSlash),
            "cyclone" => instant(0.25, Cyclone),
            "updraft" => Kind::Dodge {
                length: if lv >= 2 { 0.44 } else { 0.32 },
                speed: 300.0,
                start: Nothing,
                end: if lv >= 3 { Cyclone } else { Nothing },
                ram: Element::Wind,
                rammed: Vec::new(),
            },
            "cinder_rush" => Kind::CinderRush {
                length: if lv >= 2 { 0.56 } else { 0.36 },
                hit: Vec::new(),
                last_tile: (-1, -1),
                wave: 0,
                exploded: false,
            },
            "meteor" => instant(0.25, Meteor),
            "flame_aegis" => instant(0.35, FlameAegis),
            "frost_bloom" => Kind::FrostBloom { pulses: if lv >= 3 { 2 } else { 1 }, done: 0 },
            "glacier_lance" => instant(0.25, Lance),
            "ice_mirror" => instant(0.2, IceMirror),
            "arc_cleave" => instant(0.3, ArcCleave),
            "storm_call" => Kind::StormCall { targets: Vec::new(), struck: 0 },
            "static_field" => instant(0.2, Static),
            _ => instant(0.1, Nothing),
        };
        SkillAction { kind, level: lv, t: 0.0 }
    }

    /// Advances the skill; returns true once it has finished.
    pub fn update(&mut self, p: &mut Player, world: &mut World, dt: f32) -> bool {
        self.t += dt;
        let (t, level) = (self.t, self.level);
        match &mut self.kind {
            Kind::Instant { duration, effect, started } => {
                if !*started {
                    *started = true;
                    run(*effect, level, p, world);
                }
                t >= *duration
            }
            Kind::Dodge { length, speed, start, end, ram, rammed } => {
                if t <= dt * 1.5 {
                    run(*start, level, p, world);
                    let aim = p.aim;
                    p.start_dodge(world, aim, *length, *speed, false);
                    return false;
                }
                if !p.dodging() {
                    run(*end, level, p, world);
                    return true;
                }
                p.update_dodge(world, dt);
                if *ram != Element::None {
                    let (c, ram) = (p.center(), *ram);
                    p.strike_area(world, c, 16.0, Some(rammed), move |p, w| {
                        p.make_hit(w, 1.2, HitKind::Skill, Some(ram), 2.0, 2.5, 280.0)
                    });
                }
                false
            }
            Kind::CinderRush { length, hit, last_tile, wave, exploded } => {
                if t <= dt * 1.5 {
                    audio::play(Sound::ShootFire, 1.0, 0.2);
                }
                let aim = p.aim;
                let mut base = p.base().clone();
                world.move_entity(&mut base, aim * 250.0 * dt, false, true, false);
                p.base_mut().pos = base.pos;
                world.add(Afterimage::new(p.pos() - Vec2::new(0.0, p.z), p.facing, world.state.mode));
                let current = (t / 0.11) as i32;
                if current != *wave {
                    *wave = current;
                    hit.clear();
                    world.add(SlashEffect::new(p.center(), aim, Element::Fire, 24.0, false, current % 2 == 0));
                }
                let c = p.center() + aim * 8.0;
                p.strike_area(world, c, 20.0, Some(hit), |p, w| {
                    p.make_hit(w, 0.8, HitKind::Skill, Some(Element::Fire), 1.5, 1.2, 60.0)
                });
                if p.tile_pos() != *last_tile {
                    *last_tile = p.tile_pos();
                    world.add(FirePatch::new(p.pos(), 2.5));
                    world.chem_ignite(last_tile.0, last_tile.1);
                }
                if t >= *length && level >= 3 && !*exploded {
                    *exploded = true;
                    burst(p, world, 44.0, Element::Fire, 2.2, 1);
                }
                t >= *length
            }
            Kind::FrostBloom { pulses, done } => {
                let next = 0.15 + *done as f32 * 0.3;
                if *done < *pulses && t >= next {
                    *done += 1;
                    let (radius, freeze) = match level {
                        1 => (42.0, 3),
                        2 => (56.0, 4),
                        _ => (70.0, 5),
                    };
                    burst(p, world, radius, Element::Water, 1.5 + 0.3 * level as f32, 0);
                    let (tx, ty) = p.tile_pos();
                    for y in -freeze..=freeze {
                        for x in -freeze..=freeze {
                            if x * x + y * y <= freeze * freeze {
                                world.chem_freeze(tx + x, ty + y);
                            }
                        }
                    }
                    let sparks = 8 + 4 * level;
                    for i in 0..sparks {
                        let a = i as f32 * TAU / sparks as f32;
                        let velocity = Vec2::new(a.cos(), a.sin()) * (60.0 + 20.0 * level as f32);
                        world.add(Particle::new(p.pos(), velocity, Rgba::rgb(200, 240, 255), 0.5).spark());
                    }
                }
                t >= 0.25 + *pulses as f32 * 0.3
            }
            Kind::StormCall { targets, struck } => {
                if t <= dt * 1.5 {
                    let first = p.aim_point(world, 130.0);
                    targets.push(first);
                    let extra: usize = match level {
                        1 => 0,
                        2 => 2,
                        _ => 4,
                    };
                    let mut nearby: Vec<Vec2> = world
                        .damageables()
                        .into_iter()
                        .filter(|(_, f, b)| *f == Faction::Enemy && b.center().distance(first) < 90.0)
                        .map(|(_, _, b)| b.center() + Vec2::new(0.0, 8.0))
                        .filter(|c| c.distance(first) > 12.0)
                        .collect();
                    nearby.sort_by(|a, b| a.distance(first).total_cmp(&b.distance(first)));
                    for i in 0..extra {
                        let fallback = first + rotate(Vec2::new(32.0, 0.0), i as f32 * TAU / extra as f32);
                        targets.push(nearby.get(i).copied().unwrap_or(fallback));
                    }
                    for &at in targets.iter() {
                        world.add(GroundMarker::new(at, 24.0, 0.25, Element::Thunder.glow()));
                    }
                    audio::play(Sound::ChargeLevel, 0.8, -0.3);
                }
                while *struck < targets.len() && t >= 0.25 + *struck as f32 * 0.1 {
                    let at = targets[*struck];
                    *struck += 1;
                    world.add(LightningStrike::new(at));
                    p.strike_area(world, at - Vec2::new(0.0, 4.0), 26.0, None, |p, w| {
                        p.make_hit(w, 2.2, HitKind::Skill, Some(Element::Thunder), 3.0, 2.5, 100.0)
                    });
                    let tile = ((at.x / TILE as f32).floor() as i32, ((at.y - 3.0) / TILE as f32).floor() as i32);
                    world.chem_apply(tile.0, tile.1, Element::Thunder, true);
                    world.camera.shake(0.2, 2.0);
                    world.flash(Rgba::rgb(255, 250, 200), 0.12);
                    audio::play(Sound::Break, 0.8, 0.4);
                }
                *struck >= targets.len() && t >= 0.45 + targets.len() as f32 * 0.1
            }
        }
    }
}

fn run(effect: Effect, level: i32, p: &mut Player, world: &mut World) {
    match effect {
        Effect::Nothing => {}
        Effect::GaleSlash => {
            world.add(SlashEffect::new(p.center(), p.aim, Element::Wind, 30.0, true, false));
            let angles: &[f32] = if level >= 3 { &[-0.35, 0.0, 0.35] } else { &[0.0] };
            let volley = Volley::default();
            for &angle in angles {
                let dir = rotate(p.aim, angle);
                let mut shot = Projectile::new(
                    Faction::Player,
                    world.state.stats(Element::Wind),
                    p.pos() + dir * 10.0,
                    dir * 210.0,
                );
                shot.power = match level {
                    1 => 1.7,
                    2 => 2.3,
                    _ => 2.6,
                };
                shot.element = Element::Wind;
                shot.kind = HitKind::Skill;
                shot.poise = 3.0;
                shot.status_power = 3.0;
                shot.knockback = 280.0;
                shot.lifetime = if level == 2 { 0.7 } else { 0.55 };
                shot.piercing = true;
                shot.size = if level == 2 { 26.0 } else { 20.0 };
                shot.scale = if level == 2 { 2.4 } else { 1.8 };
                shot.texture = props::wind_slash();
                shot.rotate_to_velocity = true;
                shot.trail_color = Element::Wind.glow();
                shot.paints_path = true;
                shot.volley = Some(volley.clone());
                p.scale_skill_shot(world, &mut shot);
                world.add(shot);
            }
            audio::play(Sound::ShootWind, 1.0, -0.1);
        }
        Effect::Cyclone => {
            let at = p.center() + p.aim * 10.0;
            match level {
                1 => world.add(Cyclone::new(at, p.aim)),
                2 => world.add(Cyclone::new(at, p.aim).great()),
                _ => {
                    world.add(Cyclone::new(at, rotate(p.aim, -0.4)));
                    world.add(Cyclone::new(at, rotate(p.aim, 0.4)))
                }
            };
        }
        Effect::Meteor => {
            let target = p.aim_point(world, 120.0);
            let offsets: Vec<Vec2> = match level {
                1 => vec![Vec2::ZERO],
                2 => vec![Vec2::ZERO, Vec2::new(-26.0, 10.0), Vec2::new(26.0, 10.0)],
                _ => (0..7)
                    .map(|i| if i == 0 { Vec2::ZERO } else { rotate(Vec2::new(34.0, 0.0), i as f32 * TAU / 6.0) })
                    .collect(),
            };
            for (i, offset) in offsets.into_iter().enumerate() {
                let flight = 0.6 + i as f32 * 0.06;
                let mut shot = Projectile::new(
                    Faction::Player,
                    world.state.stats(Element::Fire),
                    p.pos(),
                    (target + offset - p.pos()) / flight,
                );
                shot.power = 0.0;
                shot.element = Element::Fire;
                shot.kind = HitKind::Skill;
                shot.lifetime = flight;
                shot.lob_height = 46.0;
                shot.size = 4.0;
                shot.texture = props::fireball();
                shot.frame_width = 12;
                shot.scale = 1.8;
                shot.trail_color = Element::Fire.glow();
                shot.ignores_walls = true;
                shot.hook = ProjectileHook::Meteor;
                world.add(shot);
            }
            audio::sfx(Sound::ShootFire);
        }
        Effect::FlameAegis => {
            let (radius, power, ignite, ward) = match level {
                1 => (36.0, 1.6, 1, 0.0),
                2 => (48.0, 1.9, 1, 3.0),
                _ => (60.0, 2.2, 2, 5.0),
            };
            burst(p, world, radius, Element::Fire, power, ignite);
            if ward > 0.0 {
                p.begin_aegis(ward);
            }
        }
        Effect::Lance => {
            let lances: &[(f32, f32, f32)] = match level {
                1 => &[(0.0, 2.0, 1.6)],
                2 => &[(-0.2, 2.6, 1.6), (0.0, 2.6, 1.6), (0.2, 2.6, 1.6)],
                _ => &[(0.0, 3.6, 2.8)],
            };
            let volley = Volley::default();
            for &(angle, power, scale) in lances {
                let dir = rotate(p.aim, angle);
                let mut shot = Projectile::new(
                    Faction::Player,
                    world.state.stats(Element::Water),
                    p.pos() + dir * 8.0,
                    dir * 300.0,
                );
                shot.power = power;
                shot.element = Element::Water;
                shot.kind = HitKind::Skill;
                shot.poise = 3.0 * scale / 1.6;
                shot.status_power = 2.5;
                shot.knockback = 120.0;
                shot.z = 5.0;
                shot.lifetime = if level >= 3 { 1.0 } else { 0.75 };
                shot.piercing = true;
                shot.paints_path = true;
                shot.size = 10.0 * scale / 1.6;
                shot.scale = scale;
                shot.texture = props::ice_shard();
                shot.rotate_to_velocity = true;
                shot.trail_color = Element::Water.glow();
                shot.volley = Some(volley.clone());
                p.scale_skill_shot(world, &mut shot);
                world.add(shot);
            }
            audio::play(Sound::ShootWind, 0.9, 0.5);
        }
        Effect::IceMirror => match level {
            1 => p.begin_mirror(4.0, 1, false),
            2 => p.begin_mirror(6.0, 1, true),
            _ => p.begin_mirror(8.0, 2, true),
        },
        Effect::ArcCleave => {
            let (leaps, power) = match level {
                1 => (4, 1.6),
                2 => (7, 1.9),
                _ => (10, 2.2),
            };
            world.add(SlashEffect::new(p.center(), p.aim, Element::Thunder, 30.0, true, false));
            let mut struck: Vec<(EntityId, Vec2)> = Vec::new();
            for (id, faction, hurtbox) in world.damageables() {
                if faction == Faction::Enemy && p.in_arc(&hurtbox, 32.0, 100.0) {
                    let hit = p.make_hit(world, power, HitKind::Skill, Some(Element::Thunder), 3.0, 2.0, 150.0);
                    p.strike_target(world, id, hit);
                    struck.push((id, hurtbox.center()));
                }
            }
            if level >= 3
                && let Some(&(id, at)) = struck.first()
            {
                world.add(LightningStrike::new(at + Vec2::new(0.0, 8.0)));
                let hit = p.make_hit(world, 2.0, HitKind::Skill, Some(Element::Thunder), 3.0, 2.5, 80.0);
                p.strike_target(world, id, hit);
            }
            let mut from = struck.first().map(|s| s.1);
            for _ in 0..leaps {
                let Some(origin) = from else { break };
                let next = world
                    .damageables()
                    .into_iter()
                    .filter(|(id, f, b)| {
                        *f == Faction::Enemy && !struck.iter().any(|s| s.0 == *id) && b.center().distance(origin) < 64.0
                    })
                    .min_by(|a, b| a.2.center().distance(origin).total_cmp(&b.2.center().distance(origin)));
                let Some((id, _, b)) = next else { break };
                world.add(LightningArc::new(origin, b.center()));
                let hit = p.make_hit(world, power * 0.55, HitKind::Skill, Some(Element::Thunder), 1.0, 1.5, 60.0);
                p.strike_target(world, id, hit);
                struck.push((id, b.center()));
                from = Some(b.center());
            }
            audio::play(Sound::Clink, 1.0, -0.5);
        }
        Effect::Static => match level {
            1 => p.begin_static(5.0, 40.0, 0.35),
            2 => p.begin_static(8.0, 52.0, 0.35),
            _ => p.begin_static(10.0, 64.0, 0.5),
        },
    }
}

fn burst(p: &mut Player, world: &mut World, radius: f32, element: Element, power: f32, ignite: i32) {
    let c = p.center();
    p.strike_area(world, c, radius, None, move |p, w| {
        p.make_hit(w, power, HitKind::Skill, Some(element), 3.0, 2.2, 200.0)
    });
    if ignite > 0 {
        ignite_around(world, p.pos(), ignite);
    }
    world.add(Shockwave::new(p.pos(), radius, element.glow()));
    audio::play(if element == Element::Fire { Sound::ShootFire } else { Sound::Break }, 0.9, 0.0);
}
