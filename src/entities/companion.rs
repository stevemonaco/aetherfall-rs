//! The equipped soul, travelling with the player. It applies its passive while present and performs its active when
//! the Soul button is pressed and its cooldown has run out. Actives grow stronger as the soul ranks up. Also buried
//! treasure, which only the hound can sniff out.

use bevy::math::Vec2;

use super::combat_effects::{ClawEffect, Exclaim, LightningStrike, Shockwave};
use super::effects::Particle;
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use crate::art::{combat as art_combat, props as art};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{IRect, safe_normalize, snap};
use crate::core::gfx::{self, Batch};
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::HitKind;
use crate::gameplay::element::Element;
use crate::gameplay::souls::SoulDef;
use crate::scenes::world::World;
use crate::world::map::{EntityData, TILE, tile_feet};

/// A creature from an RPG Maker-format sheet: 3 walk frames by 4 directions (down, left, right, up) per block.
#[derive(Clone, Copy)]
struct AnimalSheet {
    path: &'static str,
    frame_width: i32,
    frame_height: i32,
    index: i32,
}

const HOUND: AnimalSheet =
    AnimalSheet { path: "external/Animals/animals1.png", frame_width: 26, frame_height: 36, index: 1 };
const HAWK: AnimalSheet =
    AnimalSheet { path: "external/Animals/birds2.png", frame_width: 42, frame_height: 36, index: 0 };

fn draw_animal(batch: &mut Batch, sheet: AnimalSheet, feet: Vec2, facing: Direction, frame: i32) {
    let texture = gfx::external(sheet.path);
    let block_x = sheet.index % 4 * 3 * sheet.frame_width;
    let block_y = sheet.index / 4 * 4 * sheet.frame_height;
    let row = match facing {
        Direction::Left => 1,
        Direction::Right => 2,
        Direction::Up => 3,
        Direction::Down => 0,
    };
    let src = IRect::new(
        block_x + frame % 3 * sheet.frame_width,
        block_y + row * sheet.frame_height,
        sheet.frame_width,
        sheet.frame_height,
    );
    let origin = Vec2::new(sheet.frame_width as f32 / 2.0, (sheet.frame_height - 4) as f32);
    batch.draw_ex(texture, snap(feet), Some(src), Rgba::WHITE, 0.0, origin, Vec2::ONE, false, false);
}

pub struct Companion {
    base: EntityBase,
    soul: &'static SoulDef,
    cooldown: f32,
    time: f32,
    bark_timer: f32,
    guard_timer: f32,
    velocity: Vec2,
    prey: Option<EntityId>,
    facing: Direction,
    rank: i32,
}

impl Companion {
    pub fn new(soul: &'static SoulDef, at: Vec2) -> Companion {
        Companion {
            base: EntityBase { pos: at, width: 6.0, height: 6.0, ..EntityBase::default() },
            soul,
            cooldown: 0.0,
            time: 0.0,
            bark_timer: 0.0,
            guard_timer: 0.0,
            velocity: Vec2::ZERO,
            prey: None,
            facing: Direction::Down,
            rank: 1,
        }
    }

    pub fn soul(&self) -> &'static SoulDef {
        self.soul
    }

    pub fn cooldown(&self) -> f32 {
        self.cooldown
    }

    pub fn cooldown_max(&self) -> f32 {
        self.soul.cooldown * (1.0 - 0.15 * (self.rank - 1) as f32)
    }

    pub fn guard_ready(&self) -> bool {
        self.soul.id == "guardian" && self.guard_timer <= 0.0
    }

    /// The Guardian turns aside a projectile; returns whether it did.
    pub fn try_block(&mut self, world: &mut World, shot_at: Vec2) -> bool {
        if !self.guard_ready() {
            return false;
        }
        self.guard_timer = 10.0 / self.power();
        audio::play(Sound::Clink, 0.9, 0.4);
        Particle::burst(world, shot_at, self.soul.glow, 10, 60.0, 0.3, true);
        true
    }

    fn power(&self) -> f32 {
        1.0 + 0.25 * (self.rank - 1) as f32
    }

    fn flies(&self) -> bool {
        self.soul.id != "hound"
    }

    fn passive(&mut self, world: &mut World, dt: f32) {
        if self.soul.id == "hound" {
            self.bark_timer -= dt;
            let me = self.base.pos;
            let near = world.entities_of::<DigSpot>().any(|d| d.base.pos.distance(me) < 90.0);
            if near && self.bark_timer <= 0.0 {
                self.bark_timer = 2.5;
                world.add(Exclaim::new(self.base.id, self.base.pos, self.base.height));
                audio::play(Sound::MenuMove, 0.5, 0.6);
            }
        }
        if world.rng.next_f32() < 0.15 && self.flies() {
            let at = self.base.pos - Vec2::new(world.rng.range(-3, 4) as f32, 12.0);
            world.add(Particle::new(at, Vec2::new(0.0, -8.0), self.soul.glow, 0.5));
        }
    }

    fn nearest_enemy(world: &World, range: f32) -> Option<EntityId> {
        let hero = world.hero.pos;
        world
            .entities_of::<Enemy>()
            .filter(|e| e.targetable() && e.base.pos.distance(hero) < range)
            .min_by(|a, b| a.base.pos.distance(hero).total_cmp(&b.base.pos.distance(hero)))
            .map(|e| e.base.id)
    }

    fn activate(&mut self, world: &mut World) {
        if self.cooldown > 0.0 {
            audio::play(Sound::MenuCancel, 0.5, 0.0);
            return;
        }
        let power = self.power();
        let mut used = true;
        match self.soul.id {
            "hound" => {
                self.prey = Self::nearest_enemy(world, 160.0);
                used = self.prey.is_some();
                if used {
                    audio::play(Sound::Swing, 0.8, 0.4);
                }
            }
            "hawk" => {
                let view = world.camera.view();
                for id in world.ids_of::<Enemy>() {
                    world.with_as::<Enemy, _>(id, |e, _| {
                        if view.contains_point(e.base.pos.x as i32, e.base.pos.y as i32) {
                            e.mark(8.0);
                        }
                    });
                }
                audio::play(Sound::ChargeLevel, 0.7, 0.5);
                world.flash(Rgba::rgb(255, 220, 160), 0.15);
            }
            "lantern" => {
                let me = self.base.pos;
                for id in world.ids_of::<Enemy>() {
                    world.with_as::<Enemy, _>(id, |e, _| {
                        if e.base.pos.distance(me) < 90.0 * power {
                            e.stun(1.5 * power);
                        }
                    });
                }
                world.flash(Rgba::rgb(255, 245, 200), 0.3);
                world.add(Shockwave::new(me, 90.0 * power, Rgba::rgb(255, 240, 180)));
                audio::play(Sound::ChargeLevel, 1.0, 0.6);
            }
            "guardian" => {
                world.with_player(|p, _| p.begin_aegis(3.0 * power));
                audio::play(Sound::Heal, 0.7, 0.0);
            }
            "ember" => {
                let at = self.base.pos - Vec2::new(0.0, 6.0);
                world.with_player(|p, w| {
                    p.strike_area(w, at, 36.0 * power, None, move |p, w| {
                        p.make_hit(w, 1.5 * power, HitKind::Art, Some(Element::Fire), 2.0, 2.0, 160.0)
                    })
                });
                world.add(Shockwave::new(self.base.pos, 36.0 * power, Element::Fire.glow()));
                let (tx, ty) = self.base.tile_pos();
                for y in -1..=1 {
                    for x in -1..=1 {
                        world.chem_ignite(tx + x, ty + y);
                    }
                }
                audio::sfx(Sound::ShootFire);
            }
            "stormling" => {
                let target = Self::nearest_enemy(world, 180.0);
                used = target.is_some();
                if let Some(id) = target {
                    let pos = world.get(id).map(|e| e.base().pos).unwrap_or_default();
                    world.add(LightningStrike::new(pos));
                    world.with_player(|p, w| {
                        let hit = p.make_hit(w, 2.0 * power, HitKind::Art, Some(Element::Thunder), 3.0, 2.5, 60.0);
                        p.strike_target(w, id, hit);
                    });
                    audio::play(Sound::Break, 0.7, 0.5);
                }
            }
            "echo" => {
                used = world.with_player(|p, w| p.replay_last_art(w)).unwrap_or(false);
            }
            _ => {}
        }
        if !used {
            audio::play(Sound::MenuCancel, 0.5, 0.0);
            return;
        }
        self.cooldown = self.cooldown_max();
        *world.state.soul_xp.entry(self.soul.id.to_string()).or_default() += 1;
        Particle::burst(world, self.base.pos - Vec2::new(0.0, 8.0), self.soul.glow, 12, 50.0, 0.4, true);
    }

    fn hunt(&mut self, world: &mut World, dt: f32) {
        let Some(prey) = self.prey.and_then(|id| world.get_as::<Enemy>(id)).filter(|e| !e.base.removed) else {
            self.prey = None;
            return;
        };
        let (prey_id, prey_pos, prey_center) = (prey.base.id, prey.base.pos, prey.hurtbox().center());
        let to = prey_pos - self.base.pos;
        if to.length() < 10.0 {
            let power = self.power();
            world.with_player(|p, w| {
                let hit = p.make_hit(w, 1.3 * power, HitKind::Art, Some(Element::None), 3.0, 0.0, 220.0);
                p.strike_target(w, prey_id, hit);
            });
            world.add(ClawEffect::new(prey_center, safe_normalize(to + Vec2::splat(0.01)), Rgba::WHITE));
            self.prey = None;
            return;
        }
        self.facing = Direction::from_vec(to, self.facing);
        self.base.pos += to.normalize() * 260.0 * dt;
    }
}

impl Entity for Companion {
    entity_base!();

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some(if self.soul.id == "lantern" { (90.0, Rgba::rgb(255, 230, 160)) } else { (28.0, self.soul.glow) })
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        self.rank = world.state.soul_rank(self.soul.id);
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.guard_timer = (self.guard_timer - dt).max(0.0);

        if self.prey.is_some() {
            self.hunt(world, dt);
            return;
        }

        let facing = world.hero.facing;
        let side = if matches!(facing, Direction::Up | Direction::Down) { 10.0 } else { 0.0 };
        let behind = -facing.to_vec() * 14.0 + Vec2::new(side, if self.flies() { -4.0 } else { 2.0 });
        let target = world.hero.pos + behind;
        let to = target - self.base.pos;
        let distance = to.length();
        if distance > 220.0 {
            self.base.pos = target;
        } else if distance > 2.0 {
            self.velocity = self.velocity.lerp(to * 5.0, (dt * 8.0).min(1.0));
            if self.velocity.length() > 170.0 {
                self.velocity = self.velocity.normalize() * 170.0;
            }
            self.base.pos += self.velocity * dt;
            if self.velocity.length_squared() > 25.0 {
                self.facing = Direction::from_vec(self.velocity, self.facing);
            }
        } else {
            self.velocity = Vec2::ZERO;
        }

        self.passive(world, dt);
        if input::pressed(GameAction::Soul) && !world.input_locked() && !world.hero.dead {
            self.activate(world);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let flies = self.flies();
        let z = if flies { 12.0 + (self.time * 3.0).sin() * 2.0 } else { 0.0 };
        batch.draw(
            art::shadow(8),
            snap(self.base.pos - Vec2::new(4.0, 2.0)),
            Rgba::WHITE.mul(if flies { 0.5 } else { 0.8 }),
        );
        let feet = self.base.pos - Vec2::new(0.0, z);
        match self.soul.id {
            "hound" => {
                let moving = self.velocity.length_squared() > 25.0 || self.prey.is_some();
                draw_animal(batch, HOUND, feet, self.facing, if moving { (self.time * 8.0) as i32 % 3 } else { 1 });
            }
            "hawk" => draw_animal(batch, HAWK, feet + Vec2::new(0.0, 8.0), self.facing, (self.time * 6.0) as i32 % 3),
            _ => {
                let src = IRect::new((self.time * 6.0) as i32 % 2 * 12, 0, 12, 14);
                let tex = art_combat::spirit(self.soul.id, self.soul.glow);
                batch.draw_ex(
                    tex,
                    snap(feet),
                    Some(src),
                    Rgba::WHITE,
                    0.0,
                    Vec2::new(6.0, 13.0),
                    Vec2::ONE,
                    false,
                    false,
                );
            }
        }
        if self.guard_ready() {
            let ring = art_combat::ring();
            batch.draw_ex(
                ring,
                snap(feet - Vec2::new(0.0, 7.0)),
                None,
                self.soul.glow.mul(0.5),
                0.0,
                Vec2::splat(16.0),
                Vec2::splat(0.4),
                false,
                false,
            );
        }
    }
}

/// Buried treasure. Only a hound can sniff it out; dig where it barks.
pub struct DigSpot {
    base: EntityBase,
    flag: String,
    item: Option<String>,
    count: i32,
    gold: i32,
    time: f32,
}

impl DigSpot {
    pub fn new(data: &EntityData, map_id: &str) -> DigSpot {
        DigSpot {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                width: TILE as f32,
                height: TILE as f32,
                ..EntityBase::default()
            },
            flag: data.str("flag").map(str::to_string).unwrap_or_else(|| format!("dig:{map_id}:{}_{}", data.x, data.y)),
            item: data.str("item").map(str::to_string),
            count: data.int("count", 1),
            gold: data.int("gold", 0),
            time: 0.0,
        }
    }

    fn sensed(&self, world: &World) -> bool {
        world.state.soul.as_deref() == Some("hound")
            && world.companion.is_some()
            && world.hero.pos.distance(self.base.pos) < 110.0
    }
}

impl Entity for DigSpot {
    entity_base!();

    fn can_interact(&self, world: &World) -> bool {
        self.sensed(world)
    }

    fn layer(&self) -> i32 {
        0
    }

    fn added(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag) {
            self.base.removed = true;
        }
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.time += dt;
    }

    fn interact(&mut self, world: &mut World) {
        self.base.removed = true;
        world.state.flags.insert(self.flag.clone());
        audio::play(Sound::Break, 0.6, 0.4);
        Particle::burst(world, self.base.pos, Rgba::rgb(150, 110, 70), 16, 60.0, 0.5, false);
        if let Some(item) = self.item.clone() {
            world.give_item(&item, self.count);
        }
        if self.gold > 0 {
            world.state.gold += self.gold;
            world.toast(format!("Dug up {} gold!", self.gold));
            audio::sfx(Sound::Coin);
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        if !self.sensed(world) {
            return;
        }
        let pulse = 0.5 + 0.5 * (self.time * 5.0).sin();
        batch.draw(art_combat::vent(), snap(self.base.pos - Vec2::new(8.0, 10.0)), Rgba::rgb(160, 120, 80).mul(0.8));
        batch.draw(
            art::spark(),
            snap(self.base.pos - Vec2::new(1.0, 12.0 + pulse * 3.0)),
            palette::HIGHLIGHT.mul(pulse),
        );
    }
}
