//! Full-screen narration pages, used for the intro and the end of the slice.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCmd, SceneCtx};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR, LINE_HEIGHT};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::scene_any;

pub const INTRO: &[&str] = &[
    "A century ago, the sky-cities of the Aether Concord fell burning from the heavens.",
    "The Aetherfall scattered the Concord's great crystals across the land. From their embers rose monsters, and rifts that swallowed whole villages.",
    "Beneath the village of Leafhollow, a sealed vault has slept through all those years...",
    "...until today.",
];

pub const ENDING: &[&str] = &[
    "The Aether Blade passes through Vael's last ward. With a final cry, the Sovereign comes apart into wind, and the Gale Crystal falls free into your hands.",
    "Without its keeper, the Sky Citadel drifts down through the clouds and settles at last upon the mountain, its long flight over.",
    "The four crystals rest together again. Across the valley the rifts close one by one, and what they took walks home.",
    "In Ashford, Corvin banks the forge's fire. In Brinewatch the lighthouse turns. In Highcrag the hearth burns all night.",
    "In Leafhollow, Elder Orin keeps a chair by the fire for the sleeper who woke.",
    "AETHERFALL\n\nThank you for playing!",
    "Sprites: 7Soul's RPG Graphics Pack (CC BY-ND 4.0)\nMusic: Joel Steudler's 16-Bit Fantasy Action pack\nEverything else is drawn and synthesized in code.",
];

pub struct StoryScene {
    pages: &'static [&'static str],
    next: Option<Box<dyn Scene>>,
    has_next: bool,
    page: usize,
    time: f32,
}

impl StoryScene {
    /// With `next`, the story fills the screen and hands over to that scene; without it, it overlays and pops.
    pub fn new(pages: &'static [&'static str], next: Option<Box<dyn Scene>>) -> StoryScene {
        StoryScene { pages, has_next: next.is_some(), next, page: 0, time: 0.0 }
    }
}

impl Scene for StoryScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        !self.has_next
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
        if self.has_next {
            audio::play_music(Some("Menu-DreamingDarkly"));
        }
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.time += dt;
        let advance = input::pressed(GameAction::Confirm)
            || input::pressed(GameAction::Cancel)
            || input::pressed(GameAction::Click);
        if self.time < 0.4 || !advance {
            return;
        }
        audio::sfx(Sound::MenuMove);
        self.time = 0.0;
        self.page += 1;
        if self.page < self.pages.len() {
            return;
        }
        match self.next.take() {
            Some(next) => ctx.cmds.push(SceneCmd::Replace(next)),
            None => ctx.pop(),
        }
        input::consume();
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        let fade = if self.has_next { 1.0 } else { 0.85 };
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::BLACK.mul(fade));
        let Some(page) = self.pages.get(self.page) else { return };
        let alpha = (self.time / 0.6).min(1.0);
        let lines = font::wrap(page, 360);
        let mut y = SCREEN_HEIGHT as f32 / 2.0 - 10.0 - lines.len() as f32 * LINE_HEIGHT as f32 / 2.0;
        for line in &lines {
            let center = Vec2::new(SCREEN_WIDTH as f32 / 2.0, y);
            font::draw_centered(batch, line, center, palette::TEXT.mul(alpha), Some(palette::TEXT_SHADOW.mul(alpha)));
            y += LINE_HEIGHT as f32;
        }
        if self.time > 0.8 {
            let blink = ((self.time * 3.0) as i32 % 2) as f32;
            let pos = Vec2::new((SCREEN_WIDTH - 20) as f32, (SCREEN_HEIGHT - 20) as f32);
            font::draw(batch, &CURSOR.to_string(), pos, palette::HIGHLIGHT.mul(blink), None);
        }
    }
}
