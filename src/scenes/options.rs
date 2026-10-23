//! Volume, screen shake, fullscreen and a controls reference.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::{palette, settings};
use crate::scene_any;

const ROWS: [&str; 7] = ["Music", "Sound effects", "Screen shake", "Damage numbers", "Fullscreen", "Controls", "Back"];

const CONTROLS: [(&str, &str, &str); 11] = [
    ("Move", "WASD / arrows", "Left stick"),
    ("Aim", "Mouse", "Right stick"),
    ("Melee (and talk)", "J or right click", "A"),
    ("Throw (hold to charge)", "K or left click", "RT"),
    ("Dodge", "Space", "B"),
    ("Guard (tap early to parry)", "Shift", "LT"),
    ("Jump", "C or L", "Y"),
    ("Tether", "F or I", "X"),
    ("Skill", "R (hold to charge)", "RB (hold to charge)"),
    ("Element mode", "Q / E, wheel, 1-6", "LB"),
    ("Menu", "Esc / Tab", "Start"),
];

pub struct OptionsScene {
    row: usize,
    controls: bool,
}

impl OptionsScene {
    pub fn new() -> OptionsScene {
        OptionsScene { row: 0, controls: false }
    }

    fn close(ctx: &mut SceneCtx) {
        audio::sfx(Sound::MenuCancel);
        ctx.pop();
        input::consume();
    }

    fn draw_options(&self, batch: &mut Batch) {
        let s = settings::current();
        let rect = IRect::new(SCREEN_WIDTH / 2 - 120, 60, 240, 24 + ROWS.len() as i32 * 14);
        batch.draw_window(rect);
        let title_pos = Vec2::new((rect.x + 10) as f32, (rect.y + 6) as f32);
        font::draw(batch, "Options", title_pos, palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
        let on_off = |on: bool| if on { "On".to_string() } else { "Off".to_string() };
        for (i, row) in ROWS.iter().enumerate() {
            let y = (rect.y + 22 + i as i32 * 14) as f32;
            let color = if i == self.row { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, row, Vec2::new((rect.x + 24) as f32, y), color, Some(palette::TEXT_SHADOW));
            let value = match *row {
                "Music" => meter(s.music_volume),
                "Sound effects" => meter(s.sfx_volume),
                "Screen shake" => on_off(s.screen_shake),
                "Damage numbers" => on_off(s.damage_numbers),
                "Fullscreen" => on_off(s.fullscreen),
                _ => String::new(),
            };
            let value_pos = Vec2::new((rect.right() - 12 - font::measure(&value)) as f32, y);
            font::draw(batch, &value, value_pos, palette::TEXT_DIM, Some(palette::TEXT_SHADOW));
            if i == self.row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new((rect.x + 10) as f32, y), palette::HIGHLIGHT, None);
            }
        }
    }

    fn draw_controls(batch: &mut Batch) {
        let rect = IRect::new(30, 24, SCREEN_WIDTH - 60, 40 + CONTROLS.len() as i32 * 14);
        batch.draw_window(rect);
        let (x, y) = (rect.x as f32, (rect.y + 6) as f32);
        font::draw(batch, "Controls", Vec2::new(x + 10.0, y), palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
        font::draw(batch, "Keyboard / mouse", Vec2::new(x + 170.0, y), palette::TEXT_DIM, None);
        font::draw(batch, "Gamepad", Vec2::new(x + 320.0, y), palette::TEXT_DIM, None);
        for (i, (action, keys, pad)) in CONTROLS.iter().enumerate() {
            let y = (rect.y + 24 + i as i32 * 14) as f32;
            font::draw(batch, action, Vec2::new(x + 10.0, y), palette::TEXT, Some(palette::TEXT_SHADOW));
            font::draw(batch, keys, Vec2::new(x + 170.0, y), palette::TEXT, Some(palette::TEXT_SHADOW));
            font::draw(batch, pad, Vec2::new(x + 320.0, y), palette::TEXT, Some(palette::TEXT_SHADOW));
        }
    }
}

fn meter(v: f32) -> String {
    format!("{:.<10}", "|".repeat((v * 10.0).round() as usize))
}

fn step_volume(v: f32, step: i32) -> f32 {
    (((v + step as f32 * 0.1) * 10.0).round() / 10.0).clamp(0.0, 1.0)
}

impl Scene for OptionsScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        if self.controls {
            if input::pressed(GameAction::Cancel) || input::pressed(GameAction::Confirm) {
                self.controls = false;
                audio::sfx(Sound::MenuCancel);
            }
            return;
        }
        let count = ROWS.len();
        if input::pressed(GameAction::Up) {
            self.row = (self.row + count - 1) % count;
        }
        if input::pressed(GameAction::Down) {
            self.row = (self.row + 1) % count;
        }
        if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
            audio::sfx(Sound::MenuMove);
        }

        let step = if input::pressed(GameAction::Left) {
            -1
        } else if input::pressed(GameAction::Right) {
            1
        } else {
            0
        };
        let confirm = input::pressed(GameAction::Confirm);
        let toggle = step != 0 || confirm;
        let mut s = settings::current();
        match ROWS[self.row] {
            "Music" if step != 0 => {
                s.music_volume = step_volume(s.music_volume, step);
                settings::set(s);
                settings::save();
            }
            "Sound effects" if step != 0 => {
                s.sfx_volume = step_volume(s.sfx_volume, step);
                settings::set(s);
                settings::save();
                audio::sfx(Sound::Coin);
            }
            "Screen shake" if toggle => {
                s.screen_shake = !s.screen_shake;
                settings::set(s);
                settings::save();
                audio::sfx(Sound::MenuSelect);
            }
            "Damage numbers" if toggle => {
                s.damage_numbers = !s.damage_numbers;
                settings::set(s);
                settings::save();
                audio::sfx(Sound::MenuSelect);
            }
            "Fullscreen" if toggle => {
                s.fullscreen = !s.fullscreen;
                settings::set(s);
                settings::save();
            }
            "Controls" if confirm => {
                self.controls = true;
                audio::sfx(Sound::MenuSelect);
            }
            "Back" if confirm => {
                OptionsScene::close(ctx);
                return;
            }
            _ => {}
        }
        if input::pressed(GameAction::Cancel) {
            OptionsScene::close(ctx);
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::BLACK.mul(0.55));
        if self.controls {
            OptionsScene::draw_controls(batch);
        } else {
            self.draw_options(batch);
        }
    }
}
