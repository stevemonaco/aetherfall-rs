//! Townsfolk, signs, chests, warps, buildings and scenery.

use bevy::math::Vec2;

use super::effects::Particle;
use super::entity::{Entity, EntityBase};
use crate::art::hero::{Appearance, HeroPose, PersonSprite, people};
use crate::art::{bosses, props as art};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{Box2, IRect, snap};
use crate::core::gfx::{self, Batch, TexId};
use crate::entity_base;
use crate::gameplay::items;
use crate::scenes::world::World;
use crate::world::map::{EntityData, TILE, tile_center, tile_feet};

const T: f32 = TILE as f32;

pub struct Npc {
    base: EntityBase,
    look: Appearance,
    dialogue: String,
    home: Vec2,
    wander: f32,
    facing: Direction,
    move_dir: Vec2,
    timer: f32,
    walk_time: f32,
    pub name: String,
}

impl Npc {
    pub fn new(data: &EntityData) -> Npc {
        let pos = tile_feet(data.x as i32, data.y as i32);
        Npc {
            base: EntityBase { pos, solid: true, width: 12.0, height: 8.0, ..EntityBase::default() },
            look: people::get(data.str_or("sprite", "oldman1")),
            dialogue: data.str_or("dialogue", "").to_string(),
            home: pos,
            wander: data.float("wander", 0.0) * T,
            facing: Direction::parse(data.str("facing")),
            move_dir: Vec2::ZERO,
            timer: 0.0,
            walk_time: 0.0,
            name: data.str_or("name", "").to_string(),
        }
    }
}

impl Entity for Npc {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        if self.wander <= 0.0 || world.input_locked() {
            return;
        }
        self.timer -= dt;
        if self.timer <= 0.0 {
            let walk = world.rng.next_f32() < 0.5;
            self.move_dir = if walk { Direction::from_index(world.rng.below(4)).to_vec() } else { Vec2::ZERO };
            if (self.base.pos + self.move_dir * 16.0).distance(self.home) > self.wander {
                self.move_dir = Vec2::ZERO;
            }
            if self.move_dir != Vec2::ZERO {
                self.facing = Direction::from_vec(self.move_dir, self.facing);
            }
            self.timer = 0.8 + world.rng.next_f32() * 1.5;
        }
        if self.move_dir != Vec2::ZERO {
            world.move_entity(&mut self.base, self.move_dir * 26.0 * dt, false, false, true);
            self.walk_time += dt;
        }
    }

    fn interact(&mut self, world: &mut World) {
        self.facing = world.hero.facing.opposite();
        self.move_dir = Vec2::ZERO;
        self.timer = 2.0;
        world.start_dialogue(&self.dialogue, &self.name);
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        batch.draw(art::shadow(12), snap(self.base.pos - Vec2::new(6.0, 3.0)), Rgba::WHITE);
        let (pose, frame) = if self.move_dir != Vec2::ZERO {
            (HeroPose::Run, (self.walk_time * 9.0) as i32 % 6)
        } else {
            (HeroPose::Idle, (world.time * 1.4 + self.base.pos.x as f64) as i32 % 2)
        };
        PersonSprite::draw(batch, &self.look, self.base.pos, self.facing, pose, frame, Rgba::WHITE, false, 1.0, 0.0);
    }
}

pub struct Sign {
    base: EntityBase,
    text: String,
}

impl Sign {
    pub fn new(data: &EntityData) -> Sign {
        let pos = tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0);
        Sign { base: EntityBase { pos, solid: true, width: 12.0, height: 6.0, ..EntityBase::default() }, text: data.str_or("text", "...").to_string() }
    }
}

impl Entity for Sign {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn interact(&mut self, world: &mut World) {
        world.show_message(&self.text);
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        batch.draw(art::sign(), snap(self.base.pos - Vec2::new(8.0, 15.0)), Rgba::WHITE);
    }
}

pub struct Chest {
    base: EntityBase,
    flag: String,
    item: Option<String>,
    count: i32,
    gold: i32,
    style: i32,
    open_time: f32,
}

impl Chest {
    pub fn new(data: &EntityData, map_id: &str) -> Chest {
        let pos = tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0);
        let item = data.str("item").map(str::to_string);
        let sword = item.as_deref().is_some_and(|i| items::sword(i).is_some());
        Chest {
            base: EntityBase { pos, solid: true, width: 14.0, height: 8.0, ..EntityBase::default() },
            flag: data.str("flag").map(str::to_string).unwrap_or_else(|| format!("chest:{map_id}:{}_{}", data.x, data.y)),
            item,
            count: data.int("count", 1),
            gold: data.int("gold", 0),
            style: data.int("style", if sword { 3 } else { 0 }),
            open_time: -1.0,
        }
    }

    fn opened(&self, world: &World) -> bool {
        world.state.flags.contains(&self.flag)
    }
}

impl Entity for Chest {
    entity_base!();

    fn can_interact(&self, world: &World) -> bool {
        !self.opened(world)
    }

    fn added(&mut self, world: &mut World) {
        if self.opened(world) {
            self.open_time = 1.0;
        }
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        if self.open_time >= 0.0 && self.open_time < 1.0 {
            self.open_time += dt * 4.0;
        }
    }

    fn interact(&mut self, world: &mut World) {
        if self.opened(world) {
            return;
        }
        world.state.flags.insert(self.flag.clone());
        self.open_time = 0.0;
        crate::core::audio::sfx(crate::core::audio::Sound::ChestOpen);
        if let Some(item) = self.item.clone() {
            world.give_item(&item, self.count);
        }
        if self.gold > 0 {
            world.state.gold += self.gold;
            world.show_message(&format!("Found {} gold!", self.gold));
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let texture = gfx::external("external/Objects/chests.png");
        let frame = if self.open_time < 0.0 { 0 } else { ((self.open_time * 3.99) as i32).min(3) };
        let src = IRect::new(frame * 16, self.style * 16, 16, 16);
        batch.draw_src(texture, snap(self.base.pos - Vec2::new(8.0, 14.0)), Some(src), Rgba::WHITE);
    }
}

/// An invisible trigger that moves the player to another map (or another spot on this one).
pub struct Warp {
    base: EntityBase,
    art: Option<TexId>,
    pub target: String,
    pub target_tile: (i32, i32),
    pub target_facing: Direction,
    pub door: bool,
    pub condition: Option<String>,
    pub blocked_text: Option<String>,
}

impl Warp {
    pub fn new(data: &EntityData) -> Warp {
        let (w, h) = (data.int("w", 1), data.int("h", 1));
        let pos = Vec2::new((data.x + w as f32 / 2.0) * T, (data.y + h as f32) * T);
        Warp {
            base: EntityBase { pos, width: (w * TILE - 6) as f32, height: (h * TILE - 6) as f32, ..EntityBase::default() },
            art: match data.str("art") {
                Some("stairs_down") => Some(art::stairs_down()),
                Some("stairs_up") => Some(art::stairs_up()),
                _ => None,
            },
            target: data.str_or("to", "").to_string(),
            target_tile: (data.int("tx", 0), data.int("ty", 0)),
            target_facing: Direction::parse(Some(data.str_or("facing", "down"))),
            door: data.str("sound") == Some("door"),
            condition: data.str("if").map(str::to_string),
            blocked_text: data.str("blocked").map(str::to_string),
        }
    }
}

impl Entity for Warp {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn touch(&mut self, world: &mut World) {
        world.try_warp(self.condition.as_deref(), self.blocked_text.as_deref(), &self.target, self.target_tile, self.target_facing, self.door);
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        if let Some(art) = self.art {
            batch.draw(art, snap(Vec2::new(self.base.pos.x - 8.0, self.base.pos.y - T)), Rgba::WHITE);
        }
    }
}

pub struct Building {
    base: EntityBase,
    texture: TexId,
    tiles: IRect,
    restore: Option<String>,
    solid: bool,
    appear: f32,
    pub door_tile: (i32, i32),
}

impl Building {
    pub fn new(data: &EntityData) -> Building {
        let (w, h) = (data.int("w", 3), data.int("h", 3));
        let door = data.int("door", w / 2);
        let tiles = IRect::new(data.x as i32, data.y as i32, w, h);
        let center_x = tiles.x + tiles.w / 2;
        Building {
            base: EntityBase { pos: Vec2::new((center_x * TILE) as f32, (tiles.bottom() * TILE) as f32), width: 0.0, height: 0.0, ..EntityBase::default() },
            texture: art::house(w, h, door, data.str_or("roof", "red")),
            tiles,
            restore: data.str("restore").map(str::to_string),
            solid: false,
            appear: 1.0,
            door_tile: (tiles.x + door, tiles.bottom() - 1),
        }
    }

    /// A building whose rift is still open stands as a faded outline that can be walked through.
    fn restored(&self, world: &World) -> bool {
        self.restore.as_deref().is_none_or(|r| world.state.check(Some(r)))
    }

    fn solidify(&mut self, world: &mut World) {
        self.solid = true;
        for y in self.tiles.y..self.tiles.bottom() {
            for x in self.tiles.x..self.tiles.right() {
                if (x, y) != self.door_tile {
                    world.map.block(x, y, true);
                }
            }
        }
    }
}

impl Entity for Building {
    entity_base!();

    fn added(&mut self, world: &mut World) {
        if self.restored(world) {
            self.solidify(world);
        } else {
            self.appear = 0.0;
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        if self.solid {
            self.appear = (self.appear + dt).min(1.0);
            return;
        }
        let t = self.tiles;
        let area = Box2::new((t.x * TILE) as f32, (t.y * TILE) as f32, (t.w * TILE) as f32, (t.h * TILE) as f32);
        if self.restored(world) && !area.intersects(&world.hero.hitbox) {
            self.solidify(world);
            crate::core::audio::play(crate::core::audio::Sound::ChargeLevel, 0.8, -0.2);
            Particle::burst(world, area.center(), Rgba::rgb(255, 240, 200), 30, 90.0, 0.8, true);
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let (_, th) = gfx::size(self.texture);
        let at = Vec2::new((self.tiles.x * TILE) as f32, (self.tiles.y * TILE - (th - self.tiles.h * TILE)) as f32);
        if self.solid {
            batch.draw(self.texture, at, Rgba::WHITE.mul(0.4 + 0.6 * self.appear));
            return;
        }
        let shimmer = 0.18 + 0.06 * (world.time as f32 * 2.0 + self.tiles.x as f32).sin();
        batch.draw(self.texture, at, Rgba::rgb(200, 180, 255).mul(shimmer));
    }
}

/// Static scenery: trees, rocks, bushes, pots and the like. Solid ones block their base tile.
pub struct Decor {
    base: EntityBase,
    texture: TexId,
    offset: Vec2,
    blocks_tile: bool,
    dialogue: Option<String>,
    speaker: String,
    restore: Option<String>,
    blocked: bool,
    pub kind: String,
}

impl Decor {
    pub fn new(data: &EntityData) -> Decor {
        let kind = data.str_or("kind", "tree").to_string();
        let (texture, offset, blocks_tile) = match kind.as_str() {
            "tree" => (art::tree(), Vec2::new(-16.0, -36.0), true),
            "bush" => (art::bush(), Vec2::new(-8.0, -15.0), true),
            "boulder" => (art::boulder(), Vec2::new(-8.0, -15.0), true),
            "pot" => (art::pot(), Vec2::new(-8.0, -15.0), true),
            "well" => (art::well(), Vec2::new(-16.0, -30.0), true),
            "torch" => (art::torch(), Vec2::new(-4.0, -16.0), true),
            "cave" => (art::cave_mouth(), Vec2::new(-16.0, -24.0), false),
            "pod" => (art::pod(), Vec2::new(-8.0, -25.0), true),
            "bed" => (art::bed(), Vec2::new(-8.0, -25.0), true),
            "table" => (art::table(), Vec2::new(-8.0, -15.0), true),
            "shelf" => (art::shelf(), Vec2::new(-8.0, -25.0), true),
            "counter" => (art::counter(), Vec2::new(-8.0, -17.0), true),
            "plant" => (art::plant(), Vec2::new(-8.0, -17.0), true),
            "rug" => (art::rug(), Vec2::new(-8.0, -15.0), false),
            "altar" => (bosses::altar(), Vec2::new(-16.0, -27.0), true),
            _ => (art::boulder(), Vec2::new(-8.0, -15.0), true),
        };
        let (tx, ty) = (data.x as i32, data.y as i32);
        let dialogue = data.str("dialogue").map(str::to_string);
        let size = if dialogue.is_some() { T } else { 0.0 };
        Decor {
            base: EntityBase { pos: Vec2::new(tx as f32 * T + T / 2.0, (ty * TILE + TILE - 1) as f32), width: size, height: size, ..EntityBase::default() },
            texture,
            offset,
            blocks_tile,
            dialogue,
            speaker: data.str_or("name", "").to_string(),
            restore: data.str("restore").map(str::to_string),
            blocked: false,
            kind,
        }
    }

    fn restored(&self, world: &World) -> bool {
        self.restore.as_deref().is_none_or(|r| world.state.check(Some(r)))
    }

    fn block_tiles(&mut self, world: &mut World) {
        self.blocked = true;
        let (tx, ty) = ((self.base.pos.x / T) as i32, (self.base.pos.y / T) as i32);
        world.map.block(tx, ty, true);
        if self.kind == "well" {
            world.map.block(tx + 1, ty, true);
        }
        if self.kind == "altar" {
            world.map.block(tx - 1, ty, true);
            world.map.block(tx + 1, ty, true);
        }
    }
}

impl Entity for Decor {
    entity_base!();

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        (self.kind == "torch").then_some((64.0, Rgba::rgb(255, 170, 90)))
    }

    fn layer(&self) -> i32 {
        if self.kind == "rug" { 0 } else { 1 }
    }

    fn sort_y(&self) -> f32 {
        if self.kind == "cave" { self.base.pos.y - T } else { self.base.pos.y }
    }

    /// Counters relay talk to the shopkeeper standing behind them.
    fn can_interact(&self, world: &World) -> bool {
        self.dialogue.is_some() && self.restored(world)
    }

    fn interact(&mut self, world: &mut World) {
        if let Some(d) = self.dialogue.clone() {
            world.start_dialogue(&d, &self.speaker);
        }
    }

    fn added(&mut self, world: &mut World) {
        if self.blocks_tile && self.restored(world) {
            self.block_tiles(world);
        }
    }

    fn update(&mut self, world: &mut World, _dt: f32) {
        if self.blocks_tile && !self.blocked && self.restored(world) && !self.base.hitbox().inflate(8.0).intersects(&world.hero.hitbox) {
            self.block_tiles(world);
            Particle::burst(world, self.base.pos - Vec2::new(0.0, 8.0), Rgba::rgb(255, 240, 200), 20, 70.0, 0.6, true);
        }
        if self.kind == "torch" && world.rng.next_f32() < 0.5 {
            let at = self.base.pos + Vec2::new(world.rng.range(-1, 2) as f32, -16.0);
            let color = if world.rng.next_f32() < 0.5 { Rgba::rgb(255, 200, 60) } else { Rgba::rgb(255, 110, 40) };
            world.add(Particle::new(at, Vec2::new(0.0, -18.0), color, 0.35));
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let offset = if self.kind == "well" { self.offset + Vec2::new(8.0, 0.0) } else { self.offset };
        let tint = if self.restored(world) {
            Rgba::WHITE
        } else {
            Rgba::rgb(200, 180, 255).mul(0.18 + 0.06 * (world.time as f32 * 2.0 + self.base.pos.x).sin())
        };
        batch.draw(self.texture, snap(self.base.pos + offset), tint);
    }
}
