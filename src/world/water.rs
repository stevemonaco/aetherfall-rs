//! Animated water drawn beneath the baked terrain wherever water can show through.

use bevy::math::Vec2;

use super::baker;
use super::map::{TILE, Terrain, TileMap};
use super::theme::TerrainTheme;
use crate::core::canvas::Canvas;
use crate::core::color::Rgba;
use crate::core::geom::IRect;
use crate::core::gfx::{self, Batch, TexId};
use crate::core::noise;

const FRAMES: u32 = 4;

pub struct WaterLayer {
    frames: TexId,
    cells: Vec<(i32, i32)>,
}

impl WaterLayer {
    pub fn new(map: &TileMap, theme: &TerrainTheme) -> WaterLayer {
        let mut cells = Vec::new();
        for y in 0..map.height {
            for x in 0..map.width {
                if (-1..=1).any(|oy| (-1..=1).any(|ox| map.at(x + ox, y + oy) == Terrain::Water)) {
                    cells.push((x, y));
                }
            }
        }
        let (w, h, pixels) = baker::water_frames(theme.ramp(Terrain::Water));
        let frames = gfx::cached(&format!("water_{}", theme.name), || Canvas { width: w, height: h, pixels });
        WaterLayer { frames, cells }
    }

    pub fn draw(&self, batch: &mut Batch, view: IRect, time: f64) {
        let tick = (time * 3.0) as u32;
        for &(x, y) in &self.cells {
            let dest = IRect::new(x * TILE, y * TILE, TILE, TILE);
            if !dest.intersects(&view) {
                continue;
            }
            // Per-cell phase and mirroring break up the tiled pattern.
            let hash = noise::hash(x, y, 17);
            let src = IRect::new(((tick + hash % FRAMES) % FRAMES) as i32 * TILE, 0, TILE, TILE);
            let pos = Vec2::new(dest.x as f32, dest.y as f32);
            batch.draw_ex(self.frames, pos, Some(src), Rgba::WHITE, 0.0, Vec2::ZERO, Vec2::ONE, hash & 16 != 0, false);
        }
    }
}
