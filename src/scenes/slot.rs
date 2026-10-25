//! Picks a save slot, either to load it or to start a new game in it.

use bevy::math::Vec2;

use super::map_scene;
use super::scene::{Scene, SceneCtx};
use super::story::{INTRO, StoryScene};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use super::world_scene::WorldScene;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR, LINE_HEIGHT};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::gameplay::game_state::{GameState, SLOT_COUNT};
use crate::scene_any;

/// Height of one slot in the list: its name and a summary line.
const ROW: i32 = 25;
const PROMPT_BOX: IRect = IRect::new(40, SCREEN_HEIGHT - 62, SCREEN_WIDTH - 80, 50);

/// The overwrite question, drawn like a dialogue box. There is no world beneath the title for a DialogueScene.
struct Prompt {
    slot: i32,
    lines: Vec<String>,
    shown: f32,
    blip: f32,
    choice: i32,
}

impl Prompt {
    fn length(&self) -> usize {
        self.lines.iter().map(|l| l.chars().count()).sum()
    }

    fn done(&self) -> bool {
        self.shown >= self.length() as f32
    }
}

pub struct SlotScene {
    new_game: bool,
    slots: Vec<(Option<GameState>, String)>,
    row: usize,
    prompt: Option<Prompt>,
}

impl SlotScene {
    pub fn new(new_game: bool) -> SlotScene {
        SlotScene { new_game, slots: Vec::new(), row: 0, prompt: None }
    }

    fn start(ctx: &mut SceneCtx, slot: i32) {
        audio::sfx(Sound::MenuSelect);
        let state = GameState::new_game(slot);
        ctx.replace(StoryScene::new(INTRO, Some(Box::new(WorldScene::new(state, true)))));
    }

    fn update_prompt(&mut self, ctx: &mut SceneCtx, dt: f32) {
        let Some(prompt) = self.prompt.as_mut() else { return };
        if !prompt.done() {
            let speed = if input::held(GameAction::Cancel) { 3.0 } else { 1.0 };
            prompt.shown += dt * 55.0 * speed;
            prompt.blip -= dt;
            if prompt.blip <= 0.0 {
                audio::sfx(Sound::TextBlip);
                prompt.blip = 0.05;
            }
            if input::pressed(GameAction::Confirm) {
                prompt.shown = prompt.length() as f32;
            }
            return;
        }
        if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
            prompt.choice = 1 - prompt.choice;
            audio::sfx(Sound::MenuMove);
        }
        if input::pressed(GameAction::Confirm) || input::pressed(GameAction::Cancel) {
            let yes = prompt.choice == 0 && input::pressed(GameAction::Confirm);
            audio::sfx(if yes { Sound::MenuSelect } else { Sound::MenuCancel });
            let slot = prompt.slot;
            self.prompt = None;
            input::consume();
            if yes {
                SlotScene::start(ctx, slot);
            }
        }
    }

    fn draw_prompt(prompt: &Prompt, batch: &mut Batch) {
        let rect = PROMPT_BOX;
        batch.draw_window(rect);
        let mut remaining = prompt.shown as i32;
        for (i, line) in prompt.lines.iter().enumerate() {
            let count = remaining.clamp(0, line.chars().count() as i32) as usize;
            let visible: String = line.chars().take(count).collect();
            remaining -= line.chars().count() as i32;
            let pos = Vec2::new((rect.x + 8) as f32, (rect.y + 6 + i as i32 * LINE_HEIGHT) as f32);
            font::draw(batch, &visible, pos, palette::TEXT, Some(palette::TEXT_SHADOW));
        }
        if prompt.done() {
            let choice = IRect::new(rect.right() - 52, rect.y - 34, 48, 32);
            batch.draw_window(choice);
            let (cx, cy) = (choice.x as f32, choice.y as f32);
            font::draw(batch, "Yes", Vec2::new(cx + 16.0, cy + 5.0), palette::TEXT, Some(palette::TEXT_SHADOW));
            font::draw(batch, "No", Vec2::new(cx + 16.0, cy + 16.0), palette::TEXT, Some(palette::TEXT_SHADOW));
            let cursor = Vec2::new(cx + 6.0, cy + 5.0 + prompt.choice as f32 * 11.0);
            font::draw(batch, &CURSOR.to_string(), cursor, palette::HIGHLIGHT, None);
        }
    }
}

impl Scene for SlotScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
        self.slots = (1..=SLOT_COUNT)
            .map(|slot| {
                let state = GameState::load(slot);
                let summary = match &state {
                    Some(s) => {
                        let area = map_scene::map_name(&s.map_id).unwrap_or_else(|| s.map_id.clone());
                        let seconds = s.play_time as i64;
                        format!("Lv {}  {}  {}:{:02}", s.level, area, seconds / 3600, seconds / 60 % 60)
                    }
                    None => "Empty".to_string(),
                };
                (state, summary)
            })
            .collect();
        if !self.new_game {
            self.row = self.slots.iter().position(|s| s.0.is_some()).unwrap_or(0);
        }
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        if self.prompt.is_some() {
            self.update_prompt(ctx, dt);
            return;
        }
        let count = self.slots.len();
        if input::pressed(GameAction::Up) {
            self.row = (self.row + count - 1) % count;
        }
        if input::pressed(GameAction::Down) {
            self.row = (self.row + 1) % count;
        }
        if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
            audio::sfx(Sound::MenuMove);
        }
        if input::pressed(GameAction::Cancel) {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        if !input::pressed(GameAction::Confirm) {
            return;
        }

        let slot = self.row as i32 + 1;
        let existing = &self.slots[self.row].0;
        if !self.new_game {
            match existing {
                Some(state) => {
                    audio::sfx(Sound::MenuSelect);
                    ctx.replace(WorldScene::new(state.clone(), true));
                }
                None => audio::sfx(Sound::MenuCancel),
            }
            return;
        }
        if existing.is_none() {
            SlotScene::start(ctx, slot);
            return;
        }
        audio::sfx(Sound::MenuSelect);
        input::consume();
        let text = format!("Slot {slot} already holds a journey. Overwrite it?");
        self.prompt =
            Some(Prompt { slot, lines: font::wrap(&text, PROMPT_BOX.w - 16), shown: 0.0, blip: 0.0, choice: 0 });
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::BLACK.mul(0.55));
        let rect = IRect::new(SCREEN_WIDTH / 2 - 140, 18, 280, 28 + self.slots.len() as i32 * ROW);
        batch.draw_window(rect);
        let heading = if self.new_game { "Start a new journey in..." } else { "Continue which journey?" };
        let heading_pos = Vec2::new((rect.x + 10) as f32, (rect.y + 6) as f32);
        font::draw(batch, heading, heading_pos, palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
        for (i, (state, summary)) in self.slots.iter().enumerate() {
            let y = (rect.y + 22 + i as i32 * ROW) as f32;
            let x = (rect.x + 24) as f32;
            let color = if i == self.row { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, &format!("Slot {}", i + 1), Vec2::new(x, y), color, Some(palette::TEXT_SHADOW));
            let summary_color = if state.is_none() { palette::TEXT_DIM } else { palette::TEXT };
            font::draw(batch, summary, Vec2::new(x, y + 11.0), summary_color, Some(palette::TEXT_SHADOW));
            if i == self.row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new((rect.x + 10) as f32, y), palette::HIGHLIGHT, None);
            }
        }
        if let Some(prompt) = &self.prompt {
            SlotScene::draw_prompt(prompt, batch);
        }
    }
}
