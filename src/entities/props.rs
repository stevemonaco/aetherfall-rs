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
use crate::gameplay::game_data::data;
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
    /// Some(true) for a quest or gift waiting, Some(false) for news; None when they've nothing new.
    marker: Option<bool>,
    pub name: String,
}

/// Whether talking now would change anything, judged from the dialogue entry that would play: a flag not yet set,
/// a quest not yet started, or a gift. Returns whether it holds a gift or quest, or is only news.
fn marker(world: &World, dialogue: &str) -> Option<bool> {
    let entry = data().dialogue.get(dialogue)?.iter().find(|e| world.state.check(e.condition.as_deref()))?;
    let actions = entry.actions.iter().chain(&entry.yes);
    let (mut new, mut gift, mut guarded) = (false, false, false);
    for action in actions {
        let (kind, arg) = action.split_once(':').unwrap_or((action, ""));
        match kind {
            "flag" | "quest" => {
                guarded = true;
                new |= !world.state.check(Some(&format!("{}{arg}", if kind == "quest" { "quest:" } else { "" })));
                gift |= kind == "quest";
            }
            "give" | "skill" => gift = true,
            "gold" => gift |= !arg.starts_with('-'),
            _ => {}
        }
    }
    (new || (gift && !guarded)).then_some(gift)
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
            marker: None,
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
        self.marker = marker(world, &self.dialogue);
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
        if let Some(gift) = self.marker {
            let bob = ((world.time * 3.0).sin() * 1.5) as f32;
            batch.draw(art::npc_marker(gift), snap(self.base.pos + Vec2::new(-5.0, -40.0 + bob)), Rgba::WHITE);
        }
    }
}

pub struct Sign {
    base: EntityBase,
    text: String,
}

impl Sign {
    pub fn new(data: &EntityData) -> Sign {
        let pos = tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0);
        Sign {
            base: EntityBase { pos, solid: true, width: 12.0, height: 6.0, ..EntityBase::default() },
            text: data.str_or("text", "...").to_string(),
        }
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
        let sigil = item.as_deref().is_some_and(|i| items::sigil(i).is_some());
        Chest {
            base: EntityBase { pos, solid: true, width: 14.0, height: 8.0, ..EntityBase::default() },
            flag: data
                .str("flag")
                .map(str::to_string)
                .unwrap_or_else(|| format!("chest:{map_id}:{}_{}", data.x, data.y)),
            item,
            count: data.int("count", 1),
            gold: data.int("gold", 0),
            style: data.int("style", if sigil { 3 } else { 0 }),
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

/// A trigger that moves the player to another map (or another spot on this one). Most fire when walked into; a boat
/// (`art: skiff`) waits on the water instead and asks before casting off.
pub struct Warp {
    base: EntityBase,
    art: Option<TexId>,
    boat: bool,
    prompt: String,
    time: f32,
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
            base: EntityBase {
                pos,
                width: (w * TILE - 6) as f32,
                height: (h * TILE - 6) as f32,
                ..EntityBase::default()
            },
            art: match data.str("art") {
                Some("stairs_down") => Some(art::stairs_down()),
                Some("stairs_up") => Some(art::stairs_up()),
                Some("skiff") => Some(art::skiff()),
                _ => None,
            },
            boat: data.str("art") == Some("skiff"),
            prompt: data.str_or("prompt", "Take the boat?").to_string(),
            time: 0.0,
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

    fn can_interact(&self, _world: &World) -> bool {
        self.boat
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.time += dt;
    }

    fn interact(&mut self, world: &mut World) {
        if !world.state.check(self.condition.as_deref()) {
            let text = self.blocked_text.clone().unwrap_or_else(|| "The boat won't budge.".into());
            world.show_message(&text);
            return;
        }
        let facing = match self.target_facing {
            Direction::Up => "up",
            Direction::Left => "left",
            Direction::Right => "right",
            Direction::Down => "down",
        };
        let warp = format!("warp:{},{},{},{facing}", self.target, self.target_tile.0, self.target_tile.1);
        world.confirm(self.prompt.clone(), vec![warp]);
    }

    fn touch(&mut self, world: &mut World) {
        if self.boat {
            return;
        }
        world.try_warp(
            self.condition.as_deref(),
            self.blocked_text.as_deref(),
            &self.target,
            self.target_tile,
            self.target_facing,
            self.door,
        );
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let Some(art) = self.art else { return };
        if self.boat {
            let bob = (self.time * 2.0).sin().round();
            batch.draw(art, snap(Vec2::new(self.base.pos.x - 16.0, self.base.pos.y - T + bob)), Rgba::WHITE);
        } else {
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
    /// How far a restored building has rebuilt itself, tile by tile from the ground up (0 to 1).
    appear: f32,
    /// Tiles already set in place, so each new one can puff as it lands.
    built: usize,
    pub door_tile: (i32, i32),
    has_door: bool,
}

impl Building {
    pub fn new(data: &EntityData) -> Building {
        let (w, h) = (data.int("w", 3), data.int("h", 3));
        // Only buildings that lead somewhere get a door; the rest are solid all the way along.
        let door = if data.str("to").is_some() { data.int("door", w / 2) } else { -1 };
        let tiles = IRect::new(data.x as i32, data.y as i32, w, h);
        let center_x = tiles.x + tiles.w / 2;
        Building {
            base: EntityBase {
                pos: Vec2::new((center_x * TILE) as f32, (tiles.bottom() * TILE) as f32),
                width: 0.0,
                height: 0.0,
                ..EntityBase::default()
            },
            texture: art::house(w, h, door, data.str_or("roof", "red")),
            tiles,
            restore: data.str("restore").map(str::to_string),
            solid: false,
            appear: 1.0,
            built: usize::MAX,
            door_tile: (tiles.x + door.max(0), tiles.bottom() - 1),
            has_door: door >= 0,
        }
    }

    /// A building whose rift is still open stands as a faded outline that can be walked through.
    fn restored(&self, world: &World) -> bool {
        self.restore.as_deref().is_none_or(|r| world.state.check(Some(r)))
    }

    /// The texture's 16px cells, in the order they rebuild: bottom row first, left to right.
    fn cells(&self) -> Vec<(i32, i32)> {
        let (tw, th) = gfx::size(self.texture);
        let (cols, rows) = (tw / TILE, th / TILE);
        (0..rows).rev().flat_map(|r| (0..cols).map(move |c| (c, r))).collect()
    }

    fn origin(&self) -> Vec2 {
        let (_, th) = gfx::size(self.texture);
        Vec2::new((self.tiles.x * TILE) as f32, (self.tiles.y * TILE - (th - self.tiles.h * TILE)) as f32)
    }

    fn solidify(&mut self, world: &mut World) {
        self.solid = true;
        for y in self.tiles.y..self.tiles.bottom() {
            for x in self.tiles.x..self.tiles.right() {
                if !self.has_door || (x, y) != self.door_tile {
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
            self.built = 0;
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        if self.solid {
            self.appear = (self.appear + dt / 1.6).min(1.0);
            let cells = self.cells();
            let now = (self.appear * cells.len() as f32) as usize;
            while self.built < now.min(cells.len()) {
                let (c, r) = cells[self.built];
                let at = self.origin() + Vec2::new((c * TILE) as f32 + 8.0, (r * TILE) as f32 + 8.0);
                Particle::burst(world, at, Rgba::rgb(240, 225, 200), 4, 30.0, 0.35, false);
                self.built += 1;
                if self.built.is_multiple_of(3) {
                    crate::core::audio::play(crate::core::audio::Sound::Clink, 0.35, 0.3);
                }
            }
            return;
        }
        let t = self.tiles;
        let area = Box2::new((t.x * TILE) as f32, (t.y * TILE) as f32, (t.w * TILE) as f32, (t.h * TILE) as f32);
        if self.restored(world) && !area.intersects(&world.hero.hitbox) {
            self.solidify(world);
            crate::core::audio::play(crate::core::audio::Sound::ChargeLevel, 0.8, -0.2);
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let at = self.origin();
        let shimmer = 0.18 + 0.06 * (world.time as f32 * 2.0 + self.tiles.x as f32).sin();
        let ghost = Rgba::rgb(200, 180, 255).mul(shimmer);
        if !self.solid {
            batch.draw(self.texture, at, ghost);
            return;
        }
        if self.appear >= 1.0 {
            batch.draw(self.texture, at, Rgba::WHITE);
            return;
        }
        let cells = self.cells();
        let built = (self.appear * cells.len() as f32) as usize;
        for (i, &(c, r)) in cells.iter().enumerate() {
            let src = IRect::new(c * TILE, r * TILE, TILE, TILE);
            let pos = at + Vec2::new((c * TILE) as f32, (r * TILE) as f32);
            batch.draw_src(self.texture, pos, Some(src), if i < built { Rgba::WHITE } else { ghost });
        }
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
            "echo_stone" => (art::echo_stone(), Vec2::new(-8.0, -27.0), true),
            "weave_shrine" => (art::weave_shrine(), Vec2::new(-16.0, -27.0), true),
            "pact_shrine" => (art::pact_shrine(), Vec2::new(-8.0, -29.0), true),
            "forge" => (art::forge(), Vec2::new(-16.0, -27.0), true),
            _ => (art::boulder(), Vec2::new(-8.0, -15.0), true),
        };
        let (tx, ty) = (data.x as i32, data.y as i32);
        let dialogue = data.str("dialogue").map(str::to_string);
        let size = if dialogue.is_some() { T } else { 0.0 };
        Decor {
            base: EntityBase {
                pos: Vec2::new(tx as f32 * T + T / 2.0, (ty * TILE + TILE - 1) as f32),
                width: size,
                height: size,
                ..EntityBase::default()
            },
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
        if matches!(self.kind.as_str(), "altar" | "weave_shrine" | "forge") {
            world.map.block(tx - 1, ty, true);
            world.map.block(tx + 1, ty, true);
        }
    }
}

impl Entity for Decor {
    entity_base!();

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        match self.kind.as_str() {
            "torch" => Some((64.0, Rgba::rgb(255, 170, 90))),
            "echo_stone" => Some((40.0, Rgba::rgb(120, 230, 255))),
            "weave_shrine" => Some((44.0, Rgba::rgb(200, 170, 255))),
            "pact_shrine" => Some((40.0, Rgba::rgb(255, 90, 60))),
            "forge" => Some((56.0, Rgba::rgb(255, 150, 70))),
            _ => None,
        }
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
        if self.blocks_tile
            && !self.blocked
            && self.restored(world)
            && !self.base.hitbox().inflate(8.0).intersects(&world.hero.hitbox)
        {
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
