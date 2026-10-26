//! The save crystal menu: the game is already saved; from here the player can fast travel.
//! Destinations are grouped by the kind of place, towns first, each group sorted by its foes' level.

use bevy::math::Vec2;

use super::map_scene;
use super::scene::{Scene, SceneCtx};
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
use crate::ui::widgets;
use crate::world::map::MapData;

const VISIBLE: usize = 15;
const LINE: i32 = 12;
const KINDS: [(&str, &str); 5] =
    [("town", "Towns"), ("field", "Fields"), ("side", "Side paths"), ("dungeon", "Dungeons"), ("blight", "Blight")];

struct Row {
    label: String,
    map: String,
    tile: (i32, i32),
    level: i32,
    kind: &'static str,
}

/// One drawn line of the list: a destination (or fixed action) row, or the heading above a kind of place.
#[derive(Clone, Copy, PartialEq)]
enum Line {
    Row(usize),
    Heading(&'static str),
}

/// The world map's kind for the region holding `map` ("town", "field", "side", "dungeon" or "blight").
fn region_kind(map: &str) -> &'static str {
    data().world.iter().find(|n| n.maps.iter().any(|m| m == map)).map_or("", |n| n.kind.as_str())
}

fn kind_rank(kind: &str) -> usize {
    KINDS.iter().position(|k| k.0 == kind).unwrap_or(KINDS.len())
}

fn kind_heading(kind: &str) -> &'static str {
    KINDS.iter().find(|k| k.0 == kind).map_or("Elsewhere", |k| k.1)
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

    fn lines(&self) -> Vec<Line> {
        let mut lines = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            if i >= 2 && (i == 2 || self.rows[i - 1].kind != row.kind) {
                lines.push(Line::Heading(kind_heading(row.kind)));
            }
            lines.push(Line::Row(i));
        }
        lines
    }
}

impl Scene for CrystalScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, ctx: &mut SceneCtx) {
        input::consume();
        let fixed =
            |label: &str| Row { label: label.to_string(), map: String::new(), tile: (0, 0), level: 0, kind: "" };
        self.rows = vec![fixed("Stay here")];
        let Some(world) = ctx.world() else { return };
        let mut places: Vec<Row> = Vec::new();
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
            if places.iter().any(|r| r.map == parts[1] && r.tile == (x, y + 1)) {
                continue;
            }
            places.push(Row {
                label: name,
                map: parts[1].to_string(),
                tile: (x, y + 1),
                level: map.level,
                kind: region_kind(parts[1]),
            });
        }
        places.sort_by(|a, b| {
            kind_rank(a.kind)
                .cmp(&kind_rank(b.kind))
                .then(a.level.cmp(&b.level))
                .then(a.label.cmp(&b.label))
                .then(a.tile.cmp(&b.tile))
        });
        for (i, row) in places.iter().enumerate() {
            let twin = places.iter().filter(|r| r.label == row.label).count() > 1;
            let rank = places[..i].iter().filter(|r| r.label == row.label).count() + 1;
            let label = if twin { format!("{} ({rank})", row.label) } else { row.label.clone() };
            self.rows.push(Row { label, map: row.map.clone(), tile: row.tile, level: row.level, kind: row.kind });
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
        let lines = self.lines();
        let at = lines.iter().position(|&l| l == Line::Row(self.row)).unwrap_or(0);
        let heading = at > 0 && matches!(lines[at - 1], Line::Heading(_));
        self.scroll = self.scroll.clamp((at + 1).saturating_sub(VISIBLE), at - usize::from(heading));
        let confirm = input::pressed(GameAction::Confirm);
        if input::pressed(GameAction::Cancel) || (confirm && self.row == 0) {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
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
        let lines = self.lines();
        let shown = VISIBLE.min(lines.len());
        let rect = IRect::new(SCREEN_WIDTH / 2 - 130, 28, 260, 30 + shown as i32 * LINE);
        batch.draw_window(rect);
        let shadow = Some(palette::TEXT_SHADOW);
        let (x, right) = (rect.x as f32, rect.right() as f32);
        let heading = Vec2::new(x + 10.0, (rect.y + 6) as f32);
        font::draw(batch, "Your journey has been recorded.", heading, palette::HIGHLIGHT, shadow);
        widgets::rule(batch, rect.x + 6, rect.y + 18, rect.w - 12);
        let top = rect.y + 24;
        for (n, &line) in lines.iter().enumerate().skip(self.scroll).take(VISIBLE) {
            let y = top + (n - self.scroll) as i32 * LINE;
            let i = match line {
                Line::Heading(text) => {
                    widgets::section(batch, text, Vec2::new(x + 12.0, y as f32), rect.right() - 12);
                    continue;
                }
                Line::Row(i) => i,
            };
            let row = &self.rows[i];
            let selected = i == self.row;
            let color = if selected { palette::HIGHLIGHT } else { palette::TEXT };
            if i < 1 {
                let button = IRect::new(rect.x + 20, y - 2, rect.w - 40, 11);
                let fill =
                    if selected { palette::WINDOW_FILL_LIGHT.mul(0.8) } else { palette::WINDOW_FILL_LIGHT.mul(0.4) };
                batch.fill(button, fill);
                batch.draw_rect(button, palette::WINDOW_BORDER.mul(if selected { 0.6 } else { 0.3 }));
                font::draw_centered(batch, &row.label, Vec2::new(x + rect.w as f32 / 2.0, y as f32), color, shadow);
            } else {
                let accent = kind_color(row.kind);
                batch.fill(IRect::new(rect.x + 22, y - 1, 3, 9), if selected { accent } else { accent.mul(0.75) });
                font::draw(batch, &row.label, Vec2::new(x + 30.0, y as f32), color, shadow);
                if row.level > 1 {
                    let level = format!("Lv {}", row.level);
                    let level_color = if selected { palette::TEXT } else { palette::TEXT_DIM };
                    widgets::right_text(batch, &level, right - 14.0, y as f32, level_color);
                }
            }
            if selected {
                font::draw(batch, &CURSOR.to_string(), Vec2::new(x + 10.0, y as f32), palette::HIGHLIGHT, None);
            }
        }
        let arrow_x = right - 12.0;
        if self.scroll > 0 {
            font::draw(batch, "^", Vec2::new(arrow_x, (rect.y + 6) as f32), palette::TEXT_DIM, None);
        }
        if self.scroll + VISIBLE < lines.len() {
            let at = Vec2::new(arrow_x, (rect.bottom() - 10) as f32);
            font::draw(batch, "v", at, palette::TEXT_DIM, None);
        }
    }
}
