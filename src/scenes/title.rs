use bevy::math::Vec2;

use super::options::OptionsScene;
use super::scene::{Scene, SceneCtx};
use super::slot::SlotScene;
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use super::world_scene::WorldScene;
use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::canvas::Canvas;
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::{self, Batch};
use crate::core::input::{self, GameAction};
use crate::core::{noise, palette};
use crate::gameplay::game_state::{GameState, SLOT_COUNT};
use crate::scene_any;

const TITLE: &str = "AETHERFALL";

pub struct TitleScene {
    has_save: bool,
    options: Vec<&'static str>,
    row: usize,
    time: f32,
}

impl TitleScene {
    pub fn new() -> TitleScene {
        TitleScene { has_save: GameState::any_save(), options: Vec::new(), row: 0, time: 0.0 }
    }
}

impl Scene for TitleScene {
    scene_any!();

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        self.options = if self.has_save {
            vec!["Continue", "Load Game", "New Game", "Options", "Quit"]
        } else {
            vec!["New Game", "Options", "Quit"]
        };
        audio::play_music(Some("Menu-EpicEntrance"));
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.time += dt;
        let count = self.options.len();
        let before = self.row;
        if input::pressed(GameAction::Up) {
            self.row = (self.row + count - 1) % count;
        }
        if input::pressed(GameAction::Down) {
            self.row = (self.row + 1) % count;
        }
        if before != self.row {
            audio::sfx(Sound::MenuMove);
        }

        if !input::pressed(GameAction::Confirm) {
            return;
        }
        audio::sfx(Sound::MenuSelect);
        match self.options[self.row] {
            "Continue" => {
                if let Some(save) = latest_slot().and_then(GameState::load) {
                    ctx.replace(WorldScene::new(save, true));
                }
            }
            "Load Game" => ctx.push(SlotScene::new(false)),
            "New Game" => ctx.push(SlotScene::new(true)),
            "Options" => ctx.push(OptionsScene::new()),
            "Quit" => ctx.pop(),
            _ => {}
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        batch.draw(gfx::cached("title_sky", build_sky), Vec2::ZERO, Rgba::WHITE);
        let cx = SCREEN_WIDTH as f32 / 2.0;

        for i in 0..60 {
            let twinkle = ((self.time * (1 + i % 5) as f32 + i as f32).sin() + 1.0) / 2.0;
            let (x, y) = (noise::hash(i, 0, 3) % 480, noise::hash(i, 1, 3) % 150);
            batch.fill_rect(x as f32, y as f32, 1.0, 1.0, Rgba::WHITE.mul(0.3 + twinkle * 0.7));
        }

        // Falling embers from the shattered sky-cities.
        for i in 0..14 {
            let speed = 8.0 + noise::hash01(i, 5, 3) * 10.0;
            let x = noise::hash01(i, 6, 3) * 500.0 - 10.0 + (self.time + i as f32).sin() * 6.0;
            let y = (noise::hash01(i, 7, 3) * 290.0 + self.time * speed) % 290.0 - 10.0;
            batch.fill_rect(x, y, 1.0, 2.0, Rgba::rgb(255, 170, 90).mul(0.8));
        }

        let bob = ((self.time * 1.6).sin() * 2.0).round();
        batch.draw(props::save_crystal(), Vec2::new(cx - 6.0, 44.0 + bob), Rgba::WHITE);

        const SCALE: i32 = 3;
        let width = font::measure(TITLE) * SCALE;
        let pos = Vec2::new(((SCREEN_WIDTH - width) / 2) as f32, 76.0);
        for (ox, oy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (2, 2), (1, 2), (2, 1)] {
            font::draw_scaled(batch, TITLE, pos + Vec2::new(ox as f32, oy as f32), palette::OUTLINE, SCALE);
        }
        font::draw_scaled(batch, TITLE, pos, Rgba::rgb(255, 196, 92), SCALE);
        font::draw_scaled(batch, TITLE, pos + Vec2::new(0.0, -1.0), Rgba::rgb(255, 236, 170).mul(0.35), SCALE);
        font::draw_centered(
            batch,
            "An action RPG of crystals, storms and sky-cities",
            Vec2::new(cx, 110.0),
            palette::TEXT_DIM,
            Some(palette::TEXT_SHADOW),
        );

        let rect = IRect::new(cx as i32 - 50, 136, 100, 14 + self.options.len() as i32 * 12);
        batch.draw_window(rect);
        for (i, option) in self.options.iter().enumerate() {
            let y = (rect.y + 7 + i as i32 * 12) as f32;
            let selected = i == self.row;
            let color = if selected { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, option, Vec2::new((rect.x + 22) as f32, y), color, Some(palette::TEXT_SHADOW));
            if selected {
                font::draw(batch, &CURSOR.to_string(), Vec2::new((rect.x + 10) as f32, y), palette::HIGHLIGHT, None);
            }
        }
        font::draw(
            batch,
            "J/Enter: confirm   K/Esc: back   WASD/Arrows: move",
            Vec2::new(8.0, (SCREEN_HEIGHT - 12) as f32),
            palette::TEXT_DIM.mul(0.8),
            None,
        );
    }
}

/// The most recently written save slot, preferring the lowest slot on a tie.
fn latest_slot() -> Option<i32> {
    (1..=SLOT_COUNT).rev().filter(|&s| GameState::slot_exists(s)).max_by_key(|&s| saved_at(s))
}

// Mirrors the slot path in core::storage, which does not expose modification times.
#[cfg(not(target_arch = "wasm32"))]
fn saved_at(slot: i32) -> Option<std::time::SystemTime> {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let path = base.join("Aetherfall").join("rs").join(format!("slot{slot}.json"));
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(target_arch = "wasm32")]
fn saved_at(_slot: i32) -> Option<std::time::SystemTime> {
    None
}

fn build_sky() -> Canvas {
    let mut canvas = Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    let (top, mid, low) = (Rgba::rgb(10, 8, 30), Rgba::rgb(62, 30, 80), Rgba::rgb(200, 90, 70));
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let t = y as f32 / canvas.height as f32;
            let c = if t < 0.6 { top.lerp(mid, t / 0.6) } else { mid.lerp(low, (t - 0.6) / 0.4) };
            // Ordered dithering between bands keeps the gradient looking like pixel art.
            let dither = ((x + y * 2) % 4) as f32 / 16.0 - 0.1;
            canvas.set(x, y, c.lerp(if t < 0.6 { mid } else { low }, dither.clamp(0.0, 0.06)));
        }
    }

    // Ruined floating islands silhouetted against the dawn.
    let island = Rgba::rgb(30, 18, 42);
    for (cx, cy, w) in [(75, 180, 54), (390, 165, 69), (262, 207, 33)] {
        for x in -w..=w {
            let depth = ((1.0 - (x as f32 / w as f32).powi(2)) * w as f32 * 0.6) as i32;
            for y in 0..depth {
                canvas.set(cx + x, cy + y, island);
            }
            canvas.set(cx + x, cy - 1, Rgba::rgb(90, 60, 90));
        }
        for t in 0..4 {
            let tx = cx - w / 2 + (noise::hash(cx, t, 1) % w as u32) as i32;
            let th = 6 + (noise::hash(cy, t, 2) % 12) as i32;
            canvas.rect(tx, cy - th, 4, th, island);
        }
    }

    for y in 225..canvas.height {
        for x in 0..canvas.width {
            if y as f32 > 225.0 + (x as f32 * 0.05).sin() * 4.0 + (x as f32 * 0.13).sin() * 2.0 {
                canvas.set(x, y, Rgba::rgb(18, 12, 28));
            }
        }
    }
    canvas
}
