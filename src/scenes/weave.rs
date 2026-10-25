//! The Weave screen: the lattice of passives, panned around a cursor that walks the threads one cell at a time. The
//! mode keys jump between the keystones, the skill button zooms out to the whole Weave and Confirm takes the node under
//! the cursor. At a save crystal the whole Weave can be unpicked for free.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx, world_below};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::art::combat;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::{self, Batch};
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entities::combat_effects::draw_line;
use crate::gameplay::game_state::GameState;
use crate::gameplay::weave::{self, HUB, NODES, NodeKind, Region, WeaveNode};
use crate::scene_any;

const PANEL: IRect = IRect::new(10, SCREEN_HEIGHT - 52, SCREEN_WIDTH - 20, 44);
/// Where the cursor sits on screen; the Weave pans around it.
const FOCUS: Vec2 = Vec2::new(SCREEN_WIDTH as f32 / 2.0, (SCREEN_HEIGHT - 52) as f32 / 2.0 + 12.0);
const CELL: f32 = 20.0;
/// Cell size when zoomed out far enough to see the whole Weave.
const OVERVIEW_CELL: f32 = 9.0;

/// The keystones, clockwise from the north, in the order the mode keys cycle through them.
const JUMPS: [&str; 8] =
    ["overcharge", "glass_edge", "tailwind", "aether_bond", "stillwater", "bulwark", "blood_art", "unbroken"];

pub struct WeaveScene {
    cursor: &'static WeaveNode,
    camera: Vec2,
    cell: f32,
    overview: bool,
    at_crystal: bool,
    reset_armed: bool,
}

fn grid_pos(node: &WeaveNode) -> Vec2 {
    Vec2::new(node.x as f32, node.y as f32)
}

impl WeaveScene {
    pub fn new() -> WeaveScene {
        let hub = weave::by_id(HUB).expect("hub node");
        WeaveScene {
            cursor: hub,
            camera: grid_pos(hub),
            cell: CELL,
            overview: false,
            at_crystal: false,
            reset_armed: false,
        }
    }

    /// Opened from a save crystal, where the Weave can be unpicked for free.
    pub fn at_crystal() -> WeaveScene {
        WeaveScene { at_crystal: true, ..WeaveScene::new() }
    }

    /// The next node along the thread in a direction, hopping a single hole in the lattice.
    fn step(&self, dx: i32, dy: i32) -> Option<&'static WeaveNode> {
        let (x, y) = (self.cursor.x, self.cursor.y);
        weave::at(x + dx, y + dy).or_else(|| weave::at(x + dx * 2, y + dy * 2))
    }

    fn jump(&self, step: i32) -> Option<&'static WeaveNode> {
        let current = JUMPS.iter().position(|id| *id == self.cursor.id);
        let n = JUMPS.len() as i32;
        let next = match current {
            Some(i) => (i as i32 + step).rem_euclid(n),
            None if step > 0 => 0,
            None => n - 1,
        };
        weave::by_id(JUMPS[next as usize])
    }
}

fn node_radius(kind: NodeKind) -> f32 {
    match kind {
        NodeKind::Hub => 7.0,
        NodeKind::Minor => 3.0,
        NodeKind::Notable => 5.0,
        NodeKind::Keystone => 6.0,
    }
}

fn draw_disc(batch: &mut Batch, at: Vec2, radius: f32, color: Rgba) {
    let scale = Vec2::splat(radius / 16.0);
    batch.draw_ex(combat::disc(), snap(at), None, color, 0.0, Vec2::splat(16.0), scale, false, false);
}

fn draw_diamond(batch: &mut Batch, at: Vec2, size: f32, color: Rgba) {
    let quarter = std::f32::consts::FRAC_PI_4;
    batch.draw_ex(gfx::pixel(), snap(at), None, color, quarter, Vec2::splat(0.5), Vec2::splat(size), false, false);
}

impl Scene for WeaveScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        let follow = if self.overview { Vec2::ZERO } else { grid_pos(self.cursor) };
        self.camera = self.camera.lerp(follow, (dt * 12.0).min(1.0));
        let cell = if self.overview { OVERVIEW_CELL } else { CELL };
        self.cell += (cell - self.cell) * (dt * 12.0).min(1.0);
        if input::pressed(GameAction::Skill) {
            self.overview = !self.overview;
            audio::sfx(Sound::MenuSelect);
        }
        if input::pressed(GameAction::Cancel)
            || (input::pressed(GameAction::Menu) && !input::pressed(GameAction::Confirm))
        {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        let Some(world) = ctx.world() else { return };

        let moved = if input::pressed(GameAction::NextMode) {
            self.jump(1)
        } else if input::pressed(GameAction::PrevMode) {
            self.jump(-1)
        } else if input::pressed(GameAction::Left) {
            self.step(-1, 0)
        } else if input::pressed(GameAction::Right) {
            self.step(1, 0)
        } else if input::pressed(GameAction::Up) {
            self.step(0, -1)
        } else if input::pressed(GameAction::Down) {
            self.step(0, 1)
        } else {
            None
        };
        if let Some(next) = moved {
            self.cursor = next;
            self.reset_armed = false;
            audio::sfx(Sound::MenuMove);
        }

        if input::pressed(GameAction::Confirm) {
            if world.state.can_take(self.cursor) {
                world.state.take_node(&self.cursor.id);
                audio::sfx(Sound::LevelUp);
            } else {
                audio::sfx(Sound::MenuCancel);
            }
        }
        if self.at_crystal && input::pressed(GameAction::Throw) && !world.state.weave_nodes.is_empty() {
            if self.reset_armed {
                world.state.reset_weave();
                self.reset_armed = false;
                audio::sfx(Sound::Heal);
            } else {
                self.reset_armed = true;
                audio::sfx(Sound::MenuSelect);
            }
        }
    }

    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        let state = &world.state;
        batch.begin_screen();
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::rgb(8, 8, 20));
        let cell = self.cell;
        let zoom = cell / CELL;
        let screen = |n: &WeaveNode| FOCUS + (grid_pos(n) - self.camera) * cell;

        for node in NODES.iter() {
            for other in [weave::at(node.x + 1, node.y), weave::at(node.x, node.y + 1)].into_iter().flatten() {
                let lit = state.weave_has(&node.id) && state.weave_has(&other.id);
                let color = if lit {
                    let region = if node.region == Region::Hub { other.region } else { node.region };
                    region.color().mul(0.85)
                } else {
                    palette::TEXT_DIM.mul(0.3)
                };
                draw_line(batch, screen(node), screen(other), color, if lit { 2.0 } else { 1.0 });
            }
        }

        for node in NODES.iter() {
            let at = screen(node);
            let owned = state.weave_has(&node.id);
            let can = state.can_take(node);
            let reachable = weave::neighbours(node).any(|n| state.weave_has(&n.id));
            let radius = (node_radius(node.kind) * zoom).max(1.5);
            let glow = node.region.color();
            let fill = if owned {
                glow
            } else if can {
                glow.lerp(Rgba::BLACK, 0.45)
            } else if reachable {
                glow.lerp(Rgba::BLACK, 0.75)
            } else {
                glow.lerp(Rgba::rgb(20, 20, 34), 0.82)
            };
            let ring = if can { Rgba::WHITE } else { palette::OUTLINE };
            if node.kind == NodeKind::Keystone {
                draw_diamond(batch, at, radius * 2.0 + 2.0, ring);
                draw_diamond(batch, at, radius * 2.0, fill);
            } else {
                draw_disc(batch, at, radius + 1.0, ring);
                draw_disc(batch, at, radius, fill);
            }
            if std::ptr::eq(node, self.cursor) {
                let pulse = 2.0 + (world.time as f32 * 6.0).sin().abs() * 1.5;
                let r = radius + pulse + 1.0;
                batch.draw_rect(
                    IRect::new((at.x - r) as i32, (at.y - r) as i32, (r * 2.0) as i32 + 1, (r * 2.0) as i32 + 1),
                    palette::HIGHLIGHT,
                );
            }
        }

        let shadow = Some(palette::TEXT_SHADOW);
        let labels = [
            (Region::Might, Vec2::new(0.0, -9.6)),
            (Region::Ward, Vec2::new(0.0, 9.2)),
            (Region::Swiftness, Vec2::new(9.9, 0.0)),
            (Region::Spirit, Vec2::new(-9.9, 0.0)),
            (Region::Aether, Vec2::new(7.0, 6.6)),
        ];
        for (region, grid) in labels {
            let at = FOCUS + (grid - self.camera) * cell;
            let text = region.label();
            let pos = Vec2::new(at.x - font::measure(text) as f32 / 2.0, at.y - 4.0);
            font::draw(batch, text, snap(pos), region.color().mul(0.8), shadow);
        }

        font::draw(batch, "The Weave", Vec2::new(12.0, 8.0), palette::HIGHLIGHT, shadow);
        let points = format!("Points {}   Knots {}", state.weave_points(), state.knots_free());
        font::draw(
            batch,
            &points,
            Vec2::new((SCREEN_WIDTH - 12 - font::measure(&points)) as f32, 8.0),
            palette::HIGHLIGHT,
            shadow,
        );
        let mut hint = format!(
            "{}/{}: jump to keystone   {}: {}",
            input::glyph(GameAction::PrevMode),
            input::glyph(GameAction::NextMode),
            input::glyph(GameAction::Skill),
            if self.overview { "zoom in" } else { "overview" }
        );
        if self.at_crystal {
            hint.push_str(&format!("   {}: unpick all", input::glyph(GameAction::Throw)));
        }
        font::draw(batch, &hint, Vec2::new(12.0, 20.0), palette::TEXT_DIM, None);

        batch.draw_window(PANEL);
        let node = self.cursor;
        let (px, py) = ((PANEL.x + 8) as f32, PANEL.y as f32);
        let title_color = match node.kind {
            NodeKind::Keystone => palette::GOLD,
            NodeKind::Notable => palette::HIGHLIGHT,
            _ => palette::TEXT,
        };
        font::draw(batch, &node.name, Vec2::new(px, py + 5.0), title_color, shadow);
        for (i, line) in font::wrap(&node.description, PANEL.w - 16).iter().take(2).enumerate() {
            font::draw(batch, line, Vec2::new(px, py + 17.0 + i as f32 * 11.0), palette::TEXT, shadow);
        }
        let status = node_status(state, node, self.reset_armed);
        let status_pos = Vec2::new((PANEL.right() - 8 - font::measure(&status)) as f32, py + 5.0);
        font::draw(batch, &status, status_pos, palette::TEXT_DIM, None);
    }
}

fn node_status(state: &GameState, node: &WeaveNode, reset_armed: bool) -> String {
    if reset_armed {
        format!("Press {} again to refund every point", input::glyph(GameAction::Throw))
    } else if state.weave_has(&node.id) {
        "Woven".to_string()
    } else if !weave::neighbours(node).any(|n| state.weave_has(&n.id)) {
        "Weave a neighbouring node first".to_string()
    } else if node.needs_knot() && state.knots_free() <= 0 {
        "Needs a Weave Knot".to_string()
    } else if state.weave_points() < 1 {
        "No points left".to_string()
    } else if node.needs_knot() {
        "Confirm: weave (1 point, 1 knot)".to_string()
    } else {
        "Confirm: weave (1 point)".to_string()
    }
}
