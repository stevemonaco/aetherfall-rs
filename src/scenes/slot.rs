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
use crate::gameplay::element::{Element, MODES};
use crate::gameplay::game_state::{GameState, SLOT_COUNT};
use crate::scene_any;
use crate::ui::widgets;

/// Height of one slot in the list: a framed card with two lines of summary.
const ROW: i32 = 28;
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

    fn draw_slot(
        batch: &mut Batch,
        frame: IRect,
        number: usize,
        state: Option<&GameState>,
        area: &str,
        selected: bool,
    ) {
        let shadow = Some(palette::TEXT_SHADOW);
        let (fill, border) = match (state.is_some(), selected) {
            (_, true) => (palette::WINDOW_FILL_LIGHT.mul(0.7), palette::HIGHLIGHT.mul(0.8)),
            (true, false) => (palette::WINDOW_FILL_LIGHT.mul(0.35), palette::WINDOW_BORDER.mul(0.45)),
            (false, false) => (Rgba::BLACK.mul(0.2), palette::WINDOW_BORDER.mul(0.2)),
        };
        batch.fill(frame, fill);
        batch.draw_rect(frame, border);
        let (x, y, right) = (frame.x as f32, frame.y as f32, frame.right() as f32);
        let badge = Vec2::new(x + 4.0, y + 4.0);
        widgets::slot(batch, badge, selected.then_some(palette::HIGHLIGHT), state.is_none());
        let number_color = if state.is_some() { palette::TEXT } else { palette::TEXT_DIM };
        font::draw_centered(batch, &number.to_string(), badge + Vec2::new(9.0, 5.0), number_color, None);

        let Some(s) = state else {
            font::draw(batch, "Empty", Vec2::new(x + 28.0, y + 9.0), palette::TEXT_DIM, None);
            return;
        };
        let level = format!("Lv {}", s.level);
        font::draw(batch, &level, Vec2::new(x + 28.0, y + 4.0), palette::HIGHLIGHT, shadow);
        let area_x = x + 28.0 + font::measure(&level) as f32 + 8.0;
        font::draw(batch, area, Vec2::new(area_x, y + 4.0), palette::TEXT, shadow);
        let seconds = s.play_time as i64;
        let time = format!("{}h {:02}m", seconds / 3600, seconds / 60 % 60);
        widgets::right_text(batch, &time, right - 6.0, y + 4.0, palette::TEXT_DIM);
        font::draw(batch, &format!("{} G", s.gold), Vec2::new(x + 28.0, y + 15.0), palette::GOLD, shadow);

        let mut elements: Vec<Element> = MODES.to_vec();
        if s.aether_awakened() {
            elements.push(Element::Aether);
        }
        for (n, &element) in elements.iter().rev().enumerate() {
            let pip = IRect::new(frame.right() - 13 - n as i32 * 9, frame.y + 15, 7, 7);
            if s.has_sigil(element) || element == Element::Aether {
                batch.fill(pip, element.glow());
                batch.draw_rect(pip, element.dark());
            } else {
                batch.fill(pip, Rgba::BLACK.mul(0.35));
                batch.draw_rect(pip, palette::WINDOW_BORDER.mul(0.3));
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
                let area = state
                    .as_ref()
                    .map(|s| map_scene::map_name(&s.map_id).unwrap_or_else(|| s.map_id.clone()))
                    .unwrap_or_default();
                (state, area)
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
        let rect = IRect::new(SCREEN_WIDTH / 2 - 160, 6, 320, 24 + self.slots.len() as i32 * ROW);
        batch.draw_window(rect);
        let heading = if self.new_game { "Start a new journey in..." } else { "Continue which journey?" };
        let heading_pos = Vec2::new((rect.x + 10) as f32, (rect.y + 6) as f32);
        font::draw(batch, heading, heading_pos, palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
        for (i, (state, area)) in self.slots.iter().enumerate() {
            let frame = IRect::new(rect.x + 20, rect.y + 20 + i as i32 * ROW, rect.w - 30, ROW - 2);
            let selected = i == self.row;
            SlotScene::draw_slot(batch, frame, i + 1, state.as_ref(), area, selected);
            if selected {
                let at = Vec2::new((rect.x + 8) as f32, (frame.y + 9) as f32);
                font::draw(batch, &CURSOR.to_string(), at, palette::HIGHLIGHT, None);
            }
        }
        if let Some(prompt) = &self.prompt {
            SlotScene::draw_prompt(prompt, batch);
        }
    }
}
