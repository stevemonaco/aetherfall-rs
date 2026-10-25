//! Push blocks, pressure plates, tether anchors and levers, memory crystals and Aether seals.

use bevy::math::Vec2;

use super::effects::{Particle, Puff};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, TetherResponse};
use super::hazards::SteamCloud;
use super::obstacles::Crate;
use super::puzzles::SpanBridge;
use crate::art::{combat as art_combat, props as art};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{Box2, IRect, safe_dir, snap};
use crate::core::gfx::Batch;
use crate::entity_base;
use crate::gameplay::combat::{Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::map::{EntityData, TILE, Terrain, tile_center, tile_feet};

const T: f32 = TILE as f32;

/// A pushable block of ice (or Concord stone): blows send it sliding until it hits something; it holds pressure plates
/// down, can be reeled in with the tether, and drops into holes, filling them for good. Ice melts in fire. Blocks with
/// an id remember where they came to rest.
pub struct IceBlock {
    base: EntityBase,
    stone: bool,
    map_id: String,
    id: Option<String>,
    slide: Vec2,
    melt: f32,
}

impl IceBlock {
    pub fn new(data: &EntityData, map_id: &str, stone: bool) -> IceBlock {
        IceBlock {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 8.0),
                solid: true,
                width: 14.0,
                height: 12.0,
                ..EntityBase::default()
            },
            stone,
            map_id: map_id.to_string(),
            id: data.str("id").map(str::to_string),
            slide: Vec2::ZERO,
            melt: 0.0,
        }
    }

    fn prefix(&self) -> Option<String> {
        self.id.as_ref().map(|id| format!("block:{}:{id}:", self.map_id))
    }

    fn remember(&self, world: &mut World, place: &str) {
        let Some(prefix) = self.prefix() else { return };
        world.state.flags.retain(|f| !f.starts_with(&prefix));
        world.state.flags.insert(format!("{prefix}{place}"));
    }

    fn push(&mut self, direction: Vec2) {
        if self.slide != Vec2::ZERO {
            return;
        }
        self.slide = Direction::from_vec(direction, Direction::Down).to_vec();
        audio::play(Sound::Swing, 0.6, -0.5);
    }

    fn melt_away(&mut self, world: &mut World) {
        self.base.removed = true;
        audio::sfx(Sound::Burn);
        Particle::burst(world, self.hurtbox().center(), Rgba::rgb(200, 240, 255), 18, 60.0, 0.5, false);
    }

    // A block sliding into a hole falls in and fills it, leaving walkable rubble behind.
    fn try_drop(&mut self, world: &mut World) -> bool {
        let center = self.base.hitbox().center() + self.slide * 9.0;
        let (tx, ty) = ((center.x / T).floor() as i32, (center.y / T).floor() as i32);
        if world.map.at(tx, ty) != Terrain::Pit || !world.map.in_bounds(tx, ty) {
            return false;
        }
        self.base.removed = true;
        world.state.flags.insert(format!("filled:{}:{tx}_{ty}", self.map_id));
        self.remember(world, "gone");
        let mut rubble = EntityData { kind: "span".into(), x: tx as f32, y: ty as f32, ..EntityData::default() };
        rubble.extra.insert("look".into(), "rubble".into());
        world.add(SpanBridge::new(&rubble));
        audio::play(Sound::Break, 0.9, -0.4);
        world.camera.shake(0.25, 2.0);
        Particle::burst(world, tile_center(tx, ty), Rgba::rgb(160, 150, 170), 20, 70.0, 0.5, false);
        true
    }

    fn snap_to_tile(&mut self) {
        let (tx, ty) = ((self.base.pos.x / T).floor() as i32, ((self.base.pos.y - 6.0) / T).floor() as i32);
        let target = tile_center(tx, ty) + Vec2::new(0.0, 8.0);
        if target.distance(self.base.pos) < 10.0 {
            self.base.pos = target;
        }
    }
}

impl Entity for IceBlock {
    entity_base!();

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Neutral)
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 8.0, self.base.pos.y - 16.0, 16.0, 16.0)
    }

    fn tether_box(&self) -> Option<Box2> {
        Some(self.hurtbox())
    }

    fn on_tether(&mut self, world: &mut World) -> TetherResponse {
        self.push(safe_dir(world.hero.pos - self.base.pos, Vec2::Y));
        TetherResponse::PullTarget
    }

    fn added(&mut self, world: &mut World) {
        let Some(prefix) = self.prefix() else { return };
        if world.state.flags.contains(&format!("{prefix}gone")) {
            self.base.removed = true;
            return;
        }
        let saved = world.state.flags.iter().find(|f| f.starts_with(&prefix)).cloned();
        if let Some(place) =
            saved.and_then(|s| s[prefix.len()..].split_once('_').map(|(x, y)| (x.parse().ok(), y.parse().ok())))
            && let (Some(x), Some(y)) = place
        {
            self.base.pos = tile_center(x, y) + Vec2::new(0.0, 8.0);
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if hit.element == Element::Fire && !self.stone {
            self.melt += 0.5;
            world.add(SteamCloud::new(self.base.pos - Vec2::new(0.0, 6.0), false));
            if self.melt >= 1.0 {
                self.melt_away(world);
            }
            return true;
        }
        if matches!(hit.kind, HitKind::Melee | HitKind::Finisher | HitKind::ChargedBolt | HitKind::Skill)
            || hit.element == Element::Wind
        {
            self.push(safe_dir(self.base.pos - hit.from, Vec2::Y));
        }
        true
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        if !self.stone {
            let (tx, ty) = self.base.tile_pos();
            for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                if world.is_burning(tx + dx, ty + dy) {
                    self.melt += dt * 0.4;
                }
            }
        }
        if self.melt >= 1.0 {
            self.melt_away(world);
            return;
        }
        if self.slide == Vec2::ZERO || self.try_drop(world) {
            return;
        }
        let moved = world.move_entity(&mut self.base, self.slide * 170.0 * dt, false, false, true);
        if moved.length_squared() < 0.01 {
            self.snap_to_tile();
            audio::play(Sound::Clink, 0.6, -0.6);
            world.camera.shake(0.08, 1.0);
            let ahead = self.base.hitbox().center() + self.slide * 12.0;
            let stats = world.state.stats(world.state.mode);
            for (id, _, hurtbox) in world.damageables() {
                if id != self.base.id && hurtbox.contains(ahead) {
                    let mut hit = Hit {
                        power: 1.2,
                        kind: HitKind::Environment,
                        from: self.base.hitbox().center(),
                        knockback: 200.0,
                        poise: 4.0,
                        ..Hit::new(Faction::Player, stats)
                    };
                    world.hit(id, &mut hit);
                }
            }
            self.slide = Vec2::ZERO;
            let rest = ((self.base.pos.x / T).floor() as i32, ((self.base.pos.y - 6.0) / T).floor() as i32);
            self.remember(world, &format!("{}_{}", rest.0, rest.1));
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        batch.draw(art::shadow(16), snap(self.base.pos - Vec2::new(8.0, 3.0)), Rgba::WHITE);
        let tex = if self.stone { art_combat::stone_block() } else { art_combat::ice_block() };
        batch.draw(
            tex,
            snap(self.base.pos - Vec2::new(8.0, 18.0)),
            Rgba::WHITE.lerp(Rgba::rgb(150, 190, 255), self.melt),
        );
    }
}

/// A weighted plate that is pressed while the player, a block or a crate rests on it. A heavy plate ignores the player.
pub struct PressurePlate {
    base: EntityBase,
    flag: String,
    latch: bool,
    heavy: bool,
    down: bool,
}

impl PressurePlate {
    pub fn new(data: &EntityData) -> PressurePlate {
        PressurePlate {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                width: 10.0,
                height: 8.0,
                ..EntityBase::default()
            },
            flag: data.str_or("flag", "plate").to_string(),
            latch: data.bool("latch", false),
            heavy: data.bool("heavy", false),
            down: false,
        }
    }
}

impl Entity for PressurePlate {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn added(&mut self, world: &mut World) {
        self.down = world.state.flags.contains(&self.flag);
    }

    fn update(&mut self, world: &mut World, _dt: f32) {
        let area = self.base.hitbox();
        let weighed = world.entities().any(|e| {
            let any = e.as_any();
            let heavy =
                any.is::<IceBlock>() || any.is::<Crate>() || any.downcast_ref::<Enemy>().is_some_and(|en| en.frozen());
            heavy && e.base().hitbox().intersects(&area)
        });
        let stood_on = !self.heavy && world.hero.hitbox.intersects(&area) && !world.hero.airborne;
        let pressed = stood_on || weighed;
        if pressed == self.down || (!pressed && self.latch) {
            return;
        }
        self.down = pressed;
        audio::play(if pressed { Sound::Door } else { Sound::MenuCancel }, 0.7, 0.0);
        if pressed {
            world.state.flags.insert(self.flag.clone());
        } else {
            world.state.flags.remove(&self.flag);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        batch.draw(art::floor_switch(self.down), snap(self.base.pos - Vec2::new(8.0, 14.0)), Rgba::WHITE);
    }
}

/// A post the tether can latch onto to pull the player across gaps.
pub struct TetherPost {
    base: EntityBase,
    glow: f32,
}

impl TetherPost {
    pub fn new(data: &EntityData) -> TetherPost {
        TetherPost {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0),
                solid: true,
                width: 8.0,
                height: 6.0,
                ..EntityBase::default()
            },
            glow: 0.0,
        }
    }
}

impl Entity for TetherPost {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn tether_box(&self) -> Option<Box2> {
        Some(Box2::new(self.base.pos.x - 7.0, self.base.pos.y - 24.0, 14.0, 24.0))
    }

    fn on_tether(&mut self, _world: &mut World) -> TetherResponse {
        self.glow = 0.6;
        TetherResponse::PullPlayer
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn interact(&mut self, world: &mut World) {
        world.show_message(if world.state.has_tether() {
            "An anchor post of sky-metal. Latch your tether onto it to pull yourself across."
        } else {
            "An anchor post of sky-metal, ringed for some kind of line."
        });
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.glow = (self.glow - dt).max(0.0);
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        batch.draw(art::shadow(10), snap(self.base.pos - Vec2::new(5.0, 2.0)), Rgba::WHITE);
        let at = snap(self.base.pos - Vec2::new(6.0, 26.0));
        batch.draw(art_combat::tether_post(), at, Rgba::WHITE);
        if self.glow > 0.0 {
            batch.draw(art_combat::tether_post(), at, Rgba::WHITE.mul(self.glow));
        }
    }
}

/// A lever out of reach: tug it with the tether to set its flag.
pub struct TetherLever {
    base: EntityBase,
    flag: String,
}

impl TetherLever {
    pub fn new(data: &EntityData) -> TetherLever {
        TetherLever {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0),
                width: 10.0,
                height: 6.0,
                ..EntityBase::default()
            },
            flag: data.str_or("flag", "lever").to_string(),
        }
    }
}

impl Entity for TetherLever {
    entity_base!();

    fn tether_box(&self) -> Option<Box2> {
        Some(Box2::new(self.base.pos.x - 7.0, self.base.pos.y - 16.0, 14.0, 16.0))
    }

    fn on_tether(&mut self, world: &mut World) -> TetherResponse {
        if !world.state.flags.insert(self.flag.clone()) {
            return TetherResponse::None;
        }
        world.camera.shake(0.2, 1.0);
        TetherResponse::Activated
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let pulled = world.state.flags.contains(&self.flag);
        let src = IRect::new(if pulled { 16 } else { 0 }, 0, 16, 16);
        batch.draw_src(art_combat::lever(), snap(self.base.pos - Vec2::new(8.0, 15.0)), Some(src), Rgba::WHITE);
    }
}

/// The flag set once the Bastion's crystals are restored, which lets memory crystals elsewhere be woken.
const CRYSTALS_RESTORED: &str = "rift:ash_forge_anvil";

/// A memory crystal: touching it shows this place in its other era (the map's pair). A crystal with a `key` sleeps
/// until an Aether Mote wakes it; both eras' crystals share the key, so waking one wakes its twin.
pub struct MemoryCrystal {
    base: EntityBase,
    key: Option<String>,
    time: f32,
}

impl MemoryCrystal {
    pub fn new(data: &EntityData) -> MemoryCrystal {
        MemoryCrystal {
            base: EntityBase {
                pos: tile_feet(data.x as i32, data.y as i32),
                solid: true,
                width: 10.0,
                height: 6.0,
                ..EntityBase::default()
            },
            key: data.str("key").map(|k| format!("crystal:{k}")),
            time: 0.0,
        }
    }

    fn awake(&self, world: &World) -> bool {
        self.key.as_ref().is_none_or(|k| world.state.flags.contains(k))
    }
}

impl Entity for MemoryCrystal {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn light(&self, world: &World) -> Option<(f32, Rgba)> {
        self.awake(world).then_some((50.0, Rgba::rgb(230, 200, 255)))
    }

    fn added(&mut self, world: &mut World) {
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if self.awake(world) && world.rng.next_f32() < 0.1 {
            let at = self.base.pos + Vec2::new(world.rng.range(-6, 7) as f32, -12.0);
            world.add(Particle::new(at, Vec2::new(0.0, -12.0), Rgba::rgb(230, 200, 255), 0.8).spark());
        }
    }

    fn interact(&mut self, world: &mut World) {
        let Some(key) = self.key.clone().filter(|_| !self.awake(world)) else {
            world.swap_era();
            return;
        };
        if !world.state.flags.contains(CRYSTALS_RESTORED) {
            world.show_message(
                "A memory crystal, dark and cold. Something in the Fallen Bastion once woke crystals like this.",
            );
        } else if !world.state.has("aether_mote") {
            world.show_message("A memory crystal, dark and cold. An Aether Mote could wake it.");
        } else {
            let motes = world.state.count("aether_mote");
            world.confirm(
                format!("Wake the crystal with an Aether Mote? ({motes} held)"),
                vec!["take:aether_mote".into(), format!("flag:{key}"), "era".into()],
            );
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let awake = self.awake(world);
        let bob = if awake { ((self.time * 3.0).sin() * 1.5).round() } else { 0.0 };
        let tint = if awake { Rgba::WHITE } else { Rgba::rgb(110, 100, 130) };
        batch.draw(art::shadow(10), snap(self.base.pos - Vec2::new(5.0, 2.0)), Rgba::WHITE);
        batch.draw(art_combat::memory_shard(), snap(self.base.pos - Vec2::new(6.0, 22.0 + bob)), tint);
    }
}

/// A sealed Concord door that answers only to the Aether.
pub struct AetherSeal {
    base: EntityBase,
    cell: (i32, i32),
    flag: String,
    shake: f32,
    time: f32,
}

impl AetherSeal {
    pub fn new(data: &EntityData, map_id: &str) -> AetherSeal {
        let cell = data.tile();
        AetherSeal {
            base: EntityBase {
                pos: Vec2::new(cell.0 as f32 * T + T / 2.0, ((cell.1 + 1) * TILE) as f32),
                width: T,
                height: T,
                ..EntityBase::default()
            },
            cell,
            flag: data
                .str("flag")
                .map(str::to_string)
                .unwrap_or_else(|| format!("seal:{map_id}:{}_{}", cell.0, cell.1)),
            shake: 0.0,
            time: 0.0,
        }
    }

    fn area(&self) -> Box2 {
        Box2::new((self.cell.0 * TILE) as f32, (self.cell.1 * TILE) as f32, T, T)
    }
}

impl Entity for AetherSeal {
    entity_base!();

    fn can_interact(&self, world: &World) -> bool {
        !world.state.aether_awakened()
    }

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Neutral)
    }

    fn hurtbox(&self) -> Box2 {
        self.area()
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        Some((36.0, Rgba::rgb(230, 170, 255)))
    }

    fn added(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag) {
            self.base.removed = true;
        } else {
            world.map.block(self.cell.0, self.cell.1, true);
        }
    }

    fn update(&mut self, _world: &mut World, dt: f32) {
        self.shake = (self.shake - dt).max(0.0);
        self.time += dt;
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if !(hit.aether && world.state.aether_awakened()) {
            audio::play(Sound::Clink, 0.6, 0.0);
            self.shake = 0.15;
            return true;
        }
        self.base.removed = true;
        world.state.flags.insert(self.flag.clone());
        world.map.block(self.cell.0, self.cell.1, false);
        world.camera.shake(0.15, 1.0);
        audio::sfx(Sound::Break);
        let color = Rgba::rgb(240, 200, 255);
        Particle::burst(world, self.area().center(), color, 22, 80.0, 0.6, true);
        world.add(Puff::new(self.area().center(), color));
        true
    }

    fn interact(&mut self, world: &mut World) {
        world
            .show_message("A Concord seal of woven aether. No single element can open it; only every element at once.");
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let jitter = if self.shake > 0.0 { Vec2::new(world.rng.range(-1, 2) as f32, 0.0) } else { Vec2::ZERO };
        let at = Vec2::new((self.cell.0 * TILE) as f32, (self.cell.1 * TILE) as f32) + jitter;
        let src = IRect::new((self.time * 6.0) as i32 % 2 * 16, 0, 16, 16);
        batch.draw_src(art_combat::aether_seal_art(), at, Some(src), Rgba::WHITE);
    }
}
