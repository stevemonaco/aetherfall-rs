//! Gates, switches, locked doors, save crystals, triggers and bridges.

use bevy::math::Vec2;

use super::effects::{Particle, Puff};
use super::entity::{Entity, EntityBase};
use crate::art::{combat as art_combat, props as art};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::Batch;
use crate::entity_base;
use crate::gameplay::items;
use crate::scenes::world::World;
use crate::world::map::{EntityData, TILE, Terrain, tile_feet};

const T: f32 = TILE as f32;

/// Iron bars (or a wall of rift-thorns) that are open exactly while their `open` condition holds.
pub struct Gate {
    base: EntityBase,
    tile: (i32, i32),
    open_condition: String,
    open_amount: f32,
    thorns: bool,
}

impl Gate {
    pub fn new(data: &EntityData) -> Gate {
        let tile = data.tile();
        Gate {
            base: EntityBase::at(Vec2::new(tile.0 as f32 * T + T / 2.0, ((tile.1 + 1) * TILE) as f32)),
            tile,
            open_condition: data.str_or("open", "never").to_string(),
            open_amount: 0.0,
            thorns: data.str("look") == Some("thorns"),
        }
    }

    fn should_be_open(&self, world: &World) -> bool {
        world.state.check(Some(&self.open_condition))
    }
}

impl Entity for Gate {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn added(&mut self, world: &mut World) {
        let open = self.should_be_open(world);
        self.open_amount = if open { 1.0 } else { 0.0 };
        world.map.block(self.tile.0, self.tile.1, !open);
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        let open = self.should_be_open(world);
        let target = if open { 1.0 } else { 0.0 };
        if self.open_amount != target {
            if self.open_amount == if open { 0.0 } else { 1.0 } {
                audio::sfx(Sound::Door);
            }
            self.open_amount = (self.open_amount + if open { dt } else { -dt } * 3.0).clamp(0.0, 1.0);
        }
        world.map.block(self.tile.0, self.tile.1, !open);
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let visible = (T * (1.0 - self.open_amount)) as i32;
        if visible <= 0 {
            return;
        }
        if self.thorns {
            let tint = Rgba::rgb(220, 170, 255).mul(1.0 - self.open_amount);
            batch.draw(art::thorns(), Vec2::new((self.tile.0 * TILE) as f32, (self.tile.1 * TILE) as f32), tint);
            return;
        }
        let dest = IRect::new(self.tile.0 * TILE, self.tile.1 * TILE, TILE, visible);
        batch.draw_dest(art::gate(), dest, Some(IRect::new(0, TILE - visible, TILE, visible)), Rgba::WHITE);
    }
}

pub struct FloorSwitch {
    base: EntityBase,
    flag: String,
    message: Option<String>,
}

impl FloorSwitch {
    pub fn new(data: &EntityData) -> FloorSwitch {
        FloorSwitch {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                width: 8.0,
                height: 6.0,
                ..EntityBase::default()
            },
            flag: data.str_or("flag", "").to_string(),
            message: data.str("message").map(str::to_string),
        }
    }
}

impl Entity for FloorSwitch {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn touch(&mut self, world: &mut World) {
        if world.state.flags.insert(self.flag.clone()) {
            audio::sfx(Sound::Door);
            world.camera.shake(0.3, 1.0);
            if let Some(m) = self.message.clone() {
                world.show_message(&m);
            }
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        batch.draw(
            art::floor_switch(world.state.flags.contains(&self.flag)),
            snap(self.base.pos - Vec2::new(8.0, 14.0)),
            Rgba::WHITE,
        );
    }
}

pub struct LockedDoor {
    base: EntityBase,
    tile: (i32, i32),
    key: String,
    flag: String,
}

impl LockedDoor {
    pub fn new(data: &EntityData, map_id: &str) -> LockedDoor {
        let tile = data.tile();
        LockedDoor {
            base: EntityBase {
                pos: Vec2::new(tile.0 as f32 * T + T / 2.0, ((tile.1 + 1) * TILE) as f32),
                width: T,
                height: T,
                ..EntityBase::default()
            },
            tile,
            key: data.str_or("key", "grotto_key").to_string(),
            flag: data
                .str("flag")
                .map(str::to_string)
                .unwrap_or_else(|| format!("door:{map_id}:{}_{}", tile.0, tile.1)),
        }
    }
}

impl Entity for LockedDoor {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn added(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag) {
            self.base.removed = true;
        } else {
            world.map.block(self.tile.0, self.tile.1, true);
        }
    }

    fn interact(&mut self, world: &mut World) {
        if !world.state.take(&self.key, 1) {
            world.show_message("The door is locked tight.");
            return;
        }
        world.state.flags.insert(self.flag.clone());
        world.map.block(self.tile.0, self.tile.1, false);
        self.base.removed = true;
        audio::sfx(Sound::Door);
        world.add(Puff::new(self.base.hitbox().center(), Rgba::rgb(200, 170, 120)));
        world.toast(format!("Used the {}.", items::name_of(&self.key)));
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        batch.draw(
            art::locked_door(),
            Vec2::new((self.tile.0 * TILE) as f32, (self.tile.1 * TILE) as f32),
            Rgba::WHITE,
        );
    }
}

/// Records the game; coming near it makes it a fast-travel destination.
pub struct SaveCrystal {
    base: EntityBase,
    time: f32,
}

impl SaveCrystal {
    pub fn new(data: &EntityData) -> SaveCrystal {
        SaveCrystal {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 10.0,
                height: 6.0,
                ..EntityBase::default()
            },
            time: 0.0,
        }
    }

    fn flag(&self, world: &World) -> String {
        let (tx, ty) = self.base.tile_pos();
        format!("crystal:{}:{tx}_{ty}", world.map_id())
    }
}

impl Entity for SaveCrystal {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((48.0, Rgba::rgb(140, 210, 255)))
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if world.hero.pos.distance(self.base.pos) < 40.0 && world.state.flags.insert(self.flag(world)) {
            world.toast("Crystal attuned: you can travel here.");
        }
        if world.rng.next_f32() < 0.08 {
            let at = self.base.pos + Vec2::new(world.rng.range(-6, 7) as f32, -10.0);
            world.add(Particle::new(at, Vec2::new(0.0, -14.0), Rgba::rgb(160, 230, 255), 0.8).spark());
        }
    }

    fn interact(&mut self, world: &mut World) {
        audio::play(Sound::ChargeLevel, 0.7, 0.3);
        world.state.flags.insert(self.flag(world));
        world.save_game();
        world.queue(Box::new(|_| Box::new(crate::scenes::crystal::CrystalScene::new())));
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let bob = ((self.time * 2.5).sin() * 1.5).round();
        batch.draw(art::shadow(10), snap(self.base.pos - Vec2::new(5.0, 2.0)), Rgba::WHITE);
        batch.draw(art::save_crystal(), snap(self.base.pos - Vec2::new(6.0, 26.0 + bob)), Rgba::WHITE);
    }
}

/// Runs a dialogue once when the player steps on it, optionally only while a condition holds.
pub struct Trigger {
    base: EntityBase,
    dialogue: String,
    flag: String,
    condition: Option<String>,
}

impl Trigger {
    pub fn new(data: &EntityData, map_id: &str) -> Trigger {
        let (w, h) = (data.int("w", 1), data.int("h", 1));
        Trigger {
            base: EntityBase {
                pos: Vec2::new((data.x + w as f32 / 2.0) * T, (data.y + h as f32) * T),
                width: (w * TILE) as f32,
                height: (h * TILE) as f32,
                ..EntityBase::default()
            },
            dialogue: data.str_or("dialogue", "").to_string(),
            flag: data
                .str("flag")
                .map(str::to_string)
                .unwrap_or_else(|| format!("trigger:{map_id}:{}_{}", data.x, data.y)),
            condition: data.str("if").map(str::to_string),
        }
    }
}

impl Entity for Trigger {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn touch(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag) || !world.state.check(self.condition.as_deref()) {
            return;
        }
        world.state.flags.insert(self.flag.clone());
        world.start_dialogue(&self.dialogue.clone(), "");
    }

    fn draw(&self, _world: &World, _batch: &mut Batch) {}
}

/// Planks laid across water, typically once a rift is sealed; the tiles beneath become bridge.
pub struct SpanBridge {
    base: EntityBase,
    tiles: IRect,
    rubble: bool,
}

impl SpanBridge {
    pub fn new(data: &EntityData) -> SpanBridge {
        let tiles = IRect::new(data.x as i32, data.y as i32, data.int("w", 1), data.int("h", 1));
        let center_x = tiles.x + tiles.w / 2;
        SpanBridge {
            base: EntityBase {
                pos: Vec2::new((center_x * TILE) as f32, (tiles.bottom() * TILE) as f32),
                width: 0.0,
                height: 0.0,
                ..EntityBase::default()
            },
            tiles,
            rubble: data.str("look") == Some("rubble"),
        }
    }
}

impl Entity for SpanBridge {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn added(&mut self, world: &mut World) {
        for y in self.tiles.y..self.tiles.bottom() {
            for x in self.tiles.x..self.tiles.right() {
                world.map.set_terrain(x, y, if self.rubble { Terrain::Floor } else { Terrain::Bridge });
            }
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let horizontal = self.tiles.w >= self.tiles.h;
        let tex = if self.rubble { art_combat::rubble() } else { art_combat::planks(horizontal) };
        for y in self.tiles.y..self.tiles.bottom() {
            for x in self.tiles.x..self.tiles.right() {
                batch.draw(tex, Vec2::new((x * TILE) as f32, (y * TILE) as f32), Rgba::WHITE);
            }
        }
    }
}

/// A crossing only the hawk can see: while the spirit flies as a hawk these tiles become ghostly planks the hero can
/// walk on, and they fade again once it changes form (never from under the hero's feet).
pub struct HiddenPath {
    base: EntityBase,
    tiles: IRect,
    original: Vec<Terrain>,
    shown: bool,
    time: f32,
}

impl HiddenPath {
    pub fn new(data: &EntityData) -> HiddenPath {
        let tiles = IRect::new(data.x as i32, data.y as i32, data.int("w", 1), data.int("h", 1));
        HiddenPath {
            base: EntityBase {
                pos: Vec2::new((tiles.x * TILE) as f32 + T / 2.0, (tiles.bottom() * TILE) as f32),
                width: 0.0,
                height: 0.0,
                ..EntityBase::default()
            },
            tiles,
            original: Vec::new(),
            shown: false,
            time: 0.0,
        }
    }

    fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (self.tiles.y..self.tiles.bottom()).flat_map(move |y| (self.tiles.x..self.tiles.right()).map(move |x| (x, y)))
    }
}

impl Entity for HiddenPath {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn added(&mut self, world: &mut World) {
        self.original = self.cells().map(|(x, y)| world.map.at(x, y)).collect();
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        let hawk = world.state.soul.as_deref() == Some("hawk");
        if hawk == self.shown {
            return;
        }
        let area = crate::core::geom::Box2::new(
            (self.tiles.x * TILE) as f32,
            (self.tiles.y * TILE) as f32,
            (self.tiles.w * TILE) as f32,
            (self.tiles.h * TILE) as f32,
        );
        if !hawk && world.hero.hitbox.intersects(&area) {
            return;
        }
        self.shown = hawk;
        let cells: Vec<(i32, i32)> = self.cells().collect();
        for (i, (x, y)) in cells.into_iter().enumerate() {
            world.map.set_terrain(x, y, if hawk { Terrain::Bridge } else { self.original[i] });
        }
        if hawk {
            audio::play(Sound::ChargeLevel, 0.5, 0.8);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        if !self.shown {
            return;
        }
        let tint = Rgba::rgb(255, 225, 160).mul(0.55 + (self.time * 3.0).sin() * 0.15);
        let tex = art_combat::planks(self.tiles.w >= self.tiles.h);
        for (x, y) in self.cells() {
            batch.draw(tex, Vec2::new((x * TILE) as f32, (y * TILE) as f32), tint);
        }
    }
}

/// A crystal on a pedestal that answers only to a bolt or skill of one element (any element when it has none), never a
/// sword swing. Its flag stays set, or
/// for `timer` seconds when it has one, so a timed crystal sends the hero racing to whatever it opened.
pub struct CrystalSwitch {
    base: EntityBase,
    flag: String,
    element: crate::gameplay::element::Element,
    timer: f32,
    left: f32,
    time: f32,
}

impl CrystalSwitch {
    pub fn new(data: &EntityData) -> CrystalSwitch {
        CrystalSwitch {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 10.0,
                height: 6.0,
                ..EntityBase::default()
            },
            flag: data.str_or("flag", "crystal").to_string(),
            element: crate::gameplay::element::Element::parse(data.str("element")),
            timer: data.float("timer", 0.0),
            left: 0.0,
            time: 0.0,
        }
    }

    fn lit(&self, world: &World) -> bool {
        world.state.flags.contains(&self.flag)
    }

    fn color(&self) -> Rgba {
        if self.element == crate::gameplay::element::Element::None {
            Rgba::rgb(230, 230, 245)
        } else {
            self.element.glow()
        }
    }
}

impl Entity for CrystalSwitch {
    entity_base!();

    fn faction(&self) -> Option<crate::gameplay::combat::Faction> {
        Some(crate::gameplay::combat::Faction::Neutral)
    }

    fn hurtbox(&self) -> crate::core::geom::Box2 {
        crate::core::geom::Box2::new(self.base.pos.x - 6.0, self.base.pos.y - 20.0, 12.0, 20.0)
    }

    fn light(&self, world: &World) -> Option<(f32, Rgba)> {
        Some((if self.lit(world) { 44.0 } else { 20.0 }, self.color()))
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
        if self.timer > 0.0 {
            world.state.flags.remove(&self.flag);
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if self.left > 0.0 {
            self.left -= dt;
            if self.left <= 0.0 {
                world.state.flags.remove(&self.flag);
                audio::play(Sound::MenuCancel, 0.6, -0.3);
            }
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut crate::gameplay::combat::Hit) -> bool {
        use crate::gameplay::element::Element;
        if hit.source != crate::gameplay::combat::Faction::Player {
            return false;
        }
        let fits = self.element == Element::None
            || hit.element == self.element
            || (hit.aether && hit.element != Element::None);
        let ranged = matches!(
            hit.kind,
            crate::gameplay::combat::HitKind::Bolt
                | crate::gameplay::combat::HitKind::ChargedBolt
                | crate::gameplay::combat::HitKind::Skill
        );
        if !fits || !ranged {
            audio::play(Sound::Clink, 0.6, -0.2);
            return true;
        }
        if self.timer > 0.0 {
            self.left = self.timer;
        }
        if world.state.flags.insert(self.flag.clone()) {
            audio::sfx(Sound::Door);
            world.camera.shake(0.15, 1.0);
            Particle::burst(world, self.base.pos - Vec2::new(0.0, 12.0), self.color(), 14, 50.0, 0.4, true);
        }
        true
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let lit = self.lit(world);
        let bob = if lit { (self.time * 3.0).sin().round() } else { 0.0 };
        batch.draw(art::shadow(10), snap(self.base.pos - Vec2::new(5.0, 2.0)), Rgba::WHITE);
        let tint = if lit { self.color().lerp(Rgba::WHITE, 0.3) } else { self.color().mul(0.45) };
        batch.draw(art::crystal_switch(), snap(self.base.pos - Vec2::new(6.0, 22.0 + bob)), tint);
        if self.left > 0.0 && (self.left * 6.0) as i32 % 2 == 0 && self.left < 2.0 {
            batch.draw(art::crystal_switch(), snap(self.base.pos - Vec2::new(6.0, 22.0 + bob)), Rgba::WHITE.mul(0.4));
        }
    }
}

/// An orb that flips its flag each time it is struck. Gates opened by `flag` and by `!flag` take turns, so one strike
/// lowers one set of bars and raises the other.
pub struct ToggleSwitch {
    base: EntityBase,
    flag: String,
    cooldown: f32,
}

impl ToggleSwitch {
    pub fn new(data: &EntityData) -> ToggleSwitch {
        ToggleSwitch {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 10.0,
                height: 6.0,
                ..EntityBase::default()
            },
            flag: data.str_or("flag", "toggle").to_string(),
            cooldown: 0.0,
        }
    }

    fn on(&self, world: &World) -> bool {
        world.state.flags.contains(&self.flag)
    }
}

impl Entity for ToggleSwitch {
    entity_base!();

    fn faction(&self) -> Option<crate::gameplay::combat::Faction> {
        Some(crate::gameplay::combat::Faction::Neutral)
    }

    fn hurtbox(&self) -> crate::core::geom::Box2 {
        crate::core::geom::Box2::new(self.base.pos.x - 6.0, self.base.pos.y - 16.0, 12.0, 16.0)
    }

    fn light(&self, world: &World) -> Option<(f32, Rgba)> {
        Some((28.0, if self.on(world) { Rgba::rgb(120, 160, 255) } else { Rgba::rgb(255, 110, 100) }))
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut crate::gameplay::combat::Hit) -> bool {
        if hit.source != crate::gameplay::combat::Faction::Player || self.cooldown > 0.0 {
            return hit.source == crate::gameplay::combat::Faction::Player;
        }
        self.cooldown = 0.5;
        if !world.state.flags.remove(&self.flag) {
            world.state.flags.insert(self.flag.clone());
        }
        audio::play(Sound::Clink, 0.9, 0.4);
        let color = if self.on(world) { Rgba::rgb(120, 160, 255) } else { Rgba::rgb(255, 110, 100) };
        Particle::burst(world, self.base.pos - Vec2::new(0.0, 10.0), color, 10, 40.0, 0.3, true);
        true
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let color = if self.on(world) { Rgba::rgb(120, 160, 255) } else { Rgba::rgb(255, 110, 100) };
        batch.draw(art::shadow(10), snap(self.base.pos - Vec2::new(5.0, 2.0)), Rgba::WHITE);
        batch.draw(art::toggle_post(), snap(self.base.pos - Vec2::new(6.0, 16.0)), Rgba::WHITE);
        batch.draw(art::orb(), snap(self.base.pos - Vec2::new(4.0, 19.0)), color);
    }
}
