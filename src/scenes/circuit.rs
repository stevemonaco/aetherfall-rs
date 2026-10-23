//! The Circuit screen: a pannable web of passives. The cursor walks between nodes with the direction keys, the mode
//! keys jump between arms, and Confirm learns the node under the cursor. At a save crystal the whole web can be reset.

use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx, world_below};
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::art::combat;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entities::combat_effects::draw_line;
use crate::gameplay::circuit::{self, CircuitNode, HUB, NODES, NodeKind};
use crate::gameplay::element::{Element, MODES};
use crate::gameplay::game_state::GameState;
use crate::scene_any;

const PANEL: IRect = IRect::new(10, SCREEN_HEIGHT - 52, SCREEN_WIDTH - 20, 44);
/// Where the cursor sits on screen; the web pans around it.
const FOCUS: Vec2 = Vec2::new(SCREEN_WIDTH as f32 / 2.0, (SCREEN_HEIGHT - 52) as f32 / 2.0 + 12.0);

pub struct CircuitScene {
    cursor: &'static CircuitNode,
    camera: Vec2,
    at_crystal: bool,
    reset_armed: bool,
}

impl CircuitScene {
    pub fn new() -> CircuitScene {
        let hub = circuit::by_id(HUB).expect("hub node");
        CircuitScene { cursor: hub, camera: hub.pos, at_crystal: false, reset_armed: false }
    }

    /// Opened from a save crystal, where the web can be reset for free.
    pub fn at_crystal() -> CircuitScene {
        CircuitScene { at_crystal: true, ..CircuitScene::new() }
    }

    /// The closest node roughly in `dir` from the cursor; linked nodes are preferred so paths are easy to follow.
    fn step(&self, dir: Vec2) -> Option<&'static CircuitNode> {
        let from = self.cursor;
        NODES
            .iter()
            .filter(|n| n.id != from.id)
            .filter_map(|n| {
                let to = n.pos - from.pos;
                let distance = to.length();
                let alignment = to.normalize_or_zero().dot(dir);
                if alignment < 0.45 {
                    return None;
                }
                let linked = if from.links.contains(&n.id) { 0.6 } else { 1.0 };
                Some((n, distance * (2.0 - alignment) * linked))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(n, _)| n)
    }

    /// The first node of the next or previous open arm.
    fn jump_arm(&self, state: &GameState, step: i32) -> Option<&'static CircuitNode> {
        let open: Vec<Element> = MODES.into_iter().filter(|&m| state.circuit_region_open(m)).collect();
        let current = if self.cursor.id == HUB { None } else { open.iter().position(|&m| m == self.cursor.region) };
        let n = open.len() as i32;
        let next = match current {
            Some(i) => (i as i32 + step).rem_euclid(n),
            None if step > 0 => 0,
            None => n - 1,
        };
        let region = open[next as usize];
        NODES.iter().find(|node| node.region == region && node.links.iter().any(|l| l == HUB))
    }
}

fn region_color(region: Element) -> Rgba {
    if region == Element::None { Rgba::rgb(200, 210, 240) } else { region.glow() }
}

fn node_radius(kind: NodeKind) -> f32 {
    match kind {
        NodeKind::Hub => 7.0,
        NodeKind::Small => 3.0,
        NodeKind::Notable => 5.0,
        NodeKind::Keystone => 6.0,
    }
}

fn draw_disc(batch: &mut Batch, at: Vec2, radius: f32, color: Rgba) {
    let scale = Vec2::splat(radius / 16.0);
    batch.draw_ex(combat::disc(), snap(at), None, color, 0.0, Vec2::splat(16.0), scale, false, false);
}

impl Scene for CircuitScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.camera = self.camera.lerp(self.cursor.pos, (dt * 12.0).min(1.0));
        if input::pressed(GameAction::Cancel)
            || (input::pressed(GameAction::Menu) && !input::pressed(GameAction::Confirm))
        {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }
        let Some(world) = ctx.world() else { return };

        let mut dir = Vec2::ZERO;
        if input::pressed(GameAction::Left) {
            dir.x -= 1.0;
        }
        if input::pressed(GameAction::Right) {
            dir.x += 1.0;
        }
        if input::pressed(GameAction::Up) {
            dir.y -= 1.0;
        }
        if input::pressed(GameAction::Down) {
            dir.y += 1.0;
        }
        let jump = if input::pressed(GameAction::NextMode) {
            1
        } else if input::pressed(GameAction::PrevMode) {
            -1
        } else {
            0
        };
        let moved = if jump != 0 {
            self.jump_arm(&world.state, jump)
        } else if dir != Vec2::ZERO {
            self.step(dir.normalize())
        } else {
            None
        };
        if let Some(next) = moved {
            self.cursor = next;
            self.reset_armed = false;
            audio::sfx(Sound::MenuMove);
        }

        if input::pressed(GameAction::Confirm) {
            if world.state.can_unlock(self.cursor) {
                world.state.unlock(&self.cursor.id);
                audio::sfx(Sound::LevelUp);
            } else {
                audio::sfx(Sound::MenuCancel);
            }
        }
        if self.at_crystal && input::pressed(GameAction::Throw) && !world.state.circuit_nodes.is_empty() {
            if self.reset_armed {
                world.state.reset_circuit();
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
        let offset = FOCUS - self.camera;
        let screen = |n: &CircuitNode| n.pos + offset;

        for node in NODES.iter() {
            for link in node.links.iter().filter(|l| node.id < **l) {
                let Some(other) = circuit::by_id(link) else { continue };
                let region = if node.id == HUB { other.region } else { node.region };
                let open = state.circuit_region_open(region);
                let lit = state.circuit_has(&node.id) && state.circuit_has(&other.id);
                let color = if lit {
                    region_color(region).mul(0.9)
                } else if open {
                    palette::TEXT_DIM.mul(0.45)
                } else {
                    palette::TEXT_DIM.mul(0.15)
                };
                draw_line(batch, screen(node), screen(other), color, if lit { 2.0 } else { 1.0 });
            }
        }

        for node in NODES.iter() {
            let at = screen(node);
            let owned = state.circuit_has(&node.id);
            let can = state.can_unlock(node);
            let open = state.circuit_region_open(node.region);
            let radius = node_radius(node.kind);
            let glow = region_color(node.region);
            let fill = if owned {
                glow
            } else if can {
                glow.lerp(Rgba::BLACK, 0.45)
            } else if open {
                Rgba::rgb(40, 40, 64)
            } else {
                Rgba::rgb(22, 22, 34)
            };
            let ring = if can { Rgba::WHITE } else { palette::OUTLINE };
            if node.kind == NodeKind::Keystone {
                let size = radius + 1.0;
                batch.draw_ex(
                    crate::core::gfx::pixel(),
                    snap(at),
                    None,
                    ring,
                    std::f32::consts::FRAC_PI_4,
                    Vec2::splat(0.5),
                    Vec2::splat(size * 2.0),
                    false,
                    false,
                );
                batch.draw_ex(
                    crate::core::gfx::pixel(),
                    snap(at),
                    None,
                    fill,
                    std::f32::consts::FRAC_PI_4,
                    Vec2::splat(0.5),
                    Vec2::splat(size * 2.0 - 2.0),
                    false,
                    false,
                );
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

        for region in MODES {
            let angle = circuit::arm_angle(region);
            let dir = Vec2::new(angle.cos(), -angle.sin());
            let reach = NODES.iter().filter(|n| n.region == region).map(|n| n.pos.dot(dir)).fold(0.0, f32::max);
            let at = dir * (reach + 22.0) + offset;
            let color =
                if state.circuit_region_open(region) { region_color(region) } else { palette::TEXT_DIM.mul(0.4) };
            batch.draw_ex(
                combat::emblem(region),
                snap(at - Vec2::splat(6.0)),
                None,
                color,
                0.0,
                Vec2::ZERO,
                Vec2::splat(0.7),
                false,
                false,
            );
        }

        let shadow = Some(palette::TEXT_SHADOW);
        font::draw(batch, "The Circuit", Vec2::new(12.0, 8.0), palette::HIGHLIGHT, shadow);
        let points = format!("Points {}   Shards {}", state.circuit_points(), state.shards_free());
        font::draw(
            batch,
            &points,
            Vec2::new((SCREEN_WIDTH - 12 - font::measure(&points)) as f32, 8.0),
            palette::HIGHLIGHT,
            shadow,
        );
        let mut hint =
            format!("{}/{}: jump to arm", input::glyph(GameAction::PrevMode), input::glyph(GameAction::NextMode));
        if self.at_crystal {
            hint.push_str(&format!("   {}: reset all", input::glyph(GameAction::Throw)));
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
        let status = if self.reset_armed {
            format!("Press {} again to refund every point", input::glyph(GameAction::Throw))
        } else if state.circuit_has(&node.id) {
            "Learned".to_string()
        } else if !state.circuit_region_open(node.region) {
            match node.region {
                Element::Aether => "Opens when the blade awakens".to_string(),
                e => format!("Needs the Sigil of {}", e.name()),
            }
        } else if !node.links.iter().any(|l| state.circuit_has(l)) {
            "Learn a linked node first".to_string()
        } else if node.needs_shard() && state.shards_free() <= 0 {
            "Needs a Circuit Shard".to_string()
        } else if state.circuit_points() < 1 {
            "No points left".to_string()
        } else if node.needs_shard() {
            "Confirm: learn (1 point, 1 shard)".to_string()
        } else {
            "Confirm: learn (1 point)".to_string()
        };
        let status_pos = Vec2::new((PANEL.right() - 8 - font::measure(&status)) as f32, py + 5.0);
        font::draw(batch, &status, status_pos, palette::TEXT_DIM, None);
    }
}
