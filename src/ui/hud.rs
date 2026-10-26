//! The combat HUD: vitals and mode in the top-left, rank and notices top-right, boss bar at the bottom.

use bevy::math::Vec2;

use crate::art::combat as art;
use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::entities::companion::Companion;
use crate::entities::enemy::Enemy;
use crate::entities::golden_breach::GoldenBreach;
use crate::entities::player::Player;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::data;
use crate::gameplay::game_state::GameState;
use crate::gameplay::rank::CombatRank;
use crate::scenes::world::{SCREEN_HEIGHT, SCREEN_WIDTH, World};
use crate::world::map::TILE;

const BACK: Rgba = Rgba::rgb(10, 10, 22);

pub fn draw(batch: &mut Batch, world: &World) {
    let Some(player) = world.player.as_deref() else { return };
    let state = &world.state;
    draw_vitals(batch, world, state, player);
    draw_soul(batch, world);
    draw_quest(batch, state);
    draw_rank(batch, &world.rank);
    draw_toasts(batch, world);
    font::draw(
        batch,
        &format!("{} G", state.gold),
        Vec2::new(8.0, (SCREEN_HEIGHT - 14) as f32),
        palette::GOLD,
        Some(palette::TEXT_SHADOW),
    );
    if let Some(boss) = world.boss.and_then(|id| world.get_as::<Enemy>(id)) {
        draw_boss_bar(batch, boss);
    } else if let Some(champion) = world.entities_of::<Enemy>().find(|e| e.is_champion() && e.hp > 0) {
        draw_boss_bar(batch, champion);
    }
    draw_trial_mods(batch, world);
    if input::mouse_aiming() && !input::scripted() {
        draw_cursor(batch, state.mode);
    }
    if let Some(pick) = player.wheel() {
        draw_wheel(batch, state, pick, player.center() - world.camera.position);
    }
}

/// The quick-change wheel: one emblem per direction, the owned ones lit and the highlighted one ringed.
fn draw_wheel(batch: &mut Batch, state: &GameState, pick: Option<Element>, center: Vec2) {
    let disc = Vec2::splat(16.0);
    batch.draw_ex(art::disc(), center, None, BACK.mul(0.6), 0.0, disc, Vec2::splat(2.6), false, false);
    for (element, dir) in [
        (Element::Wind, Vec2::new(0.0, -1.0)),
        (Element::Fire, Vec2::new(1.0, 0.0)),
        (Element::Water, Vec2::new(0.0, 1.0)),
        (Element::Thunder, Vec2::new(-1.0, 0.0)),
    ] {
        let at = center + dir * 26.0;
        let owned = state.has_sigil(element);
        if pick == Some(element) {
            batch.draw_ex(art::disc(), at, None, Rgba::WHITE, 0.0, disc, Vec2::splat(0.68), false, false);
        }
        let tint = if !owned {
            Rgba::WHITE.mul(0.15)
        } else if pick == Some(element) {
            Rgba::WHITE
        } else {
            Rgba::WHITE.mul(0.6)
        };
        batch.draw(art::emblem(element), snap(at - Vec2::splat(9.0)), tint);
    }
}

fn blink(world: &World, period: f64) -> bool {
    (world.time / period) as i64 % 2 == 0
}

fn draw_vitals(batch: &mut Batch, world: &World, state: &GameState, player: &Player) {
    let (x, y) = (8, 8);
    let mode = state.mode;
    batch.fill(IRect::new(x + 9, y + 1, 150, 30), BACK.mul(0.55));
    let emblem_tint = if state.dormant() { Rgba::GRAY } else { Rgba::WHITE };
    batch.draw(art::emblem(mode), Vec2::new(x as f32, y as f32), emblem_tint);
    if !state.dormant() {
        let color = if player.aether_active() {
            if blink(world, 0.15) { Rgba::WHITE } else { Element::Aether.glow() }
        } else {
            Element::Aether.light()
        };
        let height = (16.0 * player.aether_meter()).round() as i32;
        batch.fill(IRect::new(x - 5, y, 4, 18), palette::OUTLINE);
        batch.fill(IRect::new(x - 4, y + 1, 2, 16), Element::Aether.dark());
        batch.fill(IRect::new(x - 4, y + 17 - height, 2, height), color);
        if !player.aether_active() {
            let locked = (16.0 * player.aether_locked()).round() as i32;
            batch.fill(IRect::new(x - 4, y + 17 - locked, 2, locked), Element::Aether.glow());
        }
    }

    let bx = x + 22;
    bar(batch, bx, y + 2, 118, 6, state.hp as f32 / state.max_hp() as f32, palette::LIFE);
    font::draw(
        batch,
        &state.hp.to_string(),
        Vec2::new((bx + 121) as f32, y as f32),
        palette::TEXT,
        Some(palette::TEXT_SHADOW),
    );

    // SP pips fill one after another.
    let sp = state.sp / 100.0;
    for i in 0..state.sp_pips() {
        let fill = (sp - i as f32).clamp(0.0, 1.0);
        let rect = IRect::new(bx + i * 15, y + 11, 13, 4);
        batch.fill(IRect::new(rect.x - 1, rect.y - 1, rect.w + 2, rect.h + 2), palette::OUTLINE);
        batch.fill(rect, Rgba::rgb(40, 46, 80));
        let color = if fill >= 1.0 { Rgba::rgb(120, 220, 255) } else { Rgba::rgb(70, 130, 190) };
        batch.fill(IRect::new(rect.x, rect.y, (rect.w as f32 * fill) as i32, rect.h), color);
        if fill >= 1.0 {
            batch.fill(IRect::new(rect.x, rect.y, rect.w, 1), Rgba::rgb(220, 250, 255));
        }
    }

    if state.corruption > 0.0 {
        let text = format!("BLIGHT -{}%", state.corruption as i32);
        font::draw(
            batch,
            &text,
            Vec2::new((bx + 62) as f32, (y + 37) as f32),
            Rgba::rgb(200, 130, 255),
            Some(palette::TEXT_SHADOW),
        );
    }
    if player.heat > 0.0 {
        let mut hot = Rgba::rgb(255, 200, 80).lerp(Rgba::rgb(255, 60, 30), player.heat / 100.0);
        if player.heat >= 100.0 && blink(world, 0.15) {
            hot = Rgba::WHITE;
        }
        font::draw(batch, "HEAT", Vec2::new((bx + 62) as f32, (y + 27) as f32), hot, Some(palette::TEXT_SHADOW));
        bar(batch, bx + 86, y + 30, 32, 2, player.heat / 100.0, hot);
    }
    if player.aether_active() {
        let at = Vec2::new((bx + 62) as f32, (y + 17) as f32);
        font::draw(batch, "AETHER", at, Element::Aether.light(), Some(palette::TEXT_SHADOW));
    }

    font::draw(
        batch,
        &format!("LV {}", state.level),
        Vec2::new((bx + 98) as f32, (y + 9) as f32),
        palette::TEXT_DIM,
        Some(palette::TEXT_SHADOW),
    );
    if state.exp_to_next() > 0 {
        bar(batch, bx, y + 25, 118, 1, state.exp as f32 / state.exp_to_next() as f32, palette::EXPERIENCE);
    }

    if player.mode_flash() > 0.0 {
        let mut mx = x;
        for m in state.owned_modes() {
            let tint = if m == mode { Rgba::WHITE } else { Rgba::WHITE.mul(0.4) };
            batch.draw_ex(
                art::emblem(m),
                Vec2::new(mx as f32, (y + 32) as f32),
                None,
                tint,
                0.0,
                Vec2::ZERO,
                Vec2::splat(0.75),
                false,
                false,
            );
            mx += 15;
        }
    }
    draw_skill_hint(batch, state, player, mode, bx, y + 28);
}

/// The guardian spirit's form beside the vitals, with a bar that fills toward its next attack.
fn draw_soul(batch: &mut Batch, world: &World) {
    let Some(companion) = world.companion.and_then(|id| world.get_as::<Companion>(id)) else { return };
    let at = Vec2::new(186.0, 10.0);
    batch.draw_ex(
        art::disc(),
        at + Vec2::splat(7.0),
        None,
        BACK.mul(0.7),
        0.0,
        Vec2::splat(16.0),
        Vec2::splat(0.5),
        false,
        false,
    );
    let soul = companion.soul();
    let (icon, src) = crate::entities::companion::form_icon(soul.id, soul.glow);
    let scale = 14.0 / src.w.max(src.h) as f32;
    batch.draw_ex(icon, at, Some(src), Rgba::WHITE, 0.0, Vec2::ZERO, Vec2::splat(scale), false, false);
    bar(batch, at.x as i32, at.y as i32 + 16, 13, 1, companion.readiness(world), soul.glow);
    if !world.state.souls_owned.is_empty() {
        font::draw(
            batch,
            &input::glyph(GameAction::Soul),
            at + Vec2::new(14.0, 4.0),
            palette::TEXT_DIM,
            Some(palette::TEXT_SHADOW),
        );
    }
}
fn draw_quest(batch: &mut Batch, state: &GameState) {
    let Some(quest) = state.tracked_quest.as_ref().and_then(|id| data().quests.get(id)) else { return };
    let Some(stage) = quest.stages.iter().find(|s| !state.check(s.done.as_deref())) else { return };
    let lines = font::wrap(&stage.text, 150);
    let y = SCREEN_HEIGHT - 30 - lines.len() as i32 * 10;
    font::draw(batch, &quest.title, Vec2::new(8.0, y as f32), palette::HIGHLIGHT.mul(0.9), Some(palette::TEXT_SHADOW));
    for (i, line) in lines.iter().enumerate() {
        font::draw(
            batch,
            line,
            Vec2::new(8.0, (y + 10 + i as i32 * 10) as f32),
            palette::TEXT.mul(0.85),
            Some(palette::TEXT_SHADOW),
        );
    }
}

/// The equipped skill's name, with one pip per unlocked level: lit when affordable, bright while charged.
fn draw_skill_hint(batch: &mut Batch, state: &GameState, player: &Player, mode: Element, x: i32, y: i32) {
    let Some(skill) = state.equipped_skill(mode) else { return };
    let levels = state.skill_level(mode);
    let charged = player.skill_charge_level();
    for i in 0..levels {
        let rect = IRect::new(x + i * 5, y + 2, 3, 3);
        let affordable = state.sp >= (i + 1) as f32 * 100.0;
        let color = if i < charged {
            Rgba::WHITE
        } else if affordable {
            mode.glow()
        } else {
            mode.dark()
        };
        batch.fill(rect.inflate(1, 1), palette::OUTLINE);
        batch.fill(rect, color);
    }
    let color = if state.sp >= 100.0 { mode.light() } else { palette::TEXT_DIM };
    let label = format!("{} {}", input::glyph(GameAction::Skill), skill.name);
    font::draw(batch, &label, Vec2::new((x + levels * 5 + 3) as f32, y as f32), color, Some(palette::TEXT_SHADOW));
}

pub fn bar(batch: &mut Batch, x: i32, y: i32, width: i32, height: i32, fraction: f32, color: Rgba) {
    let fraction = fraction.clamp(0.0, 1.0);
    batch.fill(IRect::new(x - 1, y - 1, width + 2, height + 2), palette::OUTLINE);
    batch.fill(IRect::new(x, y, width, height), color.lerp(palette::OUTLINE, 0.75));
    let fill = (width as f32 * fraction) as i32;
    batch.fill(IRect::new(x, y, fill, height), color);
    if height > 2 {
        batch.fill(IRect::new(x, y, fill, 1), color.lerp(Rgba::WHITE, 0.45));
        batch.fill(IRect::new(x, y + height - 1, fill, 1), color.lerp(palette::OUTLINE, 0.35));
    }
}

fn draw_rank(batch: &mut Batch, rank: &CombatRank) {
    if rank.rank == 0 {
        return;
    }
    let (x, y) = (SCREEN_WIDTH - 58, 8);
    let color = if rank.rank >= 5 {
        palette::HIGHLIGHT
    } else if rank.rank >= 3 {
        Rgba::rgb(150, 230, 255)
    } else {
        palette::TEXT
    };
    let pop = if rank.flash > 0.0 { 1.0 + rank.flash } else { 1.0 };
    let scale = if pop > 1.2 { 3 } else { 2 };
    font::draw_scaled(batch, rank.name(), Vec2::new((x + 1) as f32, (y + 1) as f32), palette::TEXT_SHADOW, scale);
    font::draw_scaled(batch, rank.name(), Vec2::new(x as f32, y as f32), color, scale);
    let multiplier = format!("x{}", trim_float(rank.multiplier()));
    font::draw(batch, &multiplier, Vec2::new(x as f32, (y + 18) as f32), palette::TEXT_DIM, Some(palette::TEXT_SHADOW));
    bar(batch, x, y + 28, 48, 2, rank.progress(), color);
    bar(batch, x, y + 32, 48, 1, rank.timer() / 10.0, palette::TEXT_DIM);
}

/// Formats like C#'s "0.0#": at least one and at most two decimals.
fn trim_float(v: f32) -> String {
    let s = format!("{v:.2}");
    if s.ends_with('0') { s[..s.len() - 1].to_string() } else { s }
}

fn draw_toasts(batch: &mut Batch, world: &World) {
    let mut y = if world.rank.rank > 0 { 48 } else { 10 };
    for (text, time) in world.toasts.iter().take(5) {
        let alpha = (time * 3.0).min(1.0) * ((2.6 - time) * 8.0).min(1.0);
        let width = font::measure(text) + 10;
        let rect = IRect::new(SCREEN_WIDTH - width - 8, y, width, 13);
        batch.fill(rect, BACK.mul(0.7 * alpha));
        batch.fill(IRect::new(rect.x, rect.y, 2, rect.h), palette::HIGHLIGHT.mul(alpha));
        font::draw(
            batch,
            text,
            Vec2::new((rect.x + 6) as f32, (rect.y + 2) as f32),
            palette::TEXT.mul(alpha),
            Some(palette::TEXT_SHADOW.mul(alpha)),
        );
        y += 15;
    }
}

pub fn draw_boss_bar(batch: &mut Batch, boss: &Enemy) {
    let width = 300;
    let (x, y) = ((SCREEN_WIDTH - width) / 2, SCREEN_HEIGHT - 20);
    let name = match &boss.boss {
        Some(core) => core.display_name(boss.def),
        None if boss.is_champion() => boss.champion_title(),
        None => boss.def.name.clone(),
    };
    font::draw(batch, &name, Vec2::new(x as f32, (y - 11) as f32), palette::TEXT, Some(palette::TEXT_SHADOW));
    batch.fill(IRect::new(x - 1, y - 1, width + 2, 7), palette::OUTLINE);
    batch.fill(IRect::new(x, y, width, 5), Rgba::rgb(70, 20, 20));
    let fill = (width as f32 * boss.health_fraction()) as i32;
    batch.fill(IRect::new(x, y, fill, 5), Rgba::rgb(255, 110, 50));
    batch.fill(IRect::new(x, y, fill, 1), Rgba::rgb(255, 200, 120));
    if boss.guard_max > 0.0 {
        batch.fill(IRect::new(x - 1, y + 6, width / 2 + 2, 3), palette::OUTLINE);
        let color = if boss.is_broken() { palette::HIGHLIGHT } else { Rgba::rgb(170, 190, 220) };
        batch.fill(IRect::new(x, y + 7, ((width / 2) as f32 * boss.guard_value / boss.guard_max) as i32, 1), color);
    }
}

/// The Trial Rift's mods, centred under the top edge.
fn draw_trial_mods(batch: &mut Batch, world: &World) {
    if world.trial_mods.is_empty() {
        return;
    }
    let names: Vec<&str> = world.trial_mods.iter().map(|m| m.label()).collect();
    let text = names.join("  /  ");
    let centered = |text: &str, y: f32| Vec2::new(((SCREEN_WIDTH - font::measure(text)) / 2) as f32, y);
    font::draw(batch, &text, centered(&text, 4.0), Rgba::rgb(255, 214, 110), Some(palette::TEXT_SHADOW));
    let Some(breach) = world.entities_of::<GoldenBreach>().next() else { return };
    if let Some((wave, waves, left)) = breach.progress() {
        let line = format!("Wave {wave}/{waves}  -  {left} left");
        font::draw(batch, &line, centered(&line, 14.0), palette::TEXT, Some(palette::TEXT_SHADOW));
    }
    if let Some(seconds) = breach.collapse_in() {
        let line = format!("Outside the breach: losing life. Waves reset in {:.1}s", seconds.max(0.0));
        font::draw(batch, &line, centered(&line, 24.0), Rgba::rgb(255, 110, 90), Some(palette::TEXT_SHADOW));
    }
}

fn draw_cursor(batch: &mut Batch, mode: Element) {
    let at = snap(input::mouse_screen());
    let color = mode.light();
    batch.fill_rect(at.x - 4.0, at.y, 3.0, 1.0, color);
    batch.fill_rect(at.x + 2.0, at.y, 3.0, 1.0, color);
    batch.fill_rect(at.x, at.y - 4.0, 1.0, 3.0, color);
    batch.fill_rect(at.x, at.y + 2.0, 1.0, 3.0, color);
    batch.fill_rect(at.x, at.y, 1.0, 1.0, Rgba::WHITE);
}

/// World-space aim guide while a throw is held: a dotted path that shows where the bolt will bounce.
pub fn draw_aim(batch: &mut Batch, world: &World, player: &Player) {
    if !player.aiming() {
        return;
    }
    let mut pos = player.center() + player.aim * 6.0;
    let mut dir = player.aim;
    let charged = player.throw_charge() >= 1.0;
    let mut bounces = 1;
    let range = if charged { 240.0 } else { 150.0 };
    let color = world.state.mode.light().mul(0.45 + player.throw_charge() * 0.4);
    let t = TILE as f32;
    let blocks = |x: f32, y: f32| world.map.blocks_flying((x / t).floor() as i32, ((y + 6.0) / t).floor() as i32);
    let mut travelled = 0.0;
    while travelled < range {
        let next = pos + dir * 3.0;
        if blocks(next.x, next.y) {
            if bounces <= 0 {
                break;
            }
            bounces -= 1;
            let block_x = blocks(next.x, pos.y);
            let block_y = blocks(pos.x, next.y);
            if block_x || !block_y {
                dir.x = -dir.x;
            }
            if block_y || !block_x {
                dir.y = -dir.y;
            }
            travelled += 3.0;
            continue;
        }
        pos = next;
        if (travelled / 3.0) as i32 % 3 == 0 {
            batch.fill_rect(pos.x.round(), pos.y.round(), 1.0, 1.0, color);
        }
        travelled += 3.0;
    }
}
