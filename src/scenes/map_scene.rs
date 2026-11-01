//! The world map: every region the player has seen, how they connect, towns' restoration and how many chests,
//! rifts and golden rifts each region still hides. A second view charts the current area tile by tile with what is left to find there.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx, world_below};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::art::combat;
use crate::core::audio::{self, Sound};
use crate::core::canvas::Canvas;
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::IRect;
use crate::core::gfx::{self, Batch};
use crate::core::input::{self, GameAction};
use crate::core::{noise, palette};
use crate::gameplay::game_data::{WorldNode, data};
use crate::gameplay::game_state::GameState;
use crate::scene_any;
use crate::scenes::world::World;
use crate::world::map::{MapData, Terrain};

/// The flags of a map's chests, rifts and golden rifts.
#[derive(Clone, Default)]
struct Counts {
    chests: Vec<String>,
    rifts: Vec<String>,
    golden: Vec<String>,
}

const GOLDEN: Rgba = Rgba::rgb(255, 150, 40);

static COUNTS: LazyLock<Mutex<HashMap<String, Counts>>> = LazyLock::new(Default::default);
static NAMES: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);

/// A map's display name, or `None` if no such map exists.
pub fn map_name(id: &str) -> Option<String> {
    let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(name) = names.get(id) {
        return Some(name.clone());
    }
    if !MapData::exists(id) {
        return None;
    }
    let name = MapData::load(id).name;
    names.insert(id.to_string(), name.clone());
    Some(name)
}

fn counts_for(map_id: &str) -> Counts {
    let mut counts = COUNTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(cached) = counts.get(map_id) {
        return cached.clone();
    }
    let mut found = Counts::default();
    if MapData::exists(map_id) {
        for e in MapData::load(map_id).entities {
            if e.kind == "chest" {
                found.chests.push(
                    e.str("flag").map(str::to_string).unwrap_or_else(|| format!("chest:{map_id}:{}_{}", e.x, e.y)),
                );
            }
            if e.kind == "rift" {
                found.rifts.push(format!(
                    "rift:{}",
                    e.str("id").map(str::to_string).unwrap_or_else(|| format!("{}_{}", e.x, e.y))
                ));
            }
            // Golden rifts sit in trial arenas outside the world graph, so count them by their entrance gates.
            if let Some(done) = e.str("done").filter(|_| e.kind == "trial") {
                found.golden.push(done.to_string());
            }
        }
    }
    counts.insert(map_id.to_string(), found.clone());
    found
}

fn visible(state: &GameState, node: &WorldNode) -> bool {
    node.maps.iter().any(|m| state.flags.contains(&format!("seen:{m}")))
        || node.show.is_some() && state.check(node.show.as_deref())
}

pub struct MapScene {
    nodes: Vec<&'static WorldNode>,
    here: Option<&'static WorldNode>,
    cursor: Option<&'static WorldNode>,
    time: f32,
    /// Showing the current area rather than the world.
    area: bool,
}

impl MapScene {
    pub fn new() -> MapScene {
        MapScene { nodes: Vec::new(), here: None, cursor: None, time: 0.0, area: false }
    }

    /// The corner for the details panel that hides the fewest regions, and never the selected one.
    fn details_rect(&self, cursor: &WorldNode) -> IRect {
        let (w, h) = (138, 70);
        let corners = [
            IRect::new(SCREEN_WIDTH - w - 12, 30, w, h),
            IRect::new(12, 30, w, h),
            IRect::new(SCREEN_WIDTH - w - 12, SCREEN_HEIGHT - h - 22, w, h),
            IRect::new(12, SCREEN_HEIGHT - h - 22, w, h),
        ];
        let hides = |rect: &IRect, n: &WorldNode| {
            let half = (n.name.len() as i32 * 3).max(8);
            n.x + half > rect.x && n.x - half < rect.x + rect.w && n.y + 18 > rect.y && n.y - 8 < rect.y + rect.h
        };
        let cost = |rect: &IRect| {
            let covered = self.nodes.iter().filter(|n| hides(rect, n)).count();
            covered + if hides(rect, cursor) { 100 } else { 0 }
        };
        corners.into_iter().min_by_key(cost).unwrap_or(corners[0])
    }

    fn draw_details(batch: &mut Batch, state: &GameState, node: &WorldNode, rect: IRect) {
        batch.draw_window(rect);
        let shadow = Some(palette::TEXT_SHADOW);
        let x = (rect.x + 8) as f32;
        font::draw(batch, &node.name, Vec2::new(x, (rect.y + 6) as f32), palette::HIGHLIGHT, shadow);
        let (mut chests, mut opened, mut rifts, mut closed, mut golden, mut cleared) = (0, 0, 0, 0, 0, 0);
        let done = |flags: &[String]| flags.iter().filter(|f| state.flags.contains(*f)).count();
        for map in &node.maps {
            let counts = counts_for(map);
            chests += counts.chests.len();
            opened += done(&counts.chests);
            rifts += counts.rifts.len();
            closed += done(&counts.rifts);
            golden += counts.golden.len();
            cleared += done(&counts.golden);
        }
        let mut y = (rect.y + 20) as f32;
        if let Some(town) = node.town.as_deref().filter(|t| data().towns.contains_key(*t)) {
            let text = format!("Restored {}%", (state.restoration(town) * 100.0).round());
            font::draw(batch, &text, Vec2::new(x, y), Rgba::rgb(230, 200, 255), shadow);
            y += 11.0;
        }
        if chests > 0 {
            font::draw(batch, &format!("Chests {opened}/{chests}"), Vec2::new(x, y), palette::TEXT, shadow);
            y += 11.0;
        }
        if rifts > 0 {
            font::draw(batch, &format!("Rifts sealed {closed}/{rifts}"), Vec2::new(x, y), palette::TEXT, shadow);
            y += 11.0;
        }
        if golden > 0 {
            font::draw(batch, &format!("Golden rifts {cleared}/{golden}"), Vec2::new(x, y), GOLDEN, shadow);
        }
    }
}

fn draw_dashed(batch: &mut Batch, a: Vec2, b: Vec2) {
    let length = a.distance(b);
    let dir = (b - a) / length.max(1.0);
    let mut t = 6.0;
    while t < length - 6.0 {
        batch.fill_rect(a.x + dir.x * t, a.y + dir.y * t, 2.0, 2.0, Rgba::rgb(120, 90, 60));
        t += 5.0;
    }
}

fn build_parchment() -> Canvas {
    let mut c = Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    for y in 0..c.height {
        for x in 0..c.width {
            let (fx, fy) = (x as f32, y as f32);
            let n = noise::value(fx / 30.0, fy / 30.0, 401) * 0.6 + noise::value(fx / 8.0, fy / 8.0, 403) * 0.4;
            let edge = x.min(c.width - x).min(y.min(c.height - y)) as f32 / 24.0;
            let mut color = Rgba::rgb(196, 170, 120).lerp(Rgba::rgb(230, 212, 168), n);
            if edge < 1.0 {
                color = Rgba::rgb(150, 115, 75).lerp(color, edge);
            }
            c.set(x, y, color);
        }
    }
    c
}

impl Scene for MapScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, ctx: &mut SceneCtx) {
        input::consume();
        if let Some(world) = ctx.world() {
            self.nodes = data().world.iter().filter(|n| visible(&world.state, n)).collect();
            let map_id = world.map_id();
            self.here = self.nodes.iter().copied().find(|n| n.maps.iter().any(|m| m == map_id));
        }
        self.cursor = self.here.or_else(|| self.nodes.first().copied());
        self.area = self.here.is_some_and(|n| matches!(n.kind.as_str(), "dungeon" | "side"));
        audio::sfx(Sound::MenuSelect);
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.time += dt;
        if input::pressed(GameAction::Cancel)
            || input::pressed(GameAction::Map)
            || (input::pressed(GameAction::Menu) && !input::pressed(GameAction::Confirm))
        {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        if input::pressed(GameAction::Confirm) {
            self.area = !self.area;
            audio::sfx(Sound::MenuMove);
        }
        if self.area {
            return;
        }
        let axis = |plus: GameAction, minus: GameAction| {
            if input::pressed(plus) {
                1.0
            } else if input::pressed(minus) {
                -1.0
            } else {
                0.0
            }
        };
        let step = Vec2::new(axis(GameAction::Right, GameAction::Left), axis(GameAction::Down, GameAction::Up));
        if step != Vec2::ZERO
            && let Some(cursor) = self.cursor
        {
            let from = Vec2::new(cursor.x as f32, cursor.y as f32);
            let next = self
                .nodes
                .iter()
                .copied()
                .filter(|n| !std::ptr::eq(*n, cursor))
                .map(|n| (n, Vec2::new(n.x as f32, n.y as f32) - from))
                .filter(|(_, to)| to.normalize_or_zero().dot(step) > 0.5)
                .min_by(|a, b| a.1.length().total_cmp(&b.1.length()))
                .map(|(n, _)| n);
            if let Some(next) = next {
                self.cursor = Some(next);
                audio::sfx(Sound::MenuMove);
            }
        }
    }

    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        batch.begin_screen();
        batch.draw(gfx::cached("map_parchment", build_parchment), Vec2::ZERO, Rgba::WHITE);
        let hint = format!(
            "{}: close   {}: world / area",
            input::glyph(GameAction::Cancel),
            input::glyph(GameAction::Confirm)
        );
        font::draw(batch, &hint, Vec2::new(14.0, (SCREEN_HEIGHT - 14) as f32), Rgba::rgb(110, 80, 50), None);
        if self.area {
            draw_area(batch, world, self.time);
            return;
        }
        font::draw(batch, "World Map", Vec2::new(14.0, 10.0), Rgba::rgb(80, 50, 30), None);

        let at = |n: &WorldNode| Vec2::new(n.x as f32, n.y as f32);
        for node in &self.nodes {
            for link in &node.links {
                if let Some(other) = self.nodes.iter().find(|n| &n.id == link) {
                    draw_dashed(batch, at(node), at(other));
                }
            }
        }

        let origin = Vec2::splat(16.0);
        let disc = |batch: &mut Batch, tex: gfx::TexId, pos: Vec2, color: Rgba, scale: f32| {
            batch.draw_ex(tex, pos, None, color, 0.0, origin, Vec2::splat(scale), false, false);
        };
        for &node in &self.nodes {
            let pos = at(node);
            let icon = match node.kind.as_str() {
                "town" => Rgba::rgb(200, 80, 60),
                "dungeon" => Rgba::rgb(90, 70, 130),
                "side" => Rgba::rgb(60, 130, 90),
                "blight" => Rgba::rgb(130, 40, 120),
                _ => Rgba::rgb(120, 100, 60),
            };
            disc(batch, combat::disc(), pos, Rgba::rgb(60, 40, 20), 0.38);
            disc(batch, combat::disc(), pos, icon, 0.3);
            if self.here.is_some_and(|h| std::ptr::eq(h, node)) && (self.time * 3.0) as i32 % 2 == 0 {
                disc(batch, combat::ring(), pos, Rgba::rgb(255, 230, 120), 0.55);
            }
            if self.cursor.is_some_and(|c| std::ptr::eq(c, node)) {
                disc(batch, combat::ring(), pos, Rgba::rgb(60, 30, 10), 0.7);
            }
            font::draw_centered(batch, &node.name, pos + Vec2::new(0.0, 8.0), Rgba::rgb(60, 36, 20), None);
        }

        if let Some(cursor) = self.cursor {
            MapScene::draw_details(batch, &world.state, cursor, self.details_rect(cursor));
        }
    }
}

/// The current area, a few pixels per tile, with the hero, exits, rifts, golden rifts, locked doors and unopened
/// chests marked.
fn draw_area(batch: &mut Batch, world: &World, time: f32) {
    let map = &world.map;
    font::draw(batch, world.map_name(), Vec2::new(14.0, 10.0), Rgba::rgb(80, 50, 30), None);
    let scale = (440 / map.width.max(1)).min(220 / map.height.max(1)).clamp(1, 6);
    let origin =
        Vec2::new(((SCREEN_WIDTH - map.width * scale) / 2) as f32, (26 + (220 - map.height * scale) / 2) as f32);
    let s = scale as f32;
    let tile = |x: i32, y: i32| origin + Vec2::new(x as f32 * s, y as f32 * s);
    for y in 0..map.height {
        for x in 0..map.width {
            let color = match map.at(x, y) {
                Terrain::Water | Terrain::Shallows => Rgba::rgb(120, 150, 170),
                Terrain::Pit => Rgba::rgb(60, 45, 35),
                Terrain::Forest => Rgba::rgb(110, 125, 80),
                t if t.blocks_walking() => Rgba::rgb(120, 95, 70),
                _ => Rgba::rgb(222, 204, 160),
            };
            let at = tile(x, y);
            batch.fill_rect(at.x, at.y, s, s, color);
        }
    }
    let dot = |batch: &mut Batch, x: f32, y: f32, color: Rgba| {
        let at = origin + Vec2::new(x * s, y * s) + Vec2::splat(s / 2.0);
        batch.fill_rect(at.x - 2.0, at.y - 2.0, 5.0, 5.0, Rgba::rgb(50, 30, 20));
        batch.fill_rect(at.x - 1.0, at.y - 1.0, 3.0, 3.0, color);
    };
    let flags = &world.state.flags;
    for e in &world.data.entities {
        let (x, y) = (e.x, e.y);
        match e.kind.as_str() {
            "warp" if world.should_exist(e) => dot(batch, x, y, Rgba::rgb(250, 250, 240)),
            "rift" => {
                let id = e.str("id").map(str::to_string).unwrap_or_else(|| format!("{}_{}", e.x, e.y));
                let sealed = flags.contains(&format!("rift:{id}"));
                dot(batch, x, y, if sealed { Rgba::rgb(150, 140, 150) } else { Rgba::rgb(190, 90, 240) });
            }
            "golden" => {
                let cleared = flags.contains(&format!("golden:{}", e.str_or("id", "golden")));
                dot(batch, x, y, if cleared { Rgba::rgb(150, 140, 150) } else { GOLDEN });
            }
            "trial" => {
                if let Some(done) = e.str("done") {
                    dot(batch, x, y, if flags.contains(done) { Rgba::rgb(150, 140, 150) } else { GOLDEN });
                }
            }
            "chest" => {
                let id = e
                    .str("flag")
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("chest:{}:{}_{}", world.map_id(), e.x, e.y));
                if !flags.contains(&id) && world.should_exist(e) {
                    dot(batch, x, y, Rgba::rgb(250, 210, 80));
                }
            }
            "door" => {
                let id = e
                    .str("flag")
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("door:{}:{}_{}", world.map_id(), e.x, e.y));
                if !flags.contains(&id) {
                    dot(batch, x, y, Rgba::rgb(170, 110, 60));
                }
            }
            _ => {}
        }
    }
    if (time * 3.0) as i32 % 2 == 0 {
        let hero = world.hero.pos / crate::world::map::TILE as f32;
        dot(batch, hero.x - 0.5, hero.y - 1.0, Rgba::rgb(230, 60, 50));
    }
    let legend = [
        ("You", Rgba::rgb(230, 60, 50)),
        ("Rift", Rgba::rgb(190, 90, 240)),
        ("Golden", GOLDEN),
        ("Chest", Rgba::rgb(250, 210, 80)),
        ("Door", Rgba::rgb(170, 110, 60)),
        ("Exit", Rgba::rgb(250, 250, 240)),
    ];
    let mut x = (SCREEN_WIDTH - 46 * legend.len() as i32) as f32;
    for (name, color) in legend {
        batch.fill_rect(x, 13.0, 4.0, 4.0, color);
        font::draw(batch, name, Vec2::new(x + 6.0, 10.0), Rgba::rgb(90, 60, 40), None);
        x += 46.0;
    }
}
