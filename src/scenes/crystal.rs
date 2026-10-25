//! The save crystal menu: the game is already saved; from here the player can fast travel or open the Weave.
//! Destinations list towns first, then every other place by its foes' level, coloured by the kind of place.

use bevy::math::Vec2;

use super::map_scene;
use super::scene::{Scene, SceneCtx};
use super::weave::WeaveScene;
use super::world::SCREEN_WIDTH;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::gameplay::game_data::data;
use crate::gameplay::items;
use crate::scene_any;
use crate::world::map::MapData;

const VISIBLE: usize = 12;
const LINE: i32 = 12;

struct Row {
    label: String,
    map: String,
    tile: (i32, i32),
    level: i32,
    color: Rgba,
}

/// The world map's kind for the region holding `map` ("town", "field", "side", "dungeon" or "blight").
fn region_kind(map: &str) -> &'static str {
    data().world.iter().find(|n| n.maps.iter().any(|m| m == map)).map_or("", |n| n.kind.as_str())
}

fn kind_color(kind: &str) -> Rgba {
    match kind {
        "town" => Rgba::rgb(150, 230, 150),
        "field" => Rgba::rgb(240, 220, 120),
        "side" => Rgba::rgb(240, 170, 100),
        "dungeon" => Rgba::rgb(240, 110, 100),
        "blight" => Rgba::rgb(200, 140, 255),
        _ => palette::TEXT,
    }
}

pub struct CrystalScene {
    rows: Vec<Row>,
    row: usize,
    scroll: usize,
}

impl CrystalScene {
    pub fn new() -> CrystalScene {
        CrystalScene { rows: Vec::new(), row: 0, scroll: 0 }
    }
}

impl Scene for CrystalScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, ctx: &mut SceneCtx) {
        input::consume();
        let fixed = |label: &str| Row {
            label: label.to_string(),
            map: String::new(),
            tile: (0, 0),
            level: 0,
            color: palette::TEXT,
        };
        self.rows = vec![fixed("Stay here"), fixed("Open the Weave")];
        let Some(world) = ctx.world() else { return };
        let mut places: Vec<(bool, Row)> = Vec::new();
        for flag in world.state.flags.iter().filter(|f| f.starts_with("crystal:")) {
            let parts: Vec<&str> = flag.split(':').collect();
            if parts.len() < 3 || parts[1] == world.map_id() {
                continue;
            }
            let Some(name) = map_scene::map_name(parts[1]) else { continue };
            let mut xy = parts[2].split('_').map(|v| v.parse::<i32>().unwrap_or(0));
            let (mut x, mut y) = (xy.next().unwrap_or(0), xy.next().unwrap_or(0));
            let map = MapData::load(parts[1]);
            let saves: Vec<(i32, i32)> =
                map.entities.iter().filter(|e| e.kind == "save").map(|e| (e.x as i32, e.y as i32)).collect();
            // A crystal that has since been moved still takes you to the map's crystal, not where it used to stand.
            if !saves.is_empty() && !saves.contains(&(x, y)) && !items::TOWNS.iter().any(|t| t.0 == parts[1]) {
                (x, y) = saves[0];
            }
            if places.iter().any(|(_, r)| r.map == parts[1] && r.tile == (x, y + 1)) {
                continue;
            }
            let kind = region_kind(parts[1]);
            let row = Row {
                label: name,
                map: parts[1].to_string(),
                tile: (x, y + 1),
                level: map.level,
                color: kind_color(kind),
            };
            places.push((kind == "town", row));
        }
        places.sort_by(|(town_a, a), (town_b, b)| {
            town_b.cmp(town_a).then(a.level.cmp(&b.level)).then(a.label.cmp(&b.label)).then(a.tile.cmp(&b.tile))
        });
        for (i, (_, row)) in places.iter().enumerate() {
            let twin = places.iter().filter(|(_, r)| r.label == row.label).count() > 1;
            let rank = places[..i].iter().filter(|(_, r)| r.label == row.label).count() + 1;
            let label = if twin { format!("{} ({rank})", row.label) } else { row.label.clone() };
            self.rows.push(Row { label, map: row.map.clone(), tile: row.tile, level: row.level, color: row.color });
        }
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        let count = self.rows.len();
        if input::pressed(GameAction::Up) {
            self.row = (self.row + count - 1) % count;
        }
        if input::pressed(GameAction::Down) {
            self.row = (self.row + 1) % count;
        }
        if input::pressed(GameAction::Up) || input::pressed(GameAction::Down) {
            audio::sfx(Sound::MenuMove);
        }
        self.scroll = self.scroll.clamp((self.row + 1).saturating_sub(VISIBLE), self.row);
        let confirm = input::pressed(GameAction::Confirm);
        if input::pressed(GameAction::Cancel) || (confirm && self.row == 0) {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        if confirm && self.row == 1 {
            audio::sfx(Sound::MenuSelect);
            ctx.push(WeaveScene::at_crystal());
            return;
        }
        if confirm {
            let row = &self.rows[self.row];
            audio::sfx(Sound::MenuSelect);
            ctx.pop();
            input::consume();
            if let Some(world) = ctx.world() {
                world.begin_warp(&row.map, row.tile, Direction::Down, true);
            }
        }
    }

    fn draw(&self, _below: &[Box<dyn Scene>], batch: &mut Batch) {
        batch.begin_screen();
        let shown = VISIBLE.min(self.rows.len());
        let rect = IRect::new(SCREEN_WIDTH / 2 - 130, 34, 260, 34 + shown as i32 * LINE);
        batch.draw_window(rect);
        let heading = Vec2::new((rect.x + 10) as f32, (rect.y + 6) as f32);
        font::draw(batch, "Your journey has been recorded.", heading, palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
        let top = rect.y + 22;
        for i in self.scroll..self.rows.len().min(self.scroll + VISIBLE) {
            let row = &self.rows[i];
            let y = (top + (i - self.scroll) as i32 * LINE) as f32;
            let selected = i == self.row;
            let color = if selected { row.color.lerp(Rgba::WHITE, 0.45) } else { row.color.mul(0.85) };
            font::draw(batch, &row.label, Vec2::new((rect.x + 22) as f32, y), color, Some(palette::TEXT_SHADOW));
            if i > 1 && row.level > 1 {
                let level = format!("Lv {}", row.level);
                let at = Vec2::new((rect.right() - 12 - font::measure(&level)) as f32, y);
                font::draw(batch, &level, at, palette::TEXT_DIM, Some(palette::TEXT_SHADOW));
            }
            if selected {
                font::draw(batch, &CURSOR.to_string(), Vec2::new((rect.x + 10) as f32, y), palette::HIGHLIGHT, None);
            }
        }
        let arrow_x = (rect.right() - 12) as f32;
        if self.scroll > 0 {
            font::draw(batch, "^", Vec2::new(arrow_x, (rect.y + 6) as f32), palette::TEXT_DIM, None);
        }
        if self.scroll + VISIBLE < self.rows.len() {
            let at = Vec2::new(arrow_x, (top + shown as i32 * LINE) as f32);
            font::draw(batch, "v", at, palette::TEXT_DIM, None);
        }
    }
}
