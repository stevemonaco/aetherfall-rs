//! The Circuit: one skill tree per element plus a core tree, bought with Circuit Points from levelling.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx, world_below};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::art::combat;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entities::combat_effects::draw_line;
use crate::gameplay::circuit::{self, CircuitNode, NODES};
use crate::gameplay::element::{BASIC, Element};
use crate::scene_any;

const NODE_WIDTH: i32 = 92;
const NODE_HEIGHT: i32 = 26;
const LEFT: i32 = 34;
const TOP: i32 = 48;
const COLUMN_GAP: i32 = 104;
const ROW_GAP: i32 = 36;

pub struct CircuitScene {
    trees: Vec<Element>,
    tree: usize,
    cursor: Option<&'static CircuitNode>,
}

impl CircuitScene {
    pub fn new() -> CircuitScene {
        CircuitScene { trees: vec![Element::None], tree: 0, cursor: None }
    }
}

fn nodes_of(tree: Element) -> impl Iterator<Item = &'static CircuitNode> {
    NODES.iter().filter(move |n| n.tree == tree)
}

fn node_at(n: &CircuitNode) -> Vec2 {
    Vec2::new((LEFT + n.col * COLUMN_GAP) as f32, (TOP + n.row * ROW_GAP) as f32)
}

fn tree_label(tree: Element) -> &'static str {
    if tree == Element::None { "Core" } else { tree.label() }
}

impl Scene for CircuitScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, ctx: &mut SceneCtx) {
        input::consume();
        let modes = ctx.world().map(|w| w.state.available_modes()).unwrap_or_default();
        self.trees = std::iter::once(Element::None)
            .chain(BASIC.into_iter().chain([Element::Aether]).filter(|e| modes.contains(e)))
            .collect();
        self.cursor = nodes_of(self.trees[0]).next();
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        if input::pressed(GameAction::Cancel)
            || (input::pressed(GameAction::Menu) && !input::pressed(GameAction::Confirm))
        {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        let tab = if input::pressed(GameAction::NextMode) {
            1
        } else if input::pressed(GameAction::PrevMode) {
            -1
        } else {
            0
        };
        if tab != 0 {
            let count = self.trees.len() as i32;
            self.tree = ((self.tree as i32 + tab + count) % count) as usize;
            self.cursor = nodes_of(self.trees[self.tree]).next();
            audio::sfx(Sound::MenuMove);
            return;
        }

        let axis = |plus: GameAction, minus: GameAction| {
            if input::pressed(plus) {
                1
            } else if input::pressed(minus) {
                -1
            } else {
                0
            }
        };
        let (mx, my) = (axis(GameAction::Right, GameAction::Left), axis(GameAction::Down, GameAction::Up));
        if (mx, my) != (0, 0)
            && let Some(from) = self.cursor
        {
            let tree = self.trees[self.tree];
            let others = || nodes_of(tree).filter(move |n| !std::ptr::eq(*n, from));
            let distance = |n: &CircuitNode, row_weight: f32| {
                (n.col - from.col).abs() as f32 + (n.row - from.row).abs() as f32 * row_weight
            };
            let next = others()
                .filter(|n| {
                    (n.col - from.col).signum() == mx
                        && if my == 0 { n.row == from.row } else { (n.row - from.row).signum() == my }
                })
                .min_by(|a, b| distance(a, 1.5).total_cmp(&distance(b, 1.5)))
                .or_else(|| {
                    others()
                        .filter(|n| {
                            if mx == 0 { (n.row - from.row).signum() == my } else { (n.col - from.col).signum() == mx }
                        })
                        .min_by(|a, b| distance(a, 1.0).total_cmp(&distance(b, 1.0)))
                });
            if let Some(next) = next {
                self.cursor = Some(next);
                audio::sfx(Sound::MenuMove);
            }
        }

        if input::pressed(GameAction::Confirm)
            && let (Some(cursor), Some(world)) = (self.cursor, ctx.world())
        {
            if world.state.can_unlock(cursor) {
                world.state.unlock(&cursor.id);
                audio::sfx(Sound::LevelUp);
            } else {
                audio::sfx(Sound::MenuCancel);
            }
        }
    }

    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        let state = &world.state;
        batch.begin_screen();
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::rgb(8, 8, 20).mul(0.92));
        let tree = self.trees[self.tree];
        let shadow = Some(palette::TEXT_SHADOW);

        let mut tx = 16.0;
        for (i, &t) in self.trees.iter().enumerate() {
            let selected = i == self.tree;
            let tint = if selected { Rgba::WHITE } else { Rgba::WHITE.mul(0.4) };
            let scale = Vec2::splat(0.85);
            batch.draw_ex(combat::emblem(t), Vec2::new(tx, 10.0), None, tint, 0.0, Vec2::ZERO, scale, false, false);
            if selected {
                font::draw(batch, tree_label(t), Vec2::new(tx + 18.0, 14.0), t.light(), shadow);
                tx += (24 + font::measure(tree_label(t))) as f32;
            } else {
                tx += 20.0;
            }
        }
        let points = format!("CP {}   Shards {}", state.circuit_points(), state.shards_free());
        let points_pos = Vec2::new((SCREEN_WIDTH - 14 - font::measure(&points)) as f32, 14.0);
        font::draw(batch, &points, points_pos, palette::HIGHLIGHT, shadow);
        let hint =
            format!("{} / {}: switch tree", input::glyph(GameAction::PrevMode), input::glyph(GameAction::NextMode));
        font::draw(batch, &hint, Vec2::new(16.0, 30.0), palette::TEXT_DIM, None);

        let glow = if tree == Element::None { Rgba::rgb(200, 210, 240) } else { tree.glow() };
        for node in nodes_of(tree) {
            for req in &node.requires {
                if let Some(parent) = circuit::by_id(req) {
                    let a = node_at(parent) + Vec2::new(NODE_WIDTH as f32 / 2.0, NODE_HEIGHT as f32);
                    let b = node_at(node) + Vec2::new(NODE_WIDTH as f32 / 2.0, 0.0);
                    let lit = state.circuit_nodes.contains(req);
                    let (color, width) = if lit { (glow.mul(0.8), 2.0) } else { (palette::TEXT_DIM.mul(0.3), 1.0) };
                    draw_line(batch, a, b, color, width);
                }
            }
        }

        for node in nodes_of(tree) {
            let at = node_at(node);
            let owned = state.circuit_nodes.contains(&node.id);
            let can = state.can_unlock(node);
            let rect = IRect::new(at.x as i32, at.y as i32, NODE_WIDTH, NODE_HEIGHT);
            batch.fill(rect, if owned { glow.lerp(Rgba::BLACK, 0.6) } else { Rgba::rgb(24, 24, 44) });
            let border = if owned {
                glow
            } else if can {
                Rgba::WHITE
            } else {
                palette::TEXT_DIM.mul(0.5)
            };
            batch.draw_rect(rect, border);
            if self.cursor.is_some_and(|c| std::ptr::eq(c, node)) {
                batch.draw_rect(IRect::new(rect.x - 2, rect.y - 2, rect.w + 4, rect.h + 4), palette::HIGHLIGHT);
            }
            let text_color = if owned {
                Rgba::WHITE
            } else if can {
                palette::TEXT
            } else {
                palette::TEXT_DIM
            };
            for (i, line) in font::wrap(&node.name, NODE_WIDTH - 8).iter().take(2).enumerate() {
                font::draw(
                    batch,
                    line,
                    Vec2::new((rect.x + 4) as f32, (rect.y + 3 + i as i32 * 10) as f32),
                    text_color,
                    None,
                );
            }
            let cost = if owned {
                String::new()
            } else if node.shard {
                format!("{}+S", node.cost)
            } else {
                node.cost.to_string()
            };
            let cost_pos = Vec2::new((rect.right() - 3 - font::measure(&cost)) as f32, (rect.bottom() - 10) as f32);
            font::draw(batch, &cost, cost_pos, palette::HIGHLIGHT.mul(0.8), None);
        }

        if let Some(cursor) = self.cursor {
            let panel = IRect::new(14, SCREEN_HEIGHT - 44, SCREEN_WIDTH - 28, 36);
            batch.draw_window(panel);
            let (px, py) = ((panel.x + 8) as f32, panel.y as f32);
            font::draw(batch, &cursor.name, Vec2::new(px, py + 5.0), palette::HIGHLIGHT, shadow);
            font::draw(batch, &cursor.description, Vec2::new(px, py + 18.0), palette::TEXT, shadow);
            let status = if state.circuit_nodes.contains(&cursor.id) {
                "Learned".to_string()
            } else if cursor.requires.iter().any(|r| !state.circuit_nodes.contains(r)) {
                "Requires the node above".to_string()
            } else if cursor.shard && state.shards_free() <= 0 {
                "Needs a Circuit Shard".to_string()
            } else if state.circuit_points() < cursor.cost {
                "Not enough CP".to_string()
            } else {
                format!("Confirm to learn ({} CP)", cursor.cost)
            };
            let status_pos = Vec2::new((panel.right() - 8 - font::measure(&status)) as f32, py + 5.0);
            font::draw(batch, &status, status_pos, palette::TEXT_DIM, None);
        }
    }
}
