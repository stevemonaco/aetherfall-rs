//! The save crystal menu: the game is already saved and the player healed; from here they can fast travel.

use bevy::math::Vec2;

use super::circuit::CircuitScene;
use super::map_scene;
use super::scene::{Scene, SceneCtx};
use super::world::SCREEN_WIDTH;
use crate::core::audio::{self, Sound};
use crate::core::direction::Direction;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::scene_any;

const VISIBLE: usize = 8;

struct Row {
    label: String,
    map: String,
    tile: (i32, i32),
}

pub struct CrystalScene {
    rows: Vec<Row>,
    row: usize,
    scroll: usize,
}

impl CrystalScene {
    pub fn new() -> CrystalScene {
        CrystalScene { rows: Vec::new(), row: 0, scroll: 0 }
    }
}

impl Scene for CrystalScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, ctx: &mut SceneCtx) {
        input::consume();
        let fixed = |label: &str| Row { label: label.to_string(), map: String::new(), tile: (0, 0) };
        self.rows = vec![fixed("Stay here"), fixed("Open the Circuit")];
        if let Some(world) = ctx.world() {
            for flag in world.state.flags.iter().filter(|f| f.starts_with("crystal:")) {
                let parts: Vec<&str> = flag.split(':').collect();
                if parts.len() < 3 || parts[1] == world.map_id() {
                    continue;
                }
                let Some(name) = map_scene::map_name(parts[1]) else { continue };
                let mut xy = parts[2].split('_').map(|v| v.parse::<i32>().unwrap_or(0));
                let (x, y) = (xy.next().unwrap_or(0), xy.next().unwrap_or(0));
                self.rows.push(Row { label: name, map: parts[1].to_string(), tile: (x, y + 1) });
            }
        }
        audio::sfx(Sound::Heal);
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        let count = self.rows.len();
        if input::pressed(GameAction::Up) {
            self.row = (self.row + count - 1) % count;
        }
        if input::pressed(GameAction::Down) {
            self.row = (self.row + 1) % count;
        }
        if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
            audio::sfx(Sound::MenuMove);
        }
        self.scroll = self.scroll.clamp((self.row + 1).saturating_sub(VISIBLE), self.row);
        let confirm = input::pressed(GameAction::Confirm);
        if input::pressed(GameAction::Cancel) || (confirm && self.row == 0) {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        if confirm && self.row == 1 {
            audio::sfx(Sound::MenuSelect);
            ctx.push(CircuitScene::at_crystal());
            return;
        }
        if confirm {
            let row = &self.rows[self.row];
            audio::sfx(Sound::MenuSelect);
            ctx.pop();
            input::consume();
            if let Some(world) = ctx.world() {
                world.begin_warp(&row.map, row.tile, Direction::Down, true);
            }
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        let rect = IRect::new(SCREEN_WIDTH / 2 - 110, 50, 220, 30 + VISIBLE.min(self.rows.len()) as i32 * 13);
        batch.draw_window(rect);
        let heading = Vec2::new((rect.x + 10) as f32, (rect.y + 6) as f32);
        font::draw(batch, "Your journey has been recorded.", heading, palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
        for i in self.scroll..self.rows.len().min(self.scroll + VISIBLE) {
            let y = (rect.y + 22 + (i - self.scroll) as i32 * 13) as f32;
            let label = if i <= 1 { self.rows[i].label.clone() } else { format!("Travel: {}", self.rows[i].label) };
            let color = if i == self.row { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, &label, Vec2::new((rect.x + 22) as f32, y), color, Some(palette::TEXT_SHADOW));
            if i == self.row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new((rect.x + 10) as f32, y), palette::HIGHLIGHT, None);
            }
        }
    }
}
