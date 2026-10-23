//! Tidal flats that flood and drain on a fixed rhythm.

use bevy::math::Vec2;

use super::entity::{Entity, EntityBase};
use crate::art::bosses;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::entity_base;
use crate::scenes::world::World;
use crate::world::map::{EntityData, TILE, Terrain};

/// A tidal flat: walkable shallows at low tide and deep water at high tide, on a fixed rhythm. Ripples warn before
/// the water rises; anyone caught in it is swept back to safe ground. Frozen tiles stay frozen through the tide.
pub struct Tide {
    base: EntityBase,
    area: IRect,
    period: f32,
    offset: f32,
    high: bool,
    time: f32,
}

impl Tide {
    pub fn new(data: &EntityData) -> Tide {
        let area = IRect::new(data.x as i32, data.y as i32, data.int("w", 4), data.int("h", 4));
        let pos = Vec2::new(((area.x + area.w / 2) * TILE) as f32, (area.bottom() * TILE) as f32);
        Tide {
            base: EntityBase { pos, width: 0.0, height: 0.0, ..EntityBase::default() },
            area,
            period: data.float("period", 6.0),
            offset: data.float("offset", 0.0),
            high: false,
            time: 0.0,
        }
    }

    fn phase(&self) -> f32 {
        (self.time + self.offset) % (self.period * 2.0)
    }

    fn should_be_high(&self) -> bool {
        self.phase() >= self.period
    }

    fn warning(&self) -> bool {
        !self.high && self.phase() >= self.period - 1.2
    }

    fn apply(&mut self, world: &mut World, high: bool) {
        self.high = high;
        for y in self.area.y..self.area.bottom() {
            for x in self.area.x..self.area.right() {
                if world.map.at(x, y) != Terrain::Ice {
                    world.map.set_terrain(x, y, if high { Terrain::Water } else { Terrain::Shallows });
                }
            }
        }
    }
}

impl Entity for Tide {
    entity_base!();

    fn layer(&self) -> i32 {
        0
    }

    fn added(&mut self, world: &mut World) {
        self.apply(world, self.should_be_high());
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.time += dt;
        if self.should_be_high() != self.high {
            self.apply(world, self.should_be_high());
            if self.high {
                audio::play(Sound::Burn, 0.5, -0.6);
            } else {
                audio::play(Sound::MenuMove, 0.5, 0.2);
            }
        }
    }

    fn draw(&self, world: &World, batch: &mut Batch) {
        let view = world.camera.view();
        let frame = (self.time * 3.0) as i32 % 4;
        let warning = self.warning();
        for y in self.area.y..self.area.bottom() {
            for x in self.area.x..self.area.right() {
                let at = Vec2::new((x * TILE) as f32, (y * TILE) as f32);
                if !view.intersects(&IRect::new(at.x as i32, at.y as i32, TILE, TILE)) {
                    continue;
                }
                if self.high && world.map.at(x, y) == Terrain::Water {
                    let src = IRect::new(((frame + x + y) % 4) * 16, 0, 16, 16);
                    batch.draw_src(bosses::tide_water(), at, Some(src), Rgba::WHITE);
                } else if warning && (self.time * 8.0 + (x + y) as f32) as i32 % 3 == 0 {
                    let src = IRect::new(frame * 16, 0, 16, 12);
                    batch.draw_src(bosses::ripple(), at + Vec2::new(0.0, 2.0), Some(src), Rgba::WHITE.mul(0.7));
                }
            }
        }
    }
}
