//! Dungeon entrances drawn as landmarks: the Grotto's mouth, the Sunken Temple, the Storm Spire, the Fallen Bastion's
//! gatehouse and the Sky Altar. Each is anchored at its doorway tile and blocks the rest of its footprint; the warp
//! itself stays a separate entity on the doorway.

use bevy::math::Vec2;

use super::effects::Particle;
use super::entity::{Entity, EntityBase};
use crate::art::entrances as art;
use crate::core::color::Rgba;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::Batch;
use crate::core::noise;
use crate::entity_base;
use crate::scenes::world::World;
use crate::world::map::{EntityData, TILE};

const T: f32 = TILE as f32;
const EMBER: Rgba = Rgba::rgb(255, 140, 60);
const TIDE: Rgba = Rgba::rgb(120, 230, 230);
const STORM: Rgba = Rgba::rgb(150, 190, 255);
const SKY: Rgba = Rgba::rgb(220, 236, 255);
/// Each great crystal, its colour and the top-left of its socket relative to the altar's anchor.
const SOCKETS: [(&str, Rgba, Vec2); 3] = [
    ("ember_crystal", Rgba::rgb(255, 138, 48), Vec2::new(-43.0, -59.0)),
    ("tide_crystal", Rgba::rgb(90, 170, 255), Vec2::new(-3.0, -79.0)),
    ("storm_crystal", Rgba::rgb(248, 224, 96), Vec2::new(36.0, -59.0)),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Grotto,
    Temple,
    Spire,
    Bastion,
    Altar,
}

pub struct Entrance {
    base: EntityBase,
    look: Look,
    key: String,
    tile: (i32, i32),
    dialogue: Option<String>,
    speaker: String,
    time: f32,
}

impl Entrance {
    pub fn new(data: &EntityData) -> Entrance {
        let key = data.str_or("look", "grotto").to_string();
        let look = match key.as_str() {
            "temple" => Look::Temple,
            "spire" => Look::Spire,
            "bastion" => Look::Bastion,
            "altar" => Look::Altar,
            _ => Look::Grotto,
        };
        let tile = (data.x as i32, data.y as i32);
        let dialogue = data.str("dialogue").map(str::to_string);
        let (width, height) = if dialogue.is_some() { (48.0, 24.0) } else { (0.0, 0.0) };
        Entrance {
            base: EntityBase {
                pos: Vec2::new(tile.0 as f32 * T + T / 2.0, (tile.1 + 1) as f32 * T),
                width,
                height,
                ..EntityBase::default()
            },
            look,
            key,
            tile,
            dialogue,
            speaker: data.str_or("name", "").to_string(),
            time: 0.0,
        }
    }

    /// The tiles the landmark stands on, relative to its doorway, and the doorway tiles left open.
    fn footprint(&self) -> (IRect, &'static [(i32, i32)]) {
        match self.look {
            Look::Grotto => (IRect::new(-1, 0, 3, 1), &[(0, 0)]),
            Look::Temple => (IRect::new(-3, -1, 7, 2), &[(0, 0)]),
            Look::Spire => (IRect::new(-1, -1, 3, 2), &[(0, 0)]),
            Look::Bastion => (IRect::new(-3, -3, 8, 4), &[(0, 0), (1, 0)]),
            Look::Altar => (IRect::new(-3, -1, 7, 2), &[]),
        }
    }

    fn origin(&self) -> Vec2 {
        let (ax, ay) = art::anchor(&self.key);
        snap(self.base.pos - Vec2::new(ax, ay))
    }

    fn rand_offset(world: &mut World, half_w: i32, y0: f32, span: i32) -> Vec2 {
        Vec2::new(world.rng.range(-half_w, half_w + 1) as f32, y0 - world.rng.below(span.max(1)) as f32)
    }

    fn draw_smoke(&self, batch: &mut Batch) {
        let mouth = self.base.pos - Vec2::new(5.0, 37.0);
        for i in 0..6 {
            let phase = (self.time * 0.22 + i as f32 / 6.0).fract();
            let sway = (phase * 4.0 + i as f32 * 1.9).sin() * (3.0 + phase * 8.0);
            let at = mouth + Vec2::new(sway, -phase * 34.0);
            let alpha = (1.0 - phase) * (phase * 6.0).min(1.0) * 0.22;
            let scale = 0.5 + phase * 1.1;
            batch.draw_ex(
                art::smoke_wisp(),
                snap(at),
                None,
                Rgba::WHITE.mul(alpha),
                0.0,
                Vec2::ZERO,
                Vec2::splat(scale),
                false,
                false,
            );
        }
    }

    /// A brief jagged arc between two points, re-rolled a few times a second.
    fn draw_arc(batch: &mut Batch, from: Vec2, to: Vec2, seed: i32, color: Rgba) {
        let steps = ((to - from).length() / 3.0).max(2.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let jitter = if i == 0 || i == steps { 0.0 } else { (noise::hash01(i, seed, 71) - 0.5) * 5.0 };
            let normal = (to - from).perp().normalize_or_zero();
            let at = snap(from.lerp(to, t) + normal * jitter);
            batch.fill_rect(at.x, at.y, 1.0, 1.0, color);
            batch.fill_rect(at.x, at.y + 1.0, 1.0, 1.0, color.mul(0.5));
        }
    }

    fn draw_spire(&self, batch: &mut Batch) {
        let core = self.base.pos + Vec2::new(0.0, -74.5);
        let pulse = 0.25 + 0.15 * (self.time * 3.1).sin();
        batch.draw_ex(
            art::smoke_wisp(),
            snap(core - Vec2::new(10.0, 8.0)),
            None,
            Rgba::rgb(170, 210, 255).mul(pulse),
            0.0,
            Vec2::ZERO,
            Vec2::splat(2.0),
            false,
            false,
        );
        let tick = (self.time * 9.0) as i32;
        let strike = (self.time / 2.3) as i32;
        let into_strike = self.time - strike as f32 * 2.3;
        if into_strike < 0.22 {
            let rod = self.base.pos + Vec2::new(0.0, -96.0);
            for (k, end) in
                [Vec2::new(-18.0, -45.0), Vec2::new(18.0, -45.0), Vec2::new(-14.0, -67.0)].iter().enumerate()
            {
                if !noise::hash(strike, k as i32, 72).is_multiple_of(3) {
                    Self::draw_arc(batch, rod, self.base.pos + *end, tick + k as i32 * 13, Rgba::rgb(220, 236, 255));
                }
            }
        }
        if tick % 5 < 2 {
            let a = noise::hash01(tick, 0, 73) * std::f32::consts::TAU;
            let to = core + Vec2::new(a.cos(), a.sin()) * 7.0;
            Self::draw_arc(batch, core, to, tick, Rgba::rgb(200, 225, 255));
        }
    }

    fn draw_altar(&self, world: &World, batch: &mut Batch) {
        let held: Vec<bool> = SOCKETS.iter().map(|(id, _, _)| world.state.count(id) > 0).collect();
        let all = held.iter().all(|&h| h);
        let beam = if all { 0.75 + 0.15 * (self.time * 2.0).sin() } else { 0.22 + 0.06 * (self.time * 1.3).sin() };
        let top = self.base.pos + Vec2::new(-7.0, -110.0);
        batch.draw_ex(
            art::light_column(),
            snap(top),
            None,
            Rgba::WHITE.mul(beam),
            0.0,
            Vec2::ZERO,
            Vec2::new(1.0, 1.0),
            false,
            false,
        );
        for (i, (_, color, at)) in SOCKETS.iter().enumerate() {
            let bob = if held[i] { (self.time * 2.0 + i as f32 * 2.1).sin().round() } else { 0.0 };
            let pos = snap(self.base.pos + *at + Vec2::new(0.0, bob));
            let tint = if held[i] { *color } else { color.lerp(Rgba::rgb(200, 206, 228), 0.6).mul(0.55) };
            batch.draw(art::altar_crystal(), pos, tint);
        }
    }
}

impl Entity for Entrance {
    entity_base!();

    fn layer(&self) -> i32 {
        match self.look {
            Look::Spire | Look::Altar => 1,
            _ => 0,
        }
    }

    fn light(&self, world: &World) -> Option<(f32, Rgba)> {
        match self.look {
            Look::Grotto => Some((52.0, EMBER)),
            Look::Temple => Some((44.0, TIDE)),
            Look::Spire => Some((64.0, STORM)),
            Look::Altar => Some((if world.state.check(Some("crystals:3")) { 80.0 } else { 52.0 }, SKY)),
            Look::Bastion => None,
        }
    }

    fn can_interact(&self, _world: &World) -> bool {
        self.dialogue.is_some()
    }

    fn interact(&mut self, world: &mut World) {
        if let Some(d) = self.dialogue.clone() {
            world.start_dialogue(&d, &self.speaker);
        }
    }

    fn added(&mut self, world: &mut World) {
        let (area, doors) = self.footprint();
        for dy in area.y..area.bottom() {
            for dx in area.x..area.right() {
                if !doors.contains(&(dx, dy)) {
                    world.map.block(self.tile.0 + dx, self.tile.1 + dy, true);
                }
            }
        }
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        let pos = self.base.pos;
        let roll = world.rng.next_f32();
        match self.look {
            Look::Grotto if roll < 0.12 => {
                let at = pos + Self::rand_offset(world, 7, -3.0, 6);
                let color = if world.rng.next_f32() < 0.5 { Rgba::rgb(255, 190, 70) } else { Rgba::rgb(255, 110, 40) };
                world.add(Particle::new(at, Vec2::new(world.rng.range(-6, 7) as f32, -22.0), color, 0.9));
            }
            Look::Temple if roll < 0.08 => {
                let at = pos + Self::rand_offset(world, 8, -14.0, 20);
                world.add(Particle::new(at, Vec2::new(0.0, -10.0), TIDE, 1.1));
            }
            Look::Spire if roll < 0.18 => {
                let at = pos + Vec2::new(0.0, -95.0) + Self::rand_offset(world, 5, 0.0, 6);
                let v = Vec2::new(world.rng.range(-20, 21) as f32, world.rng.range(-20, 8) as f32);
                world.add(Particle::new(at, v, Rgba::rgb(200, 225, 255), 0.35).spark());
            }
            Look::Bastion if roll < 0.1 => {
                let at = pos + Vec2::new(8.0, 0.0) + Self::rand_offset(world, 60, -20.0, 40);
                let v = Vec2::new(world.rng.range(4, 12) as f32, world.rng.range(3, 9) as f32);
                world.add(Particle::new(at, v, Rgba::rgb(150, 140, 140), 1.6));
            }
            Look::Altar if roll < 0.15 => {
                let at = pos + Self::rand_offset(world, 5, -38.0, 10);
                world.add(Particle::new(at, Vec2::new(0.0, -26.0), SKY, 1.2).spark());
            }
            _ => {}
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        batch.draw(art::texture(&self.key), self.origin(), Rgba::WHITE);
        match self.look {
            Look::Grotto => self.draw_smoke(batch),
            Look::Spire => self.draw_spire(batch),
            Look::Altar => self.draw_altar(world, batch),
            _ => {}
        }
    }
}
