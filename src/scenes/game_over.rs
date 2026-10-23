//! Offers a retry from where the player entered the area, a reload of the last save, or the title.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx};
use super::title::TitleScene;
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use super::world_scene::WorldScene;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::gameplay::game_state::GameState;
use crate::scene_any;

const OPTIONS: [&str; 3] = ["Retry", "Load last save", "Title"];

pub struct GameOverScene {
    checkpoint: GameState,
    row: usize,
    time: f32,
}

impl GameOverScene {
    pub fn new(checkpoint: GameState) -> GameOverScene {
        GameOverScene { checkpoint, row: 0, time: 0.0 }
    }
}

impl Scene for GameOverScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.time += dt;
        if self.time < 1.0 {
            return;
        }
        let count = OPTIONS.len();
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
        audio::sfx(Sound::MenuSelect);
        match self.row {
            0 | 1 => {
                let mut state = if self.row == 0 {
                    self.checkpoint.clone()
                } else {
                    GameState::load(self.checkpoint.slot).unwrap_or_else(|| self.checkpoint.clone())
                };
                state.hp = state.max_hp();
                ctx.replace(WorldScene::new(state, true));
            }
            _ => ctx.replace(TitleScene::new()),
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        let alpha = self.time.min(1.0);
        let cx = SCREEN_WIDTH as f32 / 2.0;
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::rgb(30, 0, 0).mul(alpha * 0.85));
        font::draw_centered(
            batch,
            "Your journey ends here...",
            Vec2::new(cx, 90.0),
            palette::TEXT.mul(alpha),
            Some(palette::TEXT_SHADOW.mul(alpha)),
        );
        if self.time < 1.0 {
            return;
        }
        for (i, option) in OPTIONS.iter().enumerate() {
            let y = (130 + i * 14) as f32;
            let color = if i == self.row { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw_centered(batch, option, Vec2::new(cx, y), color, Some(palette::TEXT_SHADOW));
            if i == self.row {
                let x = cx - (font::measure(option) / 2) as f32 - 10.0;
                font::draw(batch, &CURSOR.to_string(), Vec2::new(x, y), palette::HIGHLIGHT, None);
            }
        }
    }
}
