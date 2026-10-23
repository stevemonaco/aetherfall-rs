//! Objects that respond to chemistry: one-tile obstacles (thorny brush, ice, unnatural fire, shock locks, cracked
//! rock) that block their tile until the right element clears them, plus turbines, braziers and crates.

use bevy::math::Vec2;

use super::effects::{Particle, Puff};
use super::entity::{Entity, EntityBase, PLAYER_ID};
use super::hazards::SteamCloud;
use super::pickup::Pickup;
use crate::art::{combat as art_combat, props as art};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, IRect, snap};
use crate::core::gfx::Batch;
use crate::entity_base;
use crate::gameplay::combat::{self, Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::map::{EntityData, TILE, tile_center};

const T: f32 = TILE as f32;
const NEIGHBOURS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObstacleKind {
    /// Dry thorny brush. Burns away, and the fire spreads to neighbouring brush and tall grass.
    Thicket,
    /// A wall of ice. Fire melts it; so does burning ground beside it.
    IceWall,
    /// An unnatural fire that roars across a passage. Water quenches it.
    FlameWall,
    /// A sealed metal door. Electricity springs the lock.
    ShockLock,
    /// Cracked rock. Shattered by a wind blast, or by something heavy hurled into it.
    Rock,
    /// A wall that is not quite a wall. With the hawk's eyes it shimmers and crumbles at a touch.
    Secret,
}

impl ObstacleKind {
    fn key(self) -> &'static str {
        match self {
            ObstacleKind::Thicket => "thicket",
            ObstacleKind::IceWall => "icewall",
            ObstacleKind::FlameWall => "flamewall",
            ObstacleKind::ShockLock => "shocklock",
            ObstacleKind::Rock => "rock",
            ObstacleKind::Secret => "secret",
        }
    }
}

/// A one-tile obstacle that blocks its tile until the right chemistry removes it, remembered by a flag.
pub struct TileObstacle {
    base: EntityBase,
    pub kind: ObstacleKind,
    cell: (i32, i32),
    flag: String,
    shake: f32,
    time: f32,
    burning: f32,
    melt: f32,
    charge: f32,
}

impl TileObstacle {
    pub fn new(data: &EntityData, map_id: &str, kind: ObstacleKind) -> TileObstacle {
        let cell = data.tile();
        TileObstacle {
            base: EntityBase {
                pos: Vec2::new(cell.0 as f32 * T + T / 2.0, ((cell.1 + 1) * TILE) as f32),
                width: T,
                height: T,
                ..EntityBase::default()
            },
            kind,
            cell,
            flag: data
                .str("flag")
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}:{map_id}:{}_{}", kind.key(), cell.0, cell.1)),
            shake: 0.0,
            time: 0.0,
            burning: -1.0,
            melt: 0.0,
            charge: 0.0,
        }
    }

    fn area(&self) -> Box2 {
        Box2::new((self.cell.0 * TILE) as f32, (self.cell.1 * TILE) as f32, T, T)
    }

    fn draw_at(&self, world: &World) -> Vec2 {
        let jitter = if self.shake > 0.0 { Vec2::new(world.rng.range(-1, 2) as f32, 0.0) } else { Vec2::ZERO };
        Vec2::new((self.cell.0 * TILE) as f32, (self.cell.1 * TILE) as f32) + jitter
    }

    fn resist(&mut self) -> bool {
        audio::play(Sound::Clink, 0.6, 0.0);
        self.shake = 0.15;
        true
    }

    fn clear(&mut self, world: &mut World, sound: Sound, color: Rgba) {
        if self.base.removed {
            return;
        }
        self.base.removed = true;
        world.state.flags.insert(self.flag.clone());
        world.map.block(self.cell.0, self.cell.1, false);
        world.camera.shake(0.15, 1.0);
        audio::sfx(sound);
        let center = self.area().center();
        Particle::burst(world, center, color, 22, 80.0, 0.6, true);
        world.add(Puff::new(center, color));
    }

    fn ignite_thicket(&mut self) {
        if self.burning < 0.0 {
            self.burning = 0.0;
            audio::play(Sound::Burn, 0.6, 0.0);
        }
    }
}

impl Entity for TileObstacle {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        self.kind != ObstacleKind::Secret
    }

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Neutral)
    }

    fn hurtbox(&self) -> Box2 {
        self.area()
    }

    fn flammable(&self) -> bool {
        self.kind == ObstacleKind::Thicket
    }

    fn ignite(&mut self, _world: &mut World) {
        if self.kind == ObstacleKind::Thicket {
            self.ignite_thicket();
        }
    }

    fn conductor(&self) -> bool {
        self.kind == ObstacleKind::ShockLock
    }

    fn energize(&mut self, world: &mut World) {
        if self.kind != ObstacleKind::ShockLock {
            return;
        }
        self.charge += 1.0;
        self.shake = 0.2;
        if self.charge >= 1.0 {
            self.clear(world, Sound::Break, Rgba::rgb(255, 240, 140));
        }
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        match self.kind {
            ObstacleKind::Thicket if self.burning >= 0.0 => Some((36.0, Rgba::rgb(255, 150, 60))),
            ObstacleKind::FlameWall => Some((40.0, Rgba::rgb(255, 140, 50))),
            _ => None,
        }
    }

    fn added(&mut self, world: &mut World) {
        if world.state.flags.contains(&self.flag) {
            self.base.removed = true;
        } else {
            world.map.block(self.cell.0, self.cell.1, true);
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.shake = (self.shake - dt).max(0.0);
        self.time += dt;
        let (cx, cy) = self.cell;
        match self.kind {
            ObstacleKind::Thicket => {
                if self.burning < 0.0 {
                    return;
                }
                self.burning += dt;
                if world.rng.next_f32() < 0.6 {
                    let at =
                        self.area().center() + Vec2::new(world.rng.range(-6, 7) as f32, world.rng.range(-4, 6) as f32);
                    let color =
                        if world.rng.next_f32() < 0.5 { Rgba::rgb(255, 200, 70) } else { Rgba::rgb(255, 100, 40) };
                    world.add(Particle::new(at, Vec2::new(0.0, -26.0), color, 0.5));
                }
                if self.burning > 0.35 {
                    for (dx, dy) in NEIGHBOURS {
                        if world.rng.next_f32() < 0.06 {
                            world.chem_ignite(cx + dx, cy + dy);
                        }
                    }
                }
                if self.burning > 1.4 {
                    for (dx, dy) in NEIGHBOURS {
                        world.chem_ignite(cx + dx, cy + dy);
                    }
                    self.clear(world, Sound::Burn, Rgba::rgb(255, 140, 40));
                }
            }
            ObstacleKind::IceWall => {
                for (dx, dy) in NEIGHBOURS {
                    if world.is_burning(cx + dx, cy + dy) {
                        self.melt += dt * 0.6;
                    }
                }
                if self.melt >= 1.0 {
                    self.clear(world, Sound::Burn, Rgba::rgb(200, 240, 255));
                }
            }
            ObstacleKind::FlameWall => {
                if world.rng.next_f32() < 0.3 {
                    let at = self.draw_at(world) + Vec2::new(world.rng.range(2, 14) as f32, 2.0);
                    world.add(Particle::new(at, Vec2::new(0.0, -20.0), Rgba::rgb(255, 200, 80), 0.4));
                }
                if self.area().inflate(2.0).intersects(&world.hero.hurtbox) {
                    let mut hit = Hit {
                        power: 0.5,
                        element: Element::Fire,
                        kind: HitKind::Environment,
                        from: self.area().center(),
                        status_power: 1.5,
                        knockback: 160.0,
                        ..Hit::new(Faction::Neutral, combat::environment(world.chem.zone_level))
                    };
                    world.hit(PLAYER_ID, &mut hit);
                }
            }
            _ => {}
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        match self.kind {
            ObstacleKind::Thicket => {
                if matches!(hit.element, Element::Fire | Element::Aether) {
                    self.ignite_thicket();
                } else {
                    self.resist();
                }
                true
            }
            ObstacleKind::IceWall => {
                if matches!(hit.element, Element::Fire | Element::Aether) {
                    self.melt += if matches!(hit.kind, HitKind::ChargedBolt | HitKind::Skill | HitKind::Finisher) {
                        1.0
                    } else {
                        0.34
                    };
                    self.shake = 0.1;
                    world.add(SteamCloud::new(self.area().center() + Vec2::new(0.0, 8.0), false));
                    if self.melt >= 1.0 {
                        self.clear(world, Sound::Burn, Rgba::rgb(200, 240, 255));
                    }
                    return true;
                }
                self.resist()
            }
            ObstacleKind::FlameWall => {
                if matches!(hit.element, Element::Water | Element::Aether) {
                    for _ in 0..3 {
                        let at = self.area().center() + Vec2::new(world.rng.range(-6, 7) as f32, 8.0);
                        world.add(SteamCloud::new(at, false));
                    }
                    self.clear(world, Sound::Burn, Rgba::rgb(160, 210, 255));
                    return true;
                }
                if hit.element == Element::Wind {
                    self.shake = 0.3;
                    audio::play(Sound::ShootFire, 0.5, 0.0);
                    return true;
                }
                self.resist()
            }
            ObstacleKind::ShockLock => {
                if matches!(hit.element, Element::Thunder | Element::Aether) {
                    self.charge +=
                        if matches!(hit.kind, HitKind::Melee | HitKind::Bolt | HitKind::Tether) { 0.34 } else { 1.0 };
                    self.shake = 0.15;
                    Particle::burst(world, self.area().center(), Element::Thunder.glow(), 6, 40.0, 0.3, true);
                    if self.charge >= 1.0 {
                        self.clear(world, Sound::Break, Rgba::rgb(255, 240, 140));
                    }
                    return true;
                }
                self.resist()
            }
            ObstacleKind::Rock => {
                let strong =
                    matches!(hit.kind, HitKind::ChargedBolt | HitKind::Skill | HitKind::Finisher | HitKind::Environment);
                let windy = matches!(hit.element, Element::Wind | Element::Aether);
                if (windy && strong) || (hit.kind == HitKind::Environment && hit.power >= 1.0) {
                    self.clear(world, Sound::Break, Rgba::rgb(170, 130, 90));
                    return true;
                }
                self.resist()
            }
            ObstacleKind::Secret => {
                if world.state.soul.as_deref() != Some("hawk") || world.companion.is_none() {
                    return self.resist();
                }
                self.clear(world, Sound::Break, Rgba::rgb(200, 190, 170));
                world.toast("A hidden passage!");
                true
            }
        }
    }

    fn interact(&mut self, world: &mut World) {
        let text = match self.kind {
            ObstacleKind::Thicket => "Dry, thorny brush. It looks like it would go up in flames.",
            ObstacleKind::IceWall => "A wall of thick blue ice. Heat would melt it.",
            ObstacleKind::FlameWall => "Unnatural fire roars across the way. Water might quench it.",
            ObstacleKind::ShockLock => {
                "A sealed metal door with a lightning sigil. Its lock wants a jolt of electricity."
            }
            ObstacleKind::Rock => {
                "The rock is riddled with cracks. A strong gust, or something heavy, might shatter it."
            }
            ObstacleKind::Secret => return,
        };
        world.show_message(text);
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let at = self.draw_at(world);
        match self.kind {
            ObstacleKind::Thicket => {
                batch.draw(art::thorns(), at, Rgba::WHITE.lerp(Rgba::rgb(255, 120, 60), self.burning.clamp(0.0, 1.0)));
                if self.burning >= 0.0 {
                    let frame = (self.time * 10.0) as i32 % 4;
                    batch.draw_src(
                        art_combat::flames(),
                        at + Vec2::new(0.0, -6.0),
                        Some(IRect::new(frame * 16, 0, 16, 22)),
                        Rgba::WHITE,
                    );
                }
            }
            ObstacleKind::IceWall => {
                batch.draw(art::ice_wall(), at, Rgba::WHITE.lerp(Rgba::rgb(160, 200, 255), self.melt))
            }
            ObstacleKind::FlameWall => {
                let frame = (self.time * 8.0) as i32 % 2;
                batch.draw_src(art::flame_wall(), at, Some(IRect::new(frame * 16, 0, 16, 16)), Rgba::WHITE);
            }
            ObstacleKind::ShockLock => {
                batch.draw(art::iron_wall(), at, Rgba::WHITE.lerp(Rgba::rgb(255, 250, 180), self.charge.min(1.0)))
            }
            ObstacleKind::Rock => batch.draw(art::cracked_rock(), at, Rgba::WHITE),
            ObstacleKind::Secret => {
                let rock = art_combat::secret_rock(world.theme_name());
                batch.draw(rock, at, Rgba::WHITE);
                if world.state.soul.as_deref() == Some("hawk") && world.companion.is_some() {
                    batch.draw(rock, at, Rgba::WHITE.mul(0.25 + 0.2 * (self.time * 6.0).sin()));
                }
            }
        }
    }
}

/// A wind turbine. Wind sets it spinning, which powers whatever its flag controls (for a while, if timed).
pub struct Turbine {
    base: EntityBase,
    flag: String,
    duration: f32,
    spin: f32,
    angle: f32,
}

impl Turbine {
    pub fn new(data: &EntityData) -> Turbine {
        Turbine {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0),
                solid: true,
                width: 12.0,
                height: 8.0,
                ..EntityBase::default()
            },
            flag: data.str_or("flag", "turbine").to_string(),
            duration: data.float("timer", 0.0),
            spin: 0.0,
            angle: 0.0,
        }
    }
}

impl Entity for Turbine {
    entity_base!();

    fn can_interact(&self, _world: &World) -> bool {
        true
    }

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Neutral)
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 7.0, self.base.pos.y - 22.0, 14.0, 22.0)
    }

    fn added(&mut self, world: &mut World) {
        if self.duration <= 0.0 && world.state.flags.contains(&self.flag) {
            self.spin = f32::MAX;
        }
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if !matches!(hit.element, Element::Wind | Element::Aether) {
            audio::play(Sound::Clink, 0.5, 0.0);
            return true;
        }
        if self.spin <= 0.0 {
            audio::play(Sound::Door, 0.8, 0.3);
        }
        self.spin = if self.duration > 0.0 { self.duration } else { f32::MAX };
        world.state.flags.insert(self.flag.clone());
        true
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        if self.spin <= 0.0 {
            return;
        }
        self.angle += dt * 14.0;
        if self.spin < f32::MAX {
            self.spin -= dt;
            if self.spin <= 0.0 {
                world.state.flags.remove(&self.flag);
                audio::play(Sound::MenuCancel, 0.6, 0.0);
            }
        }
    }

    fn interact(&mut self, world: &mut World) {
        world.show_message(if self.duration > 0.0 {
            "A wind turbine wired to something nearby. A strong gust would spin it for a short while."
        } else {
            "A wind turbine wired to something nearby. A strong gust would set it spinning."
        });
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let pos = self.base.pos;
        batch.draw(art::shadow(12), snap(pos - Vec2::new(6.0, 2.0)), Rgba::WHITE);
        batch.draw(art_combat::turbine_base(), snap(pos - Vec2::new(8.0, 16.0)), Rgba::WHITE);
        let frame = if self.spin > 0.0 { self.angle as i32 % 3 } else { 0 };
        let src = Some(IRect::new(frame * 16, 0, 16, 16));
        batch.draw_src(art_combat::turbine_blades(), snap(pos - Vec2::new(8.0, 26.0)), src, Rgba::WHITE);
        if self.spin > 0.0 && self.spin < 3.0 && (self.spin * 6.0) as i32 % 2 == 0 {
            batch.draw_src(art_combat::turbine_blades(), snap(pos - Vec2::new(8.0, 26.0)), src, Rgba::RED.mul(0.4));
        }
    }
}

/// A brazier. Fire lights it; water or wind puts it out. Lit braziers light the dark and can open the way.
pub struct Brazier {
    base: EntityBase,
    flag: Option<String>,
    starts_lit: bool,
    lit: bool,
    time: f32,
}

impl Brazier {
    pub fn new(data: &EntityData) -> Brazier {
        Brazier {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0),
                solid: true,
                width: 12.0,
                height: 8.0,
                ..EntityBase::default()
            },
            flag: data.str("flag").map(str::to_string),
            starts_lit: data.bool("lit", false),
            lit: false,
            time: 0.0,
        }
    }

    fn light_up(&mut self, world: &mut World) {
        if self.lit {
            return;
        }
        self.lit = true;
        audio::play(Sound::ShootFire, 0.7, 0.0);
        if let Some(f) = &self.flag {
            world.state.flags.insert(f.clone());
        }
    }
}

impl Entity for Brazier {
    entity_base!();

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Neutral)
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 7.0, self.base.pos.y - 16.0, 14.0, 16.0)
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        self.lit.then_some((70.0, Rgba::rgb(255, 170, 80)))
    }

    fn flammable(&self) -> bool {
        true
    }

    fn ignite(&mut self, world: &mut World) {
        self.light_up(world);
    }

    fn added(&mut self, world: &mut World) {
        self.lit = self.starts_lit || self.flag.as_ref().is_some_and(|f| world.state.flags.contains(f));
        let (tx, ty) = self.base.tile_pos();
        world.map.block(tx, ty, true);
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if matches!(hit.element, Element::Fire | Element::Aether) {
            self.light_up(world);
        } else if matches!(hit.element, Element::Water | Element::Wind) && self.lit && !self.starts_lit {
            self.lit = false;
            world.add(SteamCloud::new(self.base.pos - Vec2::new(0.0, 8.0), false));
            if let Some(f) = &self.flag {
                world.state.flags.remove(f);
            }
        } else if hit.element == Element::None {
            audio::play(Sound::Clink, 0.5, 0.0);
        }
        true
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if self.lit && world.rng.next_f32() < 0.5 {
            let at = self.base.pos + Vec2::new(world.rng.range(-3, 4) as f32, -16.0);
            let color = if world.rng.next_f32() < 0.5 { Rgba::rgb(255, 200, 60) } else { Rgba::rgb(255, 110, 40) };
            world.add(Particle::new(at, Vec2::new(0.0, -22.0), color, 0.45));
        }
        if self.lit {
            let (tx, ty) = self.base.tile_pos();
            world.chem_ignite(tx, ty - 1);
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let pos = self.base.pos;
        batch.draw(art::shadow(12), snap(pos - Vec2::new(6.0, 2.0)), Rgba::WHITE);
        batch.draw(art_combat::brazier(), snap(pos - Vec2::new(8.0, 15.0)), Rgba::WHITE);
        if self.lit {
            let frame = (self.time * 10.0) as i32 % 4;
            batch.draw_src(
                art_combat::flames(),
                snap(pos - Vec2::new(8.0, 32.0)),
                Some(IRect::new(frame * 16, 0, 16, 22)),
                Rgba::WHITE,
            );
        }
    }
}

/// A wooden crate: breaks under blows, burns, and sometimes hides something.
pub struct Crate {
    base: EntityBase,
    burning: f32,
    shake: f32,
    hp: i32,
}

impl Crate {
    pub fn new(data: &EntityData) -> Crate {
        Crate {
            base: EntityBase {
                pos: tile_center(data.x as i32, data.y as i32) + Vec2::new(0.0, 7.0),
                solid: true,
                width: 14.0,
                height: 10.0,
                ..EntityBase::default()
            },
            burning: -1.0,
            shake: 0.0,
            hp: 2,
        }
    }

    fn smash(&mut self, world: &mut World) {
        if self.base.removed {
            return;
        }
        self.base.removed = true;
        audio::play(Sound::Break, 0.7, 0.3);
        Particle::burst(world, self.hurtbox().center(), Rgba::rgb(170, 120, 70), 14, 70.0, 0.5, false);
        let roll = world.rng.next_f32();
        if roll < 0.25 {
            let amount = 3 + world.chem.zone_level * 2;
            world.add(Pickup::gold(self.base.pos, amount));
        } else if roll < 0.4 {
            let amount = world.state.max_hp() / 10;
            world.add(Pickup::heart(self.base.pos, amount));
        }
    }
}

impl Entity for Crate {
    entity_base!();

    fn faction(&self) -> Option<Faction> {
        Some(Faction::Neutral)
    }

    fn hurtbox(&self) -> Box2 {
        Box2::new(self.base.pos.x - 7.0, self.base.pos.y - 14.0, 14.0, 14.0)
    }

    fn flammable(&self) -> bool {
        true
    }

    fn ignite(&mut self, _world: &mut World) {
        if self.burning < 0.0 {
            self.burning = 0.0;
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        if hit.element == Element::Fire && self.burning < 0.0 {
            self.burning = 0.0;
        }
        self.shake = 0.15;
        audio::play(Sound::Hit, 0.6, 0.3);
        self.hp -= 1;
        if self.hp <= 0 || matches!(hit.kind, HitKind::Finisher | HitKind::ChargedBolt | HitKind::Skill) {
            self.smash(world);
        }
        true
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.shake = (self.shake - dt).max(0.0);
        if self.burning < 0.0 {
            return;
        }
        self.burning += dt;
        if world.rng.next_f32() < 0.5 {
            let at = self.hurtbox().center() + Vec2::new(world.rng.range(-5, 6) as f32, 0.0);
            world.add(Particle::new(at, Vec2::new(0.0, -24.0), Rgba::rgb(255, 160, 60), 0.4));
        }
        if self.burning > 1.2 {
            let (tx, ty) = self.base.tile_pos();
            world.chem_ignite(tx, ty);
            self.smash(world);
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let offset = if self.shake > 0.0 { Vec2::new(world.rng.range(-1, 2) as f32, 0.0) } else { Vec2::ZERO };
        let tint = if self.burning >= 0.0 { Rgba::rgb(255, 180, 140) } else { Rgba::WHITE };
        batch.draw(art_combat::crate_box(), snap(self.base.pos - Vec2::new(8.0, 15.0) + offset), tint);
    }
}
