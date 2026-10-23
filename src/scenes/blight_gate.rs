//! The Blight Gate's list of open expeditions; choosing one drops the player at its entrance.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx, world_below};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::gameplay::blight_areas::{self, BlightAreaDef};
use crate::gameplay::game_state::GameState;
use crate::scene_any;

struct Entry {
    area: &'static BlightAreaDef,
    label: String,
    map: String,
    checkpoint: bool,
}

fn open(state: &GameState) -> Vec<Entry> {
    let mut entries = Vec::new();
    for area in blight_areas::ALL.iter().filter(|a| state.check(Some(a.unlock))) {
        entries.push(Entry { area, label: area.name.to_string(), map: area.map.to_string(), checkpoint: false });
        if area.id == "hollow" {
            for floor in blight_areas::hollow_checkpoints(state) {
                let label = format!("{}, floor {floor}", area.name);
                entries.push(Entry { area, label, map: format!("hollow_{floor}"), checkpoint: true });
            }
        }
    }
    entries
}

#[derive(Default)]
pub struct BlightGateScene {
    row: usize,
}

impl BlightGateScene {
    pub fn new() -> BlightGateScene {
        BlightGateScene::default()
    }
}

impl Scene for BlightGateScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        let Some(world) = ctx.world() else { return };
        let open = open(&world.state);
        let count = open.len();
        if input::pressed(GameAction::Cancel) || (count == 0 && input::pressed(GameAction::Confirm)) {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        if input::pressed(GameAction::Up) {
            self.row = (self.row + count - 1) % count;
        }
        if input::pressed(GameAction::Down) {
            self.row = (self.row + 1) % count;
        }
        if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
            audio::sfx(Sound::MenuMove);
        }
        if !input::pressed(GameAction::Confirm) {
            return;
        }
        let entry = &open[self.row];
        audio::sfx(Sound::MenuSelect);
        input::consume();
        world.start_run(entry.area, &entry.map, entry.area.x, entry.area.y);
        ctx.pop();
    }

    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        batch.begin_screen();
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::rgb(16, 4, 24).mul(0.85));
        let open = open(&world.state);
        let row_height = if open.len() > 7 { 14 } else { 24 };
        let top = if open.len() > 7 { 12 } else { 30 };
        let rect = IRect::new(SCREEN_WIDTH / 2 - 220, top, 440, 40 + open.len().max(1) as i32 * row_height);
        batch.draw_window(rect);
        let shadow = Some(palette::TEXT_SHADOW);
        let title = Vec2::new((rect.x + 10) as f32, (rect.y + 6) as f32);
        font::draw(batch, "The Blight Gate", title, Rgba::rgb(210, 150, 255), shadow);
        if open.is_empty() {
            let at = Vec2::new((rect.x + 20) as f32, (rect.y + 26) as f32);
            font::draw(batch, "The gate is quiet. Nothing has blighted yet.", at, palette::TEXT_DIM, None);
        }
        for (i, entry) in open.iter().enumerate() {
            let y = (rect.y + 26 + i as i32 * row_height) as f32;
            let selected = i == self.row;
            if selected {
                font::draw(batch, &CURSOR.to_string(), Vec2::new((rect.x + 8) as f32, y), palette::HIGHLIGHT, None);
            }
            let color = if selected { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, &entry.label, Vec2::new((rect.x + 20) as f32, y), color, shadow);
            let tag = if entry.checkpoint {
                "ungraded".to_string()
            } else {
                let best = blight_areas::best_grade(&world.state, entry.area.id);
                format!("Lv {}   Grade {}", entry.area.level, best.as_deref().unwrap_or("-"))
            };
            let tag_at = Vec2::new((rect.right() - 10 - font::measure(&tag)) as f32, y);
            font::draw(batch, &tag, tag_at, palette::TEXT_DIM, None);
            if row_height > 14 || selected {
                let at = if row_height > 14 {
                    Vec2::new((rect.x + 28) as f32, y + 10.0)
                } else {
                    Vec2::new((rect.x + 10) as f32, (rect.bottom() - 12) as f32)
                };
                font::draw(batch, entry.area.description, at, palette::TEXT_DIM, None);
            }
        }
    }
}
