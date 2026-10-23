//! A Combat Art in progress. It owns the player until its step reports it finished.

use std::f32::consts::TAU;

use bevy::math::Vec2;

use super::combat_effects::{
    Afterimage, Beam, GroundMarker, LightningArc, LightningStrike, Shockwave, SlashEffect, SpinEffect,
};
use super::effects::{DamageNumber, Particle};
use super::entity::{Entity, EntityId};
use super::hazards::{Cyclone, Decoy, FirePatch, SteamCloud};
use super::player::{Player, ignite_around};
use super::projectile::{Projectile, ProjectileHook};
use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{rotate, safe_dir};
use crate::gameplay::arts::ArtDef;
use crate::gameplay::combat::{Faction, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::map::TILE;

#[derive(Clone, Copy)]
enum Effect {
    Nothing,
    TripleShot,
    Decoy,
    Counter,
    GaleSlash,
    Cyclone,
    TempestWall,
    Meteor,
    FlameAegis,
    Lance,
    Mist,
    IceMirror,
    ArcCleave,
    Static,
    PrismRay,
    Flicker,
    Sanctum,
}

enum Kind {
    Instant {
        duration: f32,
        effect: Effect,
        started: bool,
    },
    Whirl {
        first: Vec<EntityId>,
        second: Vec<EntityId>,
    },
    Dodge {
        length: f32,
        speed: f32,
        start: Effect,
        ram: Element,
        trail: bool,
        rammed: Vec<EntityId>,
        last_tile: (i32, i32),
    },
    CinderRush {
        hit: Vec<EntityId>,
        last_tile: (i32, i32),
        wave: i32,
    },
    FrostBloom {
        done: bool,
    },
    StormCall {
        target: Vec2,
        struck: bool,
    },
    ArcStep,
    PrismEdge {
        next: usize,
    },
}

pub struct ArtAction {
    kind: Kind,
    t: f32,
}

fn instant(duration: f32, effect: Effect) -> Kind {
    Kind::Instant { duration, effect, started: false }
}

fn dodge(length: f32, speed: f32, start: Effect, ram: Element, trail: bool) -> Kind {
    Kind::Dodge { length, speed, start, ram, trail, rammed: Vec::new(), last_tile: (-1, -1) }
}

impl ArtAction {
    pub fn create(art: &ArtDef) -> ArtAction {
        use Effect::*;
        let kind = match art.id {
            "whirl" => Kind::Whirl { first: Vec::new(), second: Vec::new() },
            "triple_shot" => instant(0.2, TripleShot),
            "phantom_step" => dodge(0.34, 270.0, Decoy, Element::None, false),
            "counter_stance" => instant(0.25, Counter),
            "gale_slash" => instant(0.28, GaleSlash),
            "cyclone" => instant(0.25, Cyclone),
            "updraft" => dodge(0.32, 300.0, Nothing, Element::Wind, false),
            "tempest_wall" => instant(0.2, TempestWall),
            "cinder_rush" => Kind::CinderRush { hit: Vec::new(), last_tile: (-1, -1), wave: 0 },
            "meteor" => instant(0.25, Meteor),
            "blaze_step" => dodge(0.26, 260.0, Nothing, Element::None, true),
            "flame_aegis" => instant(0.35, FlameAegis),
            "frost_bloom" => Kind::FrostBloom { done: false },
            "glacier_lance" => instant(0.25, Lance),
            "mist_step" => dodge(0.26, 250.0, Mist, Element::None, false),
            "ice_mirror" => instant(0.2, IceMirror),
            "arc_cleave" => instant(0.3, ArcCleave),
            "storm_call" => Kind::StormCall { target: Vec2::ZERO, struck: false },
            "arc_step" => Kind::ArcStep,
            "static_field" => instant(0.2, Static),
            "prism_edge" => Kind::PrismEdge { next: 0 },
            "prism_ray" => instant(0.45, PrismRay),
            "flicker" => instant(0.2, Flicker),
            "sanctum" => instant(0.5, Sanctum),
            _ => instant(0.1, Nothing),
        };
        ArtAction { kind, t: 0.0 }
    }

    /// Advances the Art; returns true once it has finished.
    pub fn update(&mut self, p: &mut Player, world: &mut World, dt: f32) -> bool {
        self.t += dt;
        let t = self.t;
        match &mut self.kind {
            Kind::Instant { duration, effect, started } => {
                if !*started {
                    *started = true;
                    run(*effect, p, world);
                }
                t >= *duration
            }
            Kind::Whirl { first, second } => {
                if t < dt * 1.5 {
                    world.add(SpinEffect::new(p.center(), world.state.mode, 0.45));
                    audio::play(Sound::Swing, 1.0, -0.2);
                }
                if (0.08..0.2).contains(&t) {
                    let c = p.center();
                    p.strike_area(world, c, 32.0, Some(first), |p, w| {
                        p.make_hit(w, 1.3, HitKind::Art, None, 2.0, 1.2, 160.0)
                    });
                }
                if (0.28..0.4).contains(&t) {
                    let c = p.center();
                    p.strike_area(world, c, 32.0, Some(second), |p, w| {
                        p.make_hit(w, 1.3, HitKind::Art, None, 2.0, 1.2, 180.0)
                    });
                }
                t >= 0.45
            }
            Kind::Dodge { length, speed, start, ram, trail, rammed, last_tile } => {
                if t <= dt * 1.5 {
                    run(*start, p, world);
                    let aim = p.aim;
                    p.start_dodge(world, aim, *length, *speed, false);
                    return false;
                }
                if !p.dodging() {
                    return true;
                }
                p.update_dodge(world, dt);
                if *ram != Element::None {
                    let (c, ram) = (p.center(), *ram);
                    p.strike_area(world, c, 16.0, Some(rammed), move |p, w| {
                        p.make_hit(w, 1.2, HitKind::Art, Some(ram), 2.0, 2.5, 280.0)
                    });
                }
                if *trail && p.tile_pos() != *last_tile {
                    *last_tile = p.tile_pos();
                    world.add(FirePatch::new(p.pos(), 2.5));
                    world.chem_ignite(last_tile.0, last_tile.1);
                }
                false
            }
            Kind::CinderRush { hit, last_tile, wave } => {
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
                    p.make_hit(w, 0.8, HitKind::Art, Some(Element::Fire), 1.5, 1.2, 60.0)
                });
                if p.tile_pos() != *last_tile {
                    *last_tile = p.tile_pos();
                    world.add(FirePatch::new(p.pos(), 2.5));
                    world.chem_ignite(last_tile.0, last_tile.1);
                }
                t >= 0.36
            }
            Kind::FrostBloom { done } => {
                if t >= 0.15 && !*done {
                    *done = true;
                    burst(p, world, 42.0, Element::Water, 1.8, 0);
                    let (tx, ty) = p.tile_pos();
                    for y in -3..=3 {
                        for x in -3..=3 {
                            if x * x + y * y <= 9 {
                                world.chem_freeze(tx + x, ty + y);
                            }
                        }
                    }
                    for i in 0..8 {
                        let a = i as f32 * TAU / 8.0;
                        world.add(
                            Particle::new(p.pos(), Vec2::new(a.cos(), a.sin()) * 90.0, Rgba::rgb(200, 240, 255), 0.5)
                                .spark(),
                        );
                    }
                }
                t >= 0.4
            }
            Kind::StormCall { target, struck } => {
                if t <= dt * 1.5 {
                    *target = p.aim_point(world, 130.0);
                    world.add(GroundMarker::new(*target, 24.0, 0.25, Element::Thunder.glow()));
                    audio::play(Sound::ChargeLevel, 0.8, -0.3);
                }
                if t >= 0.25 && !*struck {
                    *struck = true;
                    let at = *target;
                    world.add(LightningStrike::new(at));
                    p.strike_area(world, at - Vec2::new(0.0, 4.0), 26.0, None, |p, w| {
                        p.make_hit(w, 2.2, HitKind::Art, Some(Element::Thunder), 3.0, 2.5, 100.0)
                    });
                    let tile = ((at.x / TILE as f32).floor() as i32, ((at.y - 3.0) / TILE as f32).floor() as i32);
                    world.chem_apply(tile.0, tile.1, Element::Thunder, true);
                    world.camera.shake(0.2, 2.0);
                    world.flash(Rgba::rgb(255, 250, 200), 0.15);
                    audio::play(Sound::Break, 0.8, 0.4);
                }
                t >= 0.45
            }
            Kind::ArcStep => {
                if t > dt * 1.5 {
                    return t >= 0.2;
                }
                let start = p.pos();
                let step = p.aim * 4.0;
                for _ in 0..18 {
                    let mut base = p.base().clone();
                    let moved = world.move_entity(&mut base, step, false, false, false);
                    p.base_mut().pos = base.pos;
                    if moved.length_squared() < 0.5 {
                        break;
                    }
                }
                let end = p.pos();
                let dir = safe_dir(end - start, p.aim);
                for (id, faction, hurtbox) in world.damageables() {
                    if faction != Faction::Enemy {
                        continue;
                    }
                    let c = hurtbox.center() + Vec2::new(0.0, 8.0);
                    let along = (c - start).dot(dir).clamp(0.0, start.distance(end));
                    if (start + dir * along).distance(c) < 14.0 {
                        let hit = p.make_hit(world, 1.4, HitKind::Art, Some(Element::Thunder), 2.0, 2.2, 80.0);
                        p.strike_target(world, id, hit);
                    }
                }
                world.add(LightningArc::new(start - Vec2::new(0.0, 8.0), end - Vec2::new(0.0, 8.0)));
                audio::play(Sound::Clink, 1.0, -0.3);
                false
            }
            Kind::PrismEdge { next } => {
                const ORDER: [Element; 5] =
                    [Element::Wind, Element::Fire, Element::Water, Element::Thunder, Element::Aether];
                if *next < ORDER.len() && t >= *next as f32 * 0.08 {
                    let element = ORDER[*next];
                    *next += 1;
                    let target = p.aim_point(world, 90.0);
                    world.add(Shockwave::new(target, 16.0, element.glow()));
                    p.strike_area(world, target - Vec2::new(0.0, 6.0), 22.0, None, move |p, w| {
                        p.make_hit(w, 1.1, HitKind::Art, Some(element), 2.0, 1.5, 40.0)
                    });
                    audio::play(Sound::Swing, 0.8, *next as f32 * 0.1);
                }
                t >= 0.5
            }
        }
    }
}

fn run(effect: Effect, p: &mut Player, world: &mut World) {
    match effect {
        Effect::Nothing => {}
        Effect::TripleShot => {
            for i in 0..3 {
                let angle = (i as f32 - 1.0) * 0.26;
                let bolt = p.make_bolt(world, rotate(p.aim, angle), false, 0.9);
                world.add(bolt);
            }
            audio::play(Sound::ShootWind, 0.8, 0.3);
        }
        Effect::Decoy => {
            world.add(Decoy::new(p.pos()));
        }
        Effect::Counter => p.begin_counter(1.2),
        Effect::GaleSlash => {
            world.add(SlashEffect::new(p.center(), p.aim, Element::Wind, 30.0, true, false));
            let mut shot = Projectile::new(
                Faction::Player,
                world.state.stats(Element::Wind),
                p.pos() + p.aim * 10.0,
                p.aim * 210.0,
            );
            shot.power = 1.7;
            shot.element = Element::Wind;
            shot.kind = HitKind::Art;
            shot.poise = 3.0;
            shot.status_power = 3.0;
            shot.knockback = 280.0;
            shot.lifetime = 0.55;
            shot.piercing = true;
            shot.size = 20.0;
            shot.scale = 1.8;
            shot.texture = props::wind_slash();
            shot.rotate_to_velocity = true;
            shot.trail_color = Element::Wind.glow();
            shot.paints_path = true;
            world.add(shot);
            audio::play(Sound::ShootWind, 1.0, -0.1);
        }
        Effect::Cyclone => {
            world.add(Cyclone::new(p.center() + p.aim * 10.0, p.aim));
        }
        Effect::TempestWall => p.begin_reflect(3.0),
        Effect::Meteor => {
            let target = p.aim_point(world, 120.0);
            let mut shot =
                Projectile::new(Faction::Player, world.state.stats(Element::Fire), p.pos(), (target - p.pos()) / 0.6);
            shot.power = 0.0;
            shot.element = Element::Fire;
            shot.kind = HitKind::Art;
            shot.lifetime = 0.6;
            shot.lob_height = 46.0;
            shot.size = 4.0;
            shot.texture = props::fireball();
            shot.frame_width = 12;
            shot.scale = 1.8;
            shot.trail_color = Element::Fire.glow();
            shot.ignores_walls = true;
            shot.hook = ProjectileHook::Meteor;
            world.add(shot);
            audio::sfx(Sound::ShootFire);
        }
        Effect::FlameAegis => burst(p, world, 36.0, Element::Fire, 1.6, 1),
        Effect::Lance => {
            let mut shot = Projectile::new(
                Faction::Player,
                world.state.stats(Element::Water),
                p.pos() + p.aim * 8.0,
                p.aim * 300.0,
            );
            shot.power = 2.0;
            shot.element = Element::Water;
            shot.kind = HitKind::Art;
            shot.poise = 3.0;
            shot.status_power = 2.5;
            shot.knockback = 120.0;
            shot.z = 5.0;
            shot.lifetime = 0.75;
            shot.piercing = true;
            shot.paints_path = true;
            shot.size = 10.0;
            shot.scale = 1.6;
            shot.texture = props::ice_shard();
            shot.rotate_to_velocity = true;
            shot.trail_color = Element::Water.glow();
            world.add(shot);
            audio::play(Sound::ShootWind, 0.9, 0.5);
        }
        Effect::Mist => {
            p.hide(2.5);
            for _ in 0..10 {
                let offset = Vec2::new(world.rng.range(-14, 15) as f32, world.rng.range(-8, 9) as f32);
                world.add(SteamCloud::new(p.pos() + offset, true));
            }
        }
        Effect::IceMirror => p.begin_mirror(4.0),
        Effect::ArcCleave => {
            world.add(SlashEffect::new(p.center(), p.aim, Element::Thunder, 30.0, true, false));
            let mut struck: Vec<(EntityId, Vec2)> = Vec::new();
            for (id, faction, hurtbox) in world.damageables() {
                if faction == Faction::Enemy && p.in_arc(&hurtbox, 32.0, 100.0) {
                    let hit = p.make_hit(world, 1.6, HitKind::Art, Some(Element::Thunder), 3.0, 2.0, 150.0);
                    p.strike_target(world, id, hit);
                    struck.push((id, hurtbox.center()));
                }
            }
            let mut from = struck.first().map(|s| s.1);
            for _ in 0..4 {
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
                let hit = p.make_hit(world, 0.9, HitKind::Art, Some(Element::Thunder), 1.0, 1.5, 60.0);
                p.strike_target(world, id, hit);
                struck.push((id, b.center()));
                from = Some(b.center());
            }
            audio::play(Sound::Clink, 1.0, -0.5);
        }
        Effect::Static => p.begin_static(5.0),
        Effect::PrismRay => {
            let start = p.center();
            let mut end = start;
            let mut i = 0;
            while i < 220 {
                let probe = start + p.aim * i as f32;
                let t = TILE as f32;
                if world.map.blocks_flying((probe.x / t).floor() as i32, ((probe.y + 6.0) / t).floor() as i32) {
                    break;
                }
                end = probe;
                i += 4;
            }
            for (id, faction, b) in world.damageables() {
                if faction == Faction::Player {
                    continue;
                }
                let c = b.center();
                let along = (c - start).dot(p.aim);
                if along < 0.0 || along > start.distance(end) + 6.0 {
                    continue;
                }
                if (start + p.aim * along).distance(c) < 10.0 + b.w / 2.0 {
                    let hit = p.make_hit(world, 3.0, HitKind::Art, Some(Element::Aether), 5.0, 2.0, 160.0);
                    p.strike_target(world, id, hit);
                }
            }
            world.add(Beam::new(start, end, Element::Aether.glow()));
            world.camera.shake(0.3, 2.0);
            audio::play(Sound::ChargeLevel, 1.0, -0.4);
        }
        Effect::Flicker => world.slow_enemies(3.0),
        Effect::Sanctum => {
            let amount = world.state.max_hp() / 4;
            world.heal_player(amount);
            p.cure_ailments();
            world.add(DamageNumber::heal(p.center(), amount));
            world.add(Shockwave::new(p.pos(), 40.0, Element::Aether.glow()));
            Particle::burst(world, p.center(), Rgba::rgb(255, 220, 255), 30, 60.0, 0.9, true);
            audio::sfx(Sound::Heal);
        }
    }
}

fn burst(p: &mut Player, world: &mut World, radius: f32, element: Element, power: f32, ignite: i32) {
    let c = p.center();
    p.strike_area(world, c, radius, None, move |p, w| {
        p.make_hit(w, power, HitKind::Art, Some(element), 3.0, 2.2, 200.0)
    });
    if ignite > 0 {
        ignite_around(world, p.pos(), ignite);
    }
    world.add(Shockwave::new(p.pos(), radius, element.glow()));
    audio::play(if element == Element::Fire { Sound::ShootFire } else { Sound::Break }, 0.9, 0.0);
}
