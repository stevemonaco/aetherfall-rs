//! The guardian spirit, travelling with the player in whichever form it has taken. It attacks nearby foes by itself,
//! applies its form's perk, and the Soul button cycles its forms. Also buried treasure, which only the hound can sniff
//! out.

use bevy::math::Vec2;

use super::combat_effects::{Beam, ClawEffect, Exclaim, LightningArc};
use super::effects::Particle;
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use super::obstacles::{ObstacleKind, TileObstacle};
use super::projectile::Projectile;
use crate::art::{combat as art_combat, props as art};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{IRect, safe_normalize, snap};
use crate::core::gfx::{self, Batch};
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::gameplay::souls::{self, SoulDef};
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
    time: f32,
    attack_timer: f32,
    bark_timer: f32,
    guard_timer: f32,
    velocity: Vec2,
    /// The foe a hound or hawk is lunging at.
    prey: Option<EntityId>,
    facing: Direction,
}

impl Companion {
    pub fn new(soul: &'static SoulDef, at: Vec2) -> Companion {
        Companion {
            base: EntityBase { pos: at, width: 6.0, height: 6.0, ..EntityBase::default() },
            soul,
            time: 0.0,
            attack_timer: 1.0,
            bark_timer: 0.0,
            guard_timer: 0.0,
            velocity: Vec2::ZERO,
            prey: None,
            facing: Direction::Down,
        }
    }

    pub fn soul(&self) -> &'static SoulDef {
        self.soul
    }

    /// How far the next attack has come, from 0 to 1, for the HUD.
    pub fn readiness(&self, world: &World) -> f32 {
        1.0 - (self.attack_timer / Self::interval(self.soul, world)).clamp(0.0, 1.0)
    }

    pub fn guard_ready(&self, world: &World) -> bool {
        world.state.charm("guardian_charm") && self.guard_timer <= 0.0
    }

    /// The Guardian Charm turns aside a projectile; returns whether it did.
    pub fn try_block(&mut self, world: &mut World, shot_at: Vec2) -> bool {
        if !self.guard_ready(world) {
            return false;
        }
        self.guard_timer = 10.0;
        audio::play(Sound::Clink, 0.9, 0.4);
        Particle::burst(world, shot_at, self.soul.glow, 10, 60.0, 0.3, true);
        true
    }

    fn interval(soul: &SoulDef, world: &World) -> f32 {
        soul.interval / (1.0 + world.state.mods().spirit_speed)
    }

    fn power(world: &World) -> f32 {
        1.0 + world.state.mods().spirit
    }

    fn flies(&self) -> bool {
        self.soul.id != "hound"
    }

    fn passive(&mut self, world: &mut World, dt: f32) {
        match self.soul.id {
            "hound" => {
                self.bark_timer -= dt;
                let me = self.base.pos;
                let near = world.entities_of::<DigSpot>().any(|d| d.base.pos.distance(me) < 90.0);
                if near && self.bark_timer <= 0.0 {
                    self.bark_timer = 2.5;
                    world.add(Exclaim::new(self.base.id, self.base.pos, self.base.height));
                    audio::play(Sound::MenuMove, 0.5, 0.6);
                }
            }
            "ember" => {
                let hero = world.hero.pos;
                let thickets: Vec<EntityId> = world
                    .entities_of::<TileObstacle>()
                    .filter(|o| o.kind == ObstacleKind::Thicket && o.base().pos.distance(hero) < 24.0)
                    .map(|o| o.base().id)
                    .collect();
                for id in thickets {
                    let mut hit = Hit {
                        element: Element::Fire,
                        kind: HitKind::Spirit,
                        from: self.base.pos,
                        ..Hit::new(Faction::Player, world.state.stats(world.state.mode))
                    };
                    world.hit(id, &mut hit);
                }
            }
            _ => {}
        }
        if world.rng.next_f32() < 0.15 && self.flies() {
            let at = self.base.pos - Vec2::new(world.rng.range(-3, 4) as f32, 12.0);
            world.add(Particle::new(at, Vec2::new(0.0, -8.0), self.soul.glow, 0.5));
        }
    }

    fn nearest_enemy(world: &World, from: Vec2, range: f32, except: Option<EntityId>) -> Option<EntityId> {
        world
            .entities_of::<Enemy>()
            .filter(|e| e.targetable() && e.base.pos.distance(from) < range && Some(e.base.id) != except)
            .min_by(|a, b| a.base.pos.distance(from).total_cmp(&b.base.pos.distance(from)))
            .map(|e| e.base.id)
    }

    /// Strikes a foe with the spirit's power; the Storm Charm carries the blow on to a second foe.
    fn strike(world: &mut World, target: EntityId, power: f32, element: Element) {
        let power = power * Self::power(world);
        let at = world.get(target).map(|e| e.hurtbox().center());
        world.with_player(|p, w| {
            let hit = p.make_hit(w, power, HitKind::Spirit, Some(element), 1.0, 1.0, 80.0);
            p.strike_target(w, target, hit);
        });
        let Some(at) = at.filter(|_| world.state.charm("storm_charm")) else { return };
        if let Some(next) = Self::nearest_enemy(world, at, 70.0, Some(target)) {
            let to = world.get(next).map(|e| e.hurtbox().center()).unwrap_or(at);
            world.add(LightningArc::new(at, to));
            world.with_player(|p, w| {
                let hit = p.make_hit(w, power * 0.6, HitKind::Spirit, Some(Element::Thunder), 0.5, 1.0, 40.0);
                p.strike_target(w, next, hit);
            });
        }
    }

    fn attack(&mut self, world: &mut World) -> bool {
        let range = match self.soul.id {
            "hawk" => 150.0,
            "hound" => 120.0,
            _ => 110.0,
        };
        let Some(target) = Self::nearest_enemy(world, world.hero.pos, range, None) else { return false };
        let to = world.get(target).map(|e| e.hurtbox().center()).unwrap_or(self.base.pos);
        let from = self.base.pos - Vec2::new(0.0, 10.0);
        match self.soul.id {
            "hound" | "hawk" => {
                self.prey = Some(target);
                audio::play(Sound::Swing, 0.6, 0.4);
            }
            "ember" => {
                let dir = safe_normalize(to - from);
                let mut ball = Projectile::new(Faction::Player, world.state.stats(world.state.mode), from, dir * 170.0);
                ball.power = 0.6 * Self::power(world);
                ball.element = Element::Fire;
                ball.kind = HitKind::Spirit;
                ball.lifetime = 0.9;
                ball.size = 6.0;
                ball.texture = art::fireball();
                ball.frame_width = 12;
                ball.trail_color = Element::Fire.glow();
                world.add(ball);
                audio::play(Sound::ShootFire, 0.4, 0.5);
            }
            "lantern" => {
                world.add(Beam::new(from, to, self.soul.glow));
                Self::strike(world, target, 0.4, Element::None);
                audio::play(Sound::ChargeLevel, 0.3, 0.9);
            }
            _ => {
                world.add(LightningArc::new(from, to));
                Self::strike(world, target, 0.35, Element::None);
                audio::play(Sound::Clink, 0.4, 0.9);
            }
        }
        true
    }

    fn hunt(&mut self, world: &mut World, dt: f32) {
        let Some(prey) = self.prey.and_then(|id| world.get_as::<Enemy>(id)).filter(|e| e.targetable()) else {
            self.prey = None;
            return;
        };
        let (prey_id, prey_pos, prey_center) = (prey.base.id, prey.base.pos, prey.hurtbox().center());
        let to = prey_pos - self.base.pos;
        if to.length() < 10.0 {
            let hawk = self.soul.id == "hawk";
            Self::strike(world, prey_id, if hawk { 0.7 } else { 0.6 }, Element::None);
            if hawk {
                world.with_as::<Enemy, _>(prey_id, |e, _| e.mark(4.0));
            }
            world.add(ClawEffect::new(prey_center, safe_normalize(to + Vec2::splat(0.01)), Rgba::WHITE));
            self.prey = None;
            return;
        }
        self.facing = Direction::from_vec(to, self.facing);
        self.base.pos += to.normalize() * 260.0 * dt;
    }

    /// The Soul button cycles through the forms the spirit has learned.
    fn cycle_form(&mut self, world: &mut World) {
        let mut forms = vec![souls::SPIRIT.id];
        forms.extend(world.state.souls_owned.iter().filter_map(|id| souls::get(id)).map(|s| s.id));
        if forms.len() < 2 {
            audio::play(Sound::MenuCancel, 0.5, 0.0);
            return;
        }
        let at = forms.iter().position(|&f| f == self.soul.id).unwrap_or(0);
        let next = souls::get(forms[(at + 1) % forms.len()]).unwrap_or(&souls::SPIRIT);
        self.soul = next;
        self.prey = None;
        world.state.soul = (next.id != souls::SPIRIT.id).then(|| next.id.to_string());
        Particle::burst(world, self.base.pos - Vec2::new(0.0, 8.0), next.glow, 14, 50.0, 0.4, true);
        world.toast(format!("Spirit form: {}", next.name));
        audio::play(Sound::ChargeLevel, 0.6, 0.6);
    }
}

impl Entity for Companion {
    entity_base!();

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some(if self.soul.id == "lantern" { (90.0, Rgba::rgb(255, 230, 160)) } else { (28.0, self.soul.glow) })
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        self.guard_timer = (self.guard_timer - dt).max(0.0);
        if input::pressed(GameAction::Soul) && !world.input_locked() && !world.hero.dead {
            self.cycle_form(world);
        }

        if self.prey.is_some() {
            self.hunt(world, dt);
            return;
        }

        self.attack_timer -= dt;
        if self.attack_timer <= 0.0 && !world.hero.dead {
            self.attack_timer = if self.attack(world) { Self::interval(self.soul, world) } else { 0.3 };
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
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
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
        if self.guard_ready(world) {
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
