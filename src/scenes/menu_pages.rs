//! The pause menu's drawn pages: the hero sheet, each mode's build summary, the bestiary and the collection atlas.

use bevy::math::Vec2;

use crate::art::hero::{HeroPose, hero_sprite};
use crate::art::{combat as combat_art, props};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::font;
use crate::core::geom::IRect;
use crate::core::gfx::{self, Batch, FrameStrip, TexId};
use crate::core::input;
use crate::core::palette;
use crate::gameplay::collection;
use crate::gameplay::combat;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::{EnemyDef, WorldNode, data};
use crate::gameplay::game_state::GameState;
use crate::gameplay::mastery;
use crate::gameplay::supports::{Group, SupportMods};
use crate::gameplay::weave::{self, WeaveMods};
use crate::ui::widgets;

const SHADOW: Option<Rgba> = Some(palette::TEXT_SHADOW);
const ELEMENTS: [Element; 4] = [Element::Wind, Element::Fire, Element::Water, Element::Thunder];

/// A row of page icons with the open one lit and L/R hints either side, centred on `center_x`.
pub fn page_strip(batch: &mut Batch, center_x: f32, y: f32, pages: &[(TexId, Rgba)], open: usize) {
    let spacing = 26.0;
    let start = center_x - pages.len() as f32 * spacing / 2.0;
    for (i, &(icon, accent)) in pages.iter().enumerate() {
        let x = start + i as f32 * spacing;
        let frame = IRect::new(x as i32, y as i32, 22, 22);
        let lit = i == open;
        batch.fill(frame, if lit { accent.mul(0.35) } else { palette::WINDOW_FILL_LIGHT.mul(0.5) });
        batch.draw_rect(frame, if lit { accent } else { palette::WINDOW_BORDER.mul(0.5) });
        let (w, h) = gfx::size(icon);
        let scale = (18.0 / w.max(h) as f32).min(1.0);
        let at = Vec2::new(x + (22.0 - w as f32 * scale) / 2.0, y + (22.0 - h as f32 * scale) / 2.0);
        let tint = if lit { Rgba::WHITE } else { Rgba::WHITE.mul(0.45) };
        batch.draw_ex(icon, at, None, tint, 0.0, Vec2::ZERO, Vec2::splat(scale), false, false);
    }
    if pages.len() > 1 {
        let (prev, next) = input::page_glyphs();
        let left = start - 8.0 - font::measure(prev) as f32;
        font::draw(batch, prev, Vec2::new(left, y + 7.0), palette::TEXT_DIM, None);
        font::draw(
            batch,
            next,
            Vec2::new(start + pages.len() as f32 * spacing + 4.0, y + 7.0),
            palette::TEXT_DIM,
            None,
        );
    }
}

/// The Status tab's pages: the hero sheet, then one build summary per mode the blade knows.
pub fn status_pages(state: &GameState) -> Vec<(TexId, Rgba)> {
    let mut pages = vec![(hero_icon(), palette::HIGHLIGHT)];
    pages.extend(state.owned_modes().into_iter().map(|m| (combat_art::emblem(m), m.glow())));
    pages
}

fn hero_icon() -> TexId {
    props::icon("tanned_hide")
}

fn label_value(batch: &mut Batch, label: &str, value: &str, x: f32, right: f32, y: f32, color: Rgba) {
    font::draw(batch, label, Vec2::new(x, y), palette::TEXT_DIM, None);
    widgets::right_text(batch, value, right, y, color);
}

/// The hero sheet: portrait, level and meters, combat stats, mastery per mode, and the treasures that open the world.
pub fn draw_hero(batch: &mut Batch, s: &GameState, area: IRect, time: f32) {
    let (x, y) = (area.x as f32, area.y as f32);
    let portrait = IRect::new(area.x + 8, area.y + 2, 70, 78);
    batch.fill(portrait, palette::WINDOW_FILL_LIGHT.mul(0.35));
    batch.draw_rect(portrait, palette::WINDOW_BORDER.mul(0.6));
    let feet = Vec2::new(portrait.x as f32 + 35.0, portrait.y as f32 + 66.0);
    let frame = (time / 0.6) as i32 % 2;
    hero_sprite::draw(batch, feet, Direction::Down, HeroPose::Idle, frame, Rgba::WHITE, false, 2.5, 0.0);
    let level = format!("Lv {}", s.level);
    font::draw_centered(
        batch,
        &level,
        Vec2::new(portrait.x as f32 + 35.0, portrait.y as f32 + 68.0),
        palette::HIGHLIGHT,
        SHADOW,
    );

    let (mx, right) = (x + 90.0, (area.right() - 8) as f32);
    let width = (right - mx) as i32;
    let maxed = s.level >= combat::MAX_LEVEL;
    let meters: [(&str, String, f32, Rgba); 3] = [
        (
            "EXP",
            if maxed { "MAX".into() } else { format!("{} / {}", s.exp, s.exp_to_next()) },
            if maxed { 1.0 } else { s.exp as f32 / s.exp_to_next().max(1) as f32 },
            palette::EXPERIENCE,
        ),
        ("Life", format!("{} / {}", s.hp, s.max_hp()), s.hp as f32 / s.max_hp().max(1) as f32, palette::LIFE),
        (
            "SP",
            format!("{} / {}", s.sp as i32, s.max_sp() as i32),
            s.sp / s.max_sp().max(1.0),
            Rgba::rgb(120, 220, 255),
        ),
    ];
    for (i, (label, value, fill, color)) in meters.iter().enumerate() {
        let row = y + 2.0 + i as f32 * 20.0;
        label_value(batch, label, value, mx, right, row, palette::TEXT);
        widgets::bar(batch, IRect::new(mx as i32, row as i32 + 11, width, 3), *fill, *color);
    }

    let mode = s.mode;
    let foc = s.foc(mode);
    let stats: [(&str, String); 6] = [
        ("Attack", format!("{}", s.atk(mode).round())),
        ("Defense", format!("{}", s.def(mode).round())),
        ("Focus", format!("{}", foc.round())),
        ("Crit", format!("{}%", (combat::crit_chance(foc) * 100.0).round())),
        ("Dodges", s.dodge_charges().to_string()),
        ("Blade", if s.smith_level > 0 { format!("+{}", s.smith_level) } else { "Unforged".into() }),
    ];
    let sy = y + 66.0;
    widgets::section(batch, &format!("In {} mode", mode.mode_label()), Vec2::new(mx, sy), right as i32);
    let col = (right - mx - 12.0) / 2.0;
    for (i, (label, value)) in stats.iter().enumerate() {
        let cx = mx + (i % 2) as f32 * (col + 12.0);
        label_value(batch, label, value, cx, cx + col, sy + 13.0 + (i / 2) as f32 * 11.0, palette::TEXT);
    }

    let my = y + 118.0;
    widgets::section(batch, "Mastery", Vec2::new(x + 8.0, my), right as i32);
    for (i, m) in s.owned_modes().into_iter().enumerate() {
        let cx = x + 8.0 + (i % 2) as f32 * ((right - x - 8.0) / 2.0);
        let ry = my + 13.0 + (i / 2) as f32 * 16.0;
        batch.draw_ex(
            combat_art::emblem(m),
            Vec2::new(cx, ry - 3.0),
            None,
            Rgba::WHITE,
            0.0,
            Vec2::ZERO,
            Vec2::splat(0.7),
            false,
            false,
        );
        let xp = s.mastery_xp(m);
        let level = mastery::level(xp);
        let done = level >= mastery::MAX_MASTERY;
        font::draw(
            batch,
            &if done { "Max".to_string() } else { format!("{level}") },
            Vec2::new(cx + 16.0, ry),
            palette::TEXT,
            None,
        );
        let progress = if done { 1.0 } else { mastery::progress(xp) };
        widgets::bar(batch, IRect::new(cx as i32 + 36, ry as i32 + 3, 90, 3), progress, m.glow());
    }

    let ty = (area.bottom() - 30) as f32;
    widgets::section(batch, "Treasures", Vec2::new(x + 8.0, ty - 12.0), right as i32);
    let treasures = ["ember_crystal", "tide_crystal", "storm_crystal", "rabbit_boots", "aether_tether", "gale_dash"];
    for (i, id) in treasures.iter().enumerate() {
        let owned = s.has(id) || (*id == "aether_tether" && s.has_tether());
        widgets::icon_slot(
            batch,
            Vec2::new(x + 8.0 + i as f32 * 21.0, ty),
            Some(props::icon(id)),
            Some(palette::HIGHLIGHT),
            !owned,
        );
    }
    let shards = s.life_shards() % 4;
    let px = x + 150.0;
    font::draw(batch, "Life Shards", Vec2::new(px, ty + 5.0), palette::TEXT_DIM, None);
    for i in 0..4 {
        let pip = IRect::new(px as i32 + 66 + i * 9, ty as i32 + 6, 6, 6);
        batch.fill(pip, if i < shards { palette::LIFE } else { Rgba::BLACK.mul(0.4) });
        batch.draw_rect(pip, palette::WINDOW_BORDER.mul(0.6));
    }
}

/// "+15%" or "-10%" for a multiplier, or nothing when it is unchanged.
fn pct(value: f32) -> Option<String> {
    let change = ((value - 1.0) * 100.0).round() as i32;
    (change != 0).then(|| format!("{change:+}%"))
}

/// The lines a socket group's gems add up to: changed numbers first, then the behaviours they grant.
fn group_lines(group: Group, m: &SupportMods, w: &WeaveMods) -> Vec<(String, String)> {
    let mut lines = Vec::new();
    let mut push = |label: &str, value: Option<String>| {
        if let Some(value) = value {
            lines.push((label.to_string(), value));
        }
    };
    push("Damage", pct(m.damage));
    match group {
        Group::Attack => {
            push("Swing speed", pct(m.swing_speed));
            push("Reach", pct(m.cleave * (1.0 + w.reach)));
            push("Finisher", pct(w.finisher));
        }
        Group::Ranged => {
            push("Charge", pct(m.charge_speed));
            let surge = m.surge + w.surge;
            push("Surge", (surge > 0.0).then(|| format!("{}%", (surge * 100.0).round())));
        }
        Group::Special => {
            push("Charge", pct(m.charge_speed));
        }
    }
    push("Ailments", pct(m.status));
    push("Guard break", pct(m.poise));
    push("Aether gain", pct(m.aether));
    push("SP gain", pct(m.sp_gain));
    push("Life steal", (m.leech > 0.0).then(|| format!("{:.1}%", m.leech * 100.0)));
    push("vs. broken", pct(m.execute));
    let behaviours: [(bool, &str); 12] = [
        (m.momentum, "Momentum"),
        (m.lunge, "Lunging"),
        (m.sweep, "Sweeping"),
        (m.whirlwind, "Whirlwind"),
        (m.deflect, "Deflecting"),
        (m.sunder, "Sundering"),
        (m.multishot, "Multishot"),
        (m.pierce, "Piercing"),
        (m.seeking, "Seeking"),
        (m.linger, "Lingering"),
        (m.echo, "Echo"),
        (m.efficiency, "Efficient"),
    ];
    for (on, name) in behaviours {
        if on {
            lines.push((name.to_string(), String::new()));
        }
    }
    lines
}

/// One mode's build: its sockets, what each group's gems add up to, and the Weave and gear on top.
pub fn draw_build(batch: &mut Batch, s: &GameState, mode: Element, area: IRect) {
    let (x, y, right) = (area.x as f32 + 8.0, area.y as f32, area.right() - 8);
    font::draw(batch, &format!("{} build", mode.mode_label()), Vec2::new(x, y), mode.glow(), SHADOW);
    let groups = GameState::socket_groups(mode);
    let mut sx = (right - 6 * 20) as f32;
    for &group in groups {
        for index in 0..2 {
            let key = GameState::socket_key(mode, group, index);
            let locked = index >= s.socket_count(mode);
            let gem = s.sockets.get(&key).filter(|_| !locked).map(|id| props::icon(id));
            widgets::icon_slot(batch, Vec2::new(sx, y - 5.0), gem, Some(mode.glow()), locked);
            sx += 20.0;
        }
    }

    let weave = s.mods();
    let col = (area.w as f32 - 16.0 - 2.0 * 10.0) / 3.0;
    let top = y + 22.0;
    let titles = [(Group::Attack, "Melee"), (Group::Ranged, "Bolts"), (Group::Special, "Skill")];
    for (i, (group, title)) in titles.iter().enumerate() {
        let cx = x + i as f32 * (col + 10.0);
        widgets::section(batch, title, Vec2::new(cx, top), (cx + col) as i32);
        let lines = group_lines(*group, &s.support_mods(mode, *group), &weave);
        if lines.is_empty() {
            font::draw(batch, "No changes", Vec2::new(cx, top + 13.0), palette::TEXT_DIM, None);
        }
        for (j, (label, value)) in lines.iter().take(9).enumerate() {
            let ly = top + 13.0 + j as f32 * 11.0;
            if value.is_empty() {
                font::draw(batch, label, Vec2::new(cx, ly), mode.glow(), None);
            } else {
                let color = if value.starts_with('-') { palette::LIFE } else { palette::EXPERIENCE };
                label_value(batch, label, value, cx, cx + col, ly, color);
            }
        }
    }

    let gy = top + 120.0;
    widgets::section(batch, "Weave and gear", Vec2::new(x, gy), right);
    let acc = s.acc();
    let foc = s.foc(mode);
    let mut stats: Vec<(&str, String)> = vec![
        ("Attack", format!("{}", s.atk(mode).round())),
        ("Defense", format!("{}", s.def(mode).round())),
        ("Crit", format!("{}%", (combat::crit_chance(foc) * 100.0).round())),
        ("Move speed", pct(weave.speed * acc.speed).unwrap_or_else(|| "-".into())),
    ];
    if weave.regen > 0.0 {
        stats.push(("Regen", format!("{:.1}%/s", weave.regen * 100.0)));
    }
    if weave.spirit > 0.0 {
        stats.push(("Spirit", format!("+{}%", (weave.spirit * 100.0).round())));
    }
    let col2 = (area.w as f32 - 16.0 - 10.0) / 2.0;
    for (i, (label, value)) in stats.iter().enumerate() {
        let cx = x + (i % 2) as f32 * (col2 + 10.0);
        label_value(batch, label, value, cx, cx + col2, gy + 13.0 + (i / 2) as f32 * 11.0, palette::TEXT);
    }
    let rows_used = stats.len().div_ceil(2) as f32;
    let mut passives: Vec<String> =
        weave.passives.iter().map(|p| weave::by_id(p).map_or_else(|| p.clone(), |n| n.name.clone())).collect();
    passives.sort();
    if !passives.is_empty() {
        let text = format!("Passives: {}", passives.join(", "));
        for (i, line) in font::wrap(&text, area.w - 16).iter().take(2).enumerate() {
            font::draw(batch, line, Vec2::new(x, gy + 15.0 + (rows_used + i as f32) * 11.0), palette::TEXT_DIM, None);
        }
    }
}

fn monster_strip(foe: &EnemyDef) -> FrameStrip {
    FrameStrip::new(gfx::external(&format!("external/Monsters/{}.png", foe.sprite)), 16, 16)
}

/// The foe sprite's first frame, for list rows.
pub fn foe_icon(batch: &mut Batch, foe: &EnemyDef, at: Vec2) {
    let strip = monster_strip(foe);
    batch.draw_src(strip.texture, at, Some(strip.source(0)), Rgba::WHITE);
}

/// Kills before a foe's weaknesses, and then its drops, are known.
const STUDY: i32 = 5;
const KNOWN: i32 = 15;

/// The bestiary page for one foe: an animated portrait, its level and tally, elemental reactions and drops.
pub fn draw_foe(batch: &mut Batch, s: &GameState, foe: &EnemyDef, area: IRect, time: f32) {
    let (x, y, right) = (area.x as f32, area.y as f32, area.right());
    let portrait = IRect::new(area.x, area.y, 60, 60);
    batch.fill(portrait, palette::WINDOW_FILL_LIGHT.mul(0.35));
    batch.draw_rect(portrait, palette::WINDOW_BORDER.mul(0.6));
    let strip = monster_strip(foe);
    let frame = (time * 4.0) as i32;
    strip.draw(batch, Vec2::new(x + 30.0, y + 52.0), frame, Rgba::WHITE, false, false, 3.0);

    let tx = x + 68.0;
    for (i, line) in font::wrap(&foe.name, right - tx as i32).iter().take(2).enumerate() {
        font::draw(batch, line, Vec2::new(tx, y + 2.0 + i as f32 * 11.0), palette::HIGHLIGHT, SHADOW);
    }
    let kills = s.kills.get(&foe.id).copied().unwrap_or(0);
    font::draw(batch, &format!("Defeated {kills}"), Vec2::new(tx, y + 28.0), palette::TEXT, None);
    let traits: Vec<&str> =
        [foe.flying.then_some("Flying"), foe.heavy.then_some("Heavy"), (foe.guard > 0.0).then_some("Guards")]
            .into_iter()
            .flatten()
            .collect();
    if !traits.is_empty() {
        font::draw(batch, &traits.join(", "), Vec2::new(tx, y + 40.0), palette::TEXT_DIM, None);
    }

    let mut ry = y + 70.0;
    if kills < STUDY {
        widgets::section(batch, "Weaknesses", Vec2::new(x, ry), right);
        let left = STUDY - kills;
        let text = format!("Defeat {left} more to learn how it reacts to each element.");
        for (i, line) in font::wrap(&text, area.w).iter().enumerate() {
            font::draw(batch, line, Vec2::new(x, ry + 13.0 + i as f32 * 11.0), palette::TEXT_DIM, None);
        }
        return;
    }
    widgets::section(batch, "Elements", Vec2::new(x, ry), right);
    for (i, e) in ELEMENTS.iter().enumerate() {
        let ex = x + (i % 2) as f32 * (area.w as f32 / 2.0);
        let ey = ry + 13.0 + (i / 2) as f32 * 18.0;
        batch.draw_ex(
            combat_art::emblem(*e),
            Vec2::new(ex, ey - 4.0),
            None,
            Rgba::WHITE,
            0.0,
            Vec2::ZERO,
            Vec2::splat(0.8),
            false,
            false,
        );
        let m = foe.resist_to(*e);
        let (text, color) = if m <= 0.0 {
            ("Immune".to_string(), palette::TEXT_DIM)
        } else if m > 1.0 {
            (format!("Weak x{m:.1}"), palette::EXPERIENCE)
        } else if m < 1.0 {
            (format!("Resists x{m:.1}"), palette::LIFE)
        } else {
            ("Normal".to_string(), palette::TEXT)
        };
        font::draw(batch, &text, Vec2::new(ex + 19.0, ey), color, None);
    }
    ry += 52.0;
    widgets::section(batch, "Drops", Vec2::new(x, ry), right);
    if kills < KNOWN {
        let text = format!("Defeat {} more to learn what it carries.", KNOWN - kills);
        font::draw(batch, &text, Vec2::new(x, ry + 13.0), palette::TEXT_DIM, None);
        return;
    }
    let mut drops: Vec<(String, String)> =
        foe.drops.iter().map(|d| (d.item.clone(), format!("{}%", (d.chance * 100.0).round()))).collect();
    if foe.gold > 0.0 {
        drops.insert(0, ("gold".into(), "Gold".into()));
    }
    if drops.is_empty() {
        font::draw(batch, "Nothing", Vec2::new(x, ry + 13.0), palette::TEXT_DIM, None);
    }
    for (i, (id, chance)) in drops.iter().take(4).enumerate() {
        let dx = x + (i % 2) as f32 * (area.w as f32 / 2.0);
        let dy = ry + 10.0 + (i / 2) as f32 * 21.0;
        let icon = Some(props::icon(id));
        widgets::icon_slot(batch, Vec2::new(dx, dy), icon, Some(palette::GOLD), false);
        let name = if id == "gold" { "Gold".to_string() } else { crate::gameplay::items::name_of(id) };
        font::draw(batch, &name, Vec2::new(dx + 22.0, dy + 1.0), palette::TEXT, None);
        if id != "gold" {
            font::draw(batch, chance, Vec2::new(dx + 22.0, dy + 10.0), palette::TEXT_DIM, None);
        }
    }
}

pub fn kind_color(kind: &str) -> Rgba {
    match kind {
        "town" => Rgba::rgb(150, 230, 150),
        "field" => Rgba::rgb(240, 220, 120),
        "side" => Rgba::rgb(240, 170, 100),
        "dungeon" => Rgba::rgb(240, 110, 100),
        "blight" => Rgba::rgb(200, 140, 255),
        _ => palette::TEXT,
    }
}

/// The atlas page for one region: its totals as meters, then each of its maps.
pub fn draw_region(batch: &mut Batch, s: &GameState, node: &WorldNode, area: IRect) {
    let (x, y, right) = (area.x as f32, area.y as f32, area.right());
    font::draw(batch, &node.name, Vec2::new(x, y), kind_color(&node.kind), SHADOW);
    let tally = collection::region_tally(s, node);
    let lines: [(&str, (i32, i32), Rgba); 4] = [
        ("Chests", tally.chests, palette::GOLD),
        ("Prisms", tally.prisms, Rgba::rgb(170, 230, 255)),
        ("Treasures", tally.treasures, Rgba::rgb(220, 150, 255)),
        ("Rifts sealed", tally.rifts, Rgba::rgb(200, 120, 255)),
    ];
    let mut ry = y + 16.0;
    for (label, (found, total), color) in lines {
        if total == 0 {
            continue;
        }
        label_value(batch, label, &format!("{found} / {total}"), x, right as f32, ry, palette::TEXT);
        widgets::bar(batch, IRect::new(area.x, ry as i32 + 11, area.w, 3), found as f32 / total as f32, color);
        ry += 20.0;
    }
    ry += 4.0;
    widgets::section(batch, "Areas", Vec2::new(x, ry), right);
    ry += 13.0;
    for map in &node.maps {
        let t = collection::map_tally(s, map);
        if t.is_empty() || !s.flags.contains(&format!("seen:{map}")) {
            continue;
        }
        if ry > (area.bottom() - 10) as f32 {
            break;
        }
        let name = crate::world::map::MapData::load(map).name;
        let done = t.chests.0 == t.chests.1 && t.rifts.0 == t.rifts.1;
        font::draw(batch, &name, Vec2::new(x, ry), if done { palette::EXPERIENCE } else { palette::TEXT }, None);
        let mut detail = format!("{}/{}", t.chests.0, t.chests.1);
        if t.prisms.1 > 0 {
            detail.push_str(&format!("  P {}/{}", t.prisms.0, t.prisms.1));
        }
        widgets::right_text(batch, &detail, right as f32, ry, palette::TEXT_DIM);
        ry += 11.0;
    }
}

pub fn region_by_id(id: &str) -> Option<&'static WorldNode> {
    data().world.iter().find(|n| n.id == id)
}

/// The Weave tab: points earned and spent, where they came from, knots, and the keystones woven so far.
pub fn draw_weave(batch: &mut Batch, s: &GameState, area: IRect, at_crystal: bool) {
    let (x, y, right) = (area.x as f32 + 8.0, area.y as f32, area.right() - 8);
    let tomes = s.count("insight_tome");
    let earned = s.level - 1 + tomes;
    let spent = earned - s.weave_points();
    font::draw(batch, "The Weave", Vec2::new(x, y), palette::HIGHLIGHT, SHADOW);
    label_value(batch, "Points woven", &format!("{spent} / {earned}"), x, right as f32, y + 18.0, palette::TEXT);
    widgets::bar(
        batch,
        IRect::new(x as i32, y as i32 + 29, right - x as i32, 3),
        spent as f32 / earned.max(1) as f32,
        Rgba::rgb(200, 170, 255),
    );

    widgets::section(batch, "Sources", Vec2::new(x, y + 42.0), right);
    let knots_owned = s.count("weave_knot");
    let knots_used = knots_owned - s.knots_free();
    let lines: [(&str, String); 4] = [
        ("From levels", format!("{}", s.level - 1)),
        ("Insight Tomes", format!("{tomes} / {}", collection::item_total("insight_tome"))),
        ("Weave Knots found", format!("{knots_owned} / {}", collection::item_total("weave_knot"))),
        ("Knots tied", format!("{knots_used} / {knots_owned}")),
    ];
    for (i, (label, value)) in lines.iter().enumerate() {
        label_value(batch, label, value, x, right as f32, y + 55.0 + i as f32 * 11.0, palette::TEXT);
    }

    let woven: Vec<&weave::WeaveNode> = s.weave_nodes.iter().filter_map(|id| weave::by_id(id)).collect();
    let count = |kind: weave::NodeKind| woven.iter().filter(|n| n.kind == kind).count();
    let ky = y + 108.0;
    widgets::section(batch, "Woven", Vec2::new(x, ky), right);
    let summary = format!(
        "{} keystones, {} notables, {} threads",
        count(weave::NodeKind::Keystone),
        count(weave::NodeKind::Notable),
        woven.len() - count(weave::NodeKind::Keystone) - count(weave::NodeKind::Notable)
    );
    font::draw(batch, &summary, Vec2::new(x, ky + 13.0), palette::TEXT, None);
    let mut names: Vec<&str> =
        woven.iter().filter(|n| n.kind != weave::NodeKind::Minor).map(|n| n.name.as_str()).collect();
    names.sort();
    for (i, line) in font::wrap(&names.join(", "), area.w - 16).iter().take(4).enumerate() {
        font::draw(batch, line, Vec2::new(x, ky + 26.0 + i as f32 * 11.0), palette::TEXT_DIM, None);
    }

    let hint = if at_crystal {
        "Confirm: open the Weave. At a save crystal you can also unpick nodes."
    } else {
        "Confirm: open the Weave. Unpick nodes at a save crystal."
    };
    for (i, line) in font::wrap(hint, area.w - 16).iter().enumerate() {
        font::draw(batch, line, Vec2::new(x, (area.bottom() - 22) as f32 + i as f32 * 11.0), palette::TEXT_DIM, None);
    }
}
