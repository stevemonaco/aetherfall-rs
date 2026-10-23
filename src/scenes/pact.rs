//! Taking on or setting aside Aether Pacts. More Heat, more reward.

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
use crate::gameplay::game_data::data;
use crate::gameplay::pacts;
use crate::scene_any;

pub struct PactScene {
    row: usize,
}

impl PactScene {
    pub fn new() -> PactScene {
        PactScene { row: 0 }
    }
}

impl Scene for PactScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        let count = pacts::ALL.len();
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
        let Some(world) = ctx.world() else { return };
        let state = &mut world.state;
        let pact = &pacts::ALL[self.row];
        if !state.check(Some(pact.unlock)) {
            audio::sfx(Sound::MenuCancel);
            return;
        }
        if !state.pacts.remove(pact.id) {
            state.pacts.insert(pact.id.to_string());
        }
        let sound = if state.pacts.contains(pact.id) { Sound::BossRoar } else { Sound::MenuSelect };
        audio::play(sound, 0.6, 0.4);
    }

    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        let state = &world.state;
        batch.begin_screen();
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::rgb(20, 4, 12).mul(0.85));
        let rect = IRect::new(SCREEN_WIDTH / 2 - 190, 24, 380, 40 + pacts::ALL.len() as i32 * 22);
        batch.draw_window(rect);
        let shadow = Some(palette::TEXT_SHADOW);
        let (x, top) = (rect.x as f32, rect.y as f32);
        font::draw(batch, "Aether Pacts", Vec2::new(x + 10.0, top + 6.0), Rgba::rgb(255, 120, 120), shadow);
        let summary = format!("Heat {}   Rewards x{:.2}", pacts::heat(state), pacts::reward_multiplier(state));
        let summary_pos = Vec2::new((rect.right() - 10 - font::measure(&summary)) as f32, top + 6.0);
        font::draw(batch, &summary, summary_pos, palette::HIGHLIGHT, shadow);
        for (i, pact) in pacts::ALL.iter().enumerate() {
            let y = top + 24.0 + i as f32 * 22.0;
            let unlocked = state.check(Some(pact.unlock));
            let on = state.pacts.contains(pact.id);
            if i == self.row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new(x + 8.0, y), palette::HIGHLIGHT, None);
            }
            let label =
                if unlocked { format!("{} {}", if on { "[x]" } else { "[ ]" }, pact.name) } else { "[?] ???".into() };
            let color = if on {
                Rgba::rgb(255, 140, 120)
            } else if unlocked {
                palette::TEXT
            } else {
                palette::TEXT_DIM
            };
            font::draw(batch, &label, Vec2::new(x + 20.0, y), color, shadow);
            font::draw(
                batch,
                &format!("+{}", pact.heat),
                Vec2::new((rect.right() - 30) as f32, y),
                palette::TEXT_DIM,
                None,
            );
            let description = if unlocked { pact.description } else { "Unlocked by defeating a greater foe." };
            font::draw(batch, description, Vec2::new(x + 36.0, y + 10.0), palette::TEXT_DIM, None);
        }
        let records: Vec<String> = state
            .flags
            .iter()
            .filter_map(|f| f.strip_prefix("heat:"))
            .map(|r| {
                let (boss, heat) = r.split_once('=').unwrap_or((r, ""));
                let name = data().enemies.get(&boss.replace("boss_", "")).map_or(boss, |e| e.name.as_str());
                format!("{name} {heat}")
            })
            .collect();
        if !records.is_empty() {
            let lines = font::wrap(&format!("Best Heat: {}", records.join(", ")), rect.w - 20);
            for (i, line) in lines.iter().enumerate() {
                let pos = Vec2::new(x + 10.0, (rect.bottom() + 4 + i as i32 * 10) as f32);
                font::draw(batch, line, pos, palette::HIGHLIGHT, shadow);
            }
        }
    }
}
