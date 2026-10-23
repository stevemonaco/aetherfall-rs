//! A smith tempering the blade with gold and monster materials, up to the level their forge allows.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx, world_below};
use super::world::SCREEN_WIDTH;
use crate::art::{combat, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::gameplay::game_state::GameState;
use crate::gameplay::items;
use crate::scene_any;

type Recipe = (i32, &'static [(&'static str, i32)]);

const RECIPES: [Recipe; 10] = [
    (200, &[("slime_gel", 3), ("beast_fang", 2)]),
    (500, &[("beast_fang", 3), ("ember_shard", 1)]),
    (1200, &[("tide_pearl", 3), ("beast_fang", 2)]),
    (2500, &[("frost_core", 3), ("storm_glass", 1)]),
    (5000, &[("storm_glass", 3), ("concord_alloy", 1)]),
    (8000, &[("blight_core", 2), ("concord_alloy", 2)]),
    (11000, &[("blight_core", 3), ("concord_alloy", 2)]),
    (15000, &[("blight_core", 4), ("concord_alloy", 3)]),
    (20000, &[("blight_core", 5), ("concord_alloy", 3)]),
    (26000, &[("blight_core", 6), ("concord_alloy", 4)]),
];

pub struct SmithScene {
    max_level: i32,
    message: String,
    row: usize,
}

impl SmithScene {
    pub fn new(max: i32) -> SmithScene {
        SmithScene { max_level: max, message: String::new(), row: 0 }
    }

    fn forge(&mut self, state: &mut GameState) {
        let level = state.smith_level;
        if level >= self.max_level || level as usize >= RECIPES.len() {
            self.message = "My forge can't take it any further. Seek a greater one.".into();
            audio::sfx(Sound::MenuCancel);
            return;
        }
        let (gold, materials) = RECIPES[level as usize];
        if state.gold < gold || materials.iter().any(|&(item, count)| state.count(item) < count) {
            self.message = "You're short on something. Come back when you have it all.".into();
            audio::sfx(Sound::MenuCancel);
            return;
        }
        state.gold -= gold;
        for &(item, count) in materials {
            state.take(item, count);
        }
        state.smith_level += 1;
        self.message = format!("There! Your blade is now +{}.", state.smith_level);
        audio::play(Sound::Break, 0.6, 0.5);
        audio::sfx(Sound::LevelUp);
    }
}

impl Scene for SmithScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, ctx: &mut SceneCtx) {
        input::consume();
        let maxed = ctx.world().is_some_and(|w| w.state.smith_level >= self.max_level);
        self.message = if maxed {
            "That blade is as fine as my forge can make it."
        } else {
            "Bring me materials and I'll temper that blade."
        }
        .into();
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
            self.row = 1 - self.row;
            audio::sfx(Sound::MenuMove);
        }
        if input::pressed(GameAction::Cancel) || (input::pressed(GameAction::Confirm) && self.row == 1) {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        if input::pressed(GameAction::Confirm)
            && let Some(world) = ctx.world()
        {
            self.forge(&mut world.state);
        }
    }

    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        let state = &world.state;
        batch.begin_screen();
        let shadow = Some(palette::TEXT_SHADOW);
        let rect = IRect::new(SCREEN_WIDTH / 2 - 150, 30, 300, 150);
        let (x, y) = (rect.x as f32, rect.y as f32);
        batch.draw_window(rect);
        let level = state.smith_level;
        font::draw(batch, "SMITHY", Vec2::new(x + 10.0, y + 6.0), palette::HIGHLIGHT, shadow);
        let gold = format!("{} G", state.gold);
        font::draw(
            batch,
            &gold,
            Vec2::new((rect.right() - 10 - font::measure(&gold)) as f32, y + 6.0),
            palette::GOLD,
            shadow,
        );
        let blade = combat::blade(state.mode);
        batch.draw_ex(
            blade,
            Vec2::new(x + 14.0, y + 26.0),
            None,
            Rgba::WHITE,
            0.0,
            Vec2::ZERO,
            Vec2::splat(2.0),
            false,
            false,
        );
        let atk = format!("Blade +{level}   ATK {}", state.weapon_atk(state.mode).round());
        font::draw(batch, &atk, Vec2::new(x + 56.0, y + 26.0), palette::TEXT, shadow);

        if level < self.max_level && (level as usize) < RECIPES.len() {
            let (cost, materials) = RECIPES[level as usize];
            let next_atk = state.weapon_atk(state.mode) / (1.0 + 0.08 * level as f32) * (1.0 + 0.08 * (level + 1) as f32);
            let next = format!("Next: +{}  (ATK {})", level + 1, next_atk.round());
            font::draw(batch, &next, Vec2::new(x + 14.0, y + 46.0), palette::TEXT_DIM, shadow);
            let mut row_y = y + 60.0;
            for &(item, count) in materials {
                let have = state.count(item);
                batch.draw(props::icon(item), Vec2::new(x + 18.0, row_y - 2.0), Rgba::WHITE);
                let text = format!("{}  {have}/{count}", items::name_of(item));
                let color = if have >= count { palette::TEXT } else { palette::LIFE };
                font::draw(batch, &text, Vec2::new(x + 34.0, row_y), color, shadow);
                row_y += 13.0;
            }
            let color = if state.gold >= cost { palette::GOLD } else { palette::LIFE };
            font::draw(batch, &format!("{cost} G"), Vec2::new(x + 34.0, row_y), color, shadow);
        } else {
            font::draw(batch, "No further upgrades here.", Vec2::new(x + 14.0, y + 46.0), palette::TEXT_DIM, shadow);
        }

        for (i, option) in ["Forge", "Leave"].iter().enumerate() {
            let row_y = (rect.bottom() - 32 + i as i32 * 12) as f32;
            let color = if i == self.row { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, option, Vec2::new(x + 30.0, row_y), color, shadow);
            if i == self.row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new(x + 18.0, row_y), palette::HIGHLIGHT, None);
            }
        }
        let footer = IRect::new(rect.x, rect.bottom() + 2, rect.w, 22);
        batch.draw_window(footer);
        font::draw(
            batch,
            &self.message,
            Vec2::new((footer.x + 8) as f32, (footer.y + 6) as f32),
            palette::TEXT,
            shadow,
        );
    }
}
