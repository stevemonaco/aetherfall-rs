//! The Soul Blazer beat after a rift is sealed: the screen cuts to the town it belonged to, settles on whatever came
//! back (a house, a townsperson, a new path) and plays it into being, then returns to where the hero stood. The town
//! runs in a world of its own so the real one is left exactly as it was.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH, World};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::font;
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::palette;
use crate::entities::combat_effects::Shockwave;
use crate::entities::effects::Particle;
use crate::gameplay::game_state::GameState;
use crate::scene_any;
use crate::world::map::tile_center;

const FADE: f32 = 0.5;
/// When the restored place appears, and when the scene starts fading back.
const REVEAL: f32 = 1.3;
const END: f32 = 4.2;

pub struct RestoreScene {
    town: World,
    flag: String,
    label: String,
    focus: Vec2,
    time: f32,
    revealed: bool,
}

impl RestoreScene {
    /// `state` is the game as it is now; the town is shown as it was a moment before `flag` was set.
    pub fn new(mut state: GameState, town: &str, flag: &str, label: &str) -> RestoreScene {
        state.flags.remove(flag);
        let mut world = World::new(state, false);
        world.load_map(town, None, Direction::Down);
        world.hide_hero();
        let rank = |kind: &str| match kind {
            "building" => 0,
            "npc" => 1,
            "span" | "decor" => 2,
            _ => 3,
        };
        let focus = world
            .data
            .entities
            .iter()
            .filter(|e| e.extra.values().any(|v| v.as_str().is_some_and(|s| s.split(',').any(|c| c == flag))))
            .min_by_key(|e| rank(&e.kind))
            .map(|e| tile_center(e.x as i32 + e.int("w", 1) / 2, e.y as i32 + e.int("h", 1) / 2))
            .unwrap_or(world.hero.pos);
        RestoreScene {
            town: world,
            flag: flag.to_string(),
            label: label.to_string(),
            focus,
            time: 0.0,
            revealed: false,
        }
    }

    fn reveal(&mut self) {
        self.revealed = true;
        self.town.state.flags.insert(self.flag.clone());
        self.town.flash(Rgba::rgb(240, 225, 255), 0.5);
        self.town.add(Shockwave::new(self.focus + Vec2::new(0.0, 8.0), 40.0, Rgba::rgb(230, 200, 255)));
        Particle::burst(&mut self.town, self.focus, Rgba::rgb(240, 220, 255), 40, 90.0, 1.0, true);
        audio::play(Sound::LevelUp, 0.9, 0.2);
    }
}

impl Scene for RestoreScene {
    scene_any!();

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.time += dt;
        self.town.update(dt);
        self.town.requests.clear();
        let (w, h) = (self.town.map.pixel_width(), self.town.map.pixel_height());
        self.town.camera.follow(self.focus, w, h, 0.0);
        if !self.revealed && self.time >= REVEAL {
            self.reveal();
        }
        if self.time >= END {
            ctx.pop();
            if let Some(world) = ctx.world() {
                world.resume_music();
            }
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        self.town.draw(batch);
        batch.begin_screen();
        let fade = if self.time < FADE {
            1.0 - self.time / FADE
        } else if self.time > END - FADE {
            (self.time - (END - FADE)) / FADE
        } else {
            0.0
        };
        if self.revealed {
            let text = format!("Restored: {}", self.label);
            let width = font::measure(&text) + 16;
            let rect = IRect::new((SCREEN_WIDTH - width) / 2, SCREEN_HEIGHT - 40, width, 18);
            batch.draw_window(rect);
            let at = Vec2::new((rect.x + 8) as f32, (rect.y + 5) as f32);
            font::draw(batch, &text, at, palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
        }
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::BLACK.mul(fade.clamp(0.0, 1.0)));
    }
}
