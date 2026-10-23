//! A typewriter text box. With an `ask` callback, the last page ends in a Yes/No choice.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH, World};
use crate::core::audio::{self, Sound};
use crate::core::font::{self, CURSOR, LINE_HEIGHT};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::scene_any;

const LINES_PER_PAGE: usize = 3;
const CHARS_PER_SECOND: f32 = 55.0;
const BOX: IRect = IRect::new(40, SCREEN_HEIGHT - 62, SCREEN_WIDTH - 80, 50);

pub type Then = Box<dyn FnOnce(&mut World)>;
pub type Ask = Box<dyn FnOnce(&mut World, bool)>;

pub struct DialogueScene {
    pages: Vec<Vec<String>>,
    speaker: String,
    then: Option<Then>,
    ask: Option<Ask>,
    page: usize,
    shown: f32,
    blip: f32,
    choice: i32,
    time: f32,
}

impl DialogueScene {
    pub fn new(lines: Vec<String>, speaker: String, then: Option<Then>) -> DialogueScene {
        let mut pages = Vec::new();
        for text in &lines {
            let wrapped = font::wrap(text, BOX.w - 16);
            for chunk in wrapped.chunks(LINES_PER_PAGE) {
                pages.push(chunk.to_vec());
            }
        }
        if pages.is_empty() {
            pages.push(vec![String::new()]);
        }
        DialogueScene { pages, speaker, then, ask: None, page: 0, shown: 0.0, blip: 0.0, choice: 0, time: 0.0 }
    }

    pub fn ask(question: String, speaker: String, ask: Ask) -> DialogueScene {
        let mut scene = DialogueScene::new(vec![question], speaker, None);
        scene.ask = Some(ask);
        scene
    }

    fn page_length(&self) -> usize {
        self.pages[self.page].iter().map(|l| l.chars().count()).sum()
    }

    fn page_done(&self) -> bool {
        self.shown >= self.page_length() as f32
    }

    fn asking(&self) -> bool {
        self.ask.is_some() && self.page == self.pages.len() - 1 && self.page_done()
    }
}

impl Scene for DialogueScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.time += dt;
        if !self.page_done() {
            let speed = if input::held(GameAction::Cancel) { 3.0 } else { 1.0 };
            self.shown += dt * CHARS_PER_SECOND * speed;
            self.blip -= dt;
            if self.blip <= 0.0 {
                audio::sfx(Sound::TextBlip);
                self.blip = 0.05;
            }
            if input::pressed(GameAction::Confirm) {
                self.shown = self.page_length() as f32;
            }
            return;
        }

        if self.asking() {
            if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
                self.choice = 1 - self.choice;
                audio::sfx(Sound::MenuMove);
            }
            if input::pressed(GameAction::Confirm) || input::pressed(GameAction::Cancel) {
                let yes = self.choice == 0 && input::pressed(GameAction::Confirm);
                audio::sfx(if yes { Sound::MenuSelect } else { Sound::MenuCancel });
                ctx.pop();
                input::consume();
                if let (Some(ask), Some(world)) = (self.ask.take(), ctx.world()) {
                    ask(world, yes);
                }
            }
            return;
        }

        if input::pressed(GameAction::Confirm) || input::pressed(GameAction::Cancel) {
            if self.page < self.pages.len() - 1 {
                self.page += 1;
                self.shown = 0.0;
                audio::sfx(Sound::MenuMove);
            } else {
                ctx.pop();
                input::consume();
                if let (Some(then), Some(world)) = (self.then.take(), ctx.world()) {
                    then(world);
                }
            }
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        batch.draw_window(BOX);
        if !self.speaker.is_empty() {
            let tab = IRect::new(BOX.x + 6, BOX.y - 12, font::measure(&self.speaker) + 12, 14);
            batch.draw_window(tab);
            font::draw(
                batch,
                &self.speaker,
                Vec2::new((tab.x + 6) as f32, (tab.y + 3) as f32),
                palette::HIGHLIGHT,
                Some(palette::TEXT_SHADOW),
            );
        }

        let mut remaining = self.shown as i32;
        for (i, line) in self.pages[self.page].iter().enumerate() {
            let count = remaining.clamp(0, line.chars().count() as i32) as usize;
            let visible: String = line.chars().take(count).collect();
            remaining -= line.chars().count() as i32;
            let pos = Vec2::new((BOX.x + 8) as f32, (BOX.y + 6 + i as i32 * LINE_HEIGHT) as f32);
            font::draw(batch, &visible, pos, palette::TEXT, Some(palette::TEXT_SHADOW));
        }

        if self.page_done() && !self.asking() && (self.time * 3.0) as i32 % 2 == 0 {
            font::draw(
                batch,
                &CURSOR.to_string(),
                Vec2::new((BOX.right() - 12) as f32, (BOX.bottom() - 11) as f32),
                palette::HIGHLIGHT,
                None,
            );
        }

        if self.asking() {
            let choice = IRect::new(BOX.right() - 52, BOX.y - 34, 48, 32);
            batch.draw_window(choice);
            let (cx, cy) = (choice.x as f32, choice.y as f32);
            font::draw(batch, "Yes", Vec2::new(cx + 16.0, cy + 5.0), palette::TEXT, Some(palette::TEXT_SHADOW));
            font::draw(batch, "No", Vec2::new(cx + 16.0, cy + 16.0), palette::TEXT, Some(palette::TEXT_SHADOW));
            font::draw(
                batch,
                &CURSOR.to_string(),
                Vec2::new(cx + 6.0, cy + 5.0 + self.choice as f32 * 11.0),
                palette::HIGHLIGHT,
                None,
            );
        }
    }
}
