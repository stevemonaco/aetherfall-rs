//! The pause menu: items, equipment, skill loadouts, status and system options.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::LazyLock;

use bevy::math::Vec2;

use super::dialogue::DialogueScene;
use super::options::OptionsScene;
use super::scene::{Scene, SceneCtx, world_below};
use super::title::TitleScene;
use super::weave::WeaveScene;
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH, World};
use crate::art::hero::{HeroPose, hero_sprite};
use crate::art::{combat as combat_art, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::{Batch, TexId};
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::gameplay::combat;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::{self, EnemyDef, QuestDef, data};
use crate::gameplay::game_state::GameState;
use crate::gameplay::items::{self, ItemKind};
use crate::gameplay::mastery;
use crate::gameplay::supports::Group;
use crate::gameplay::{pacts, souls};
use crate::scene_any;

const TABS: [&str; 9] = ["Items", "Gear", "Skills", "Souls", "Weave", "Quests", "Foes", "Status", "System"];
const TAB_BOX: IRect = IRect::new(12, 12, 92, 120);
const CONTENT_BOX: IRect = IRect::new(110, 12, 358, 246);
const VISIBLE_ROWS: usize = 14;
/// Leaves room under the picker for a three-line description inside the content window.
const VISIBLE_CHOICES: usize = 8;

/// Quests in the order the data file lists them.
static QUESTS: LazyLock<Vec<&'static QuestDef>> = LazyLock::new(|| {
    let text = game_data::file("data/quests.json").unwrap_or("");
    let mut quests: Vec<&'static QuestDef> = data().quests.values().collect();
    quests.sort_by_key(|q| text.find(&format!("\"{}\"", q.id)).unwrap_or(usize::MAX));
    quests
});

#[derive(Default)]
struct Row {
    id: String,
    label: String,
    detail: String,
    icon: Option<TexId>,
    marked: bool,
    header: bool,
    /// The mode whose skill this row picks.
    mode: Option<Element>,
    /// The socket this row fills, and whether it is still locked.
    socket: Option<(Element, Group, i32)>,
    locked: bool,
}

impl Row {
    fn new(id: impl Into<String>, label: impl Into<String>, detail: impl Into<String>) -> Row {
        Row { id: id.into(), label: label.into(), detail: detail.into(), ..Row::default() }
    }

    fn header(label: impl Into<String>) -> Row {
        Row { label: label.into(), header: true, ..Row::default() }
    }
}

fn complete(state: &GameState, quest: &QuestDef) -> bool {
    quest.stages.iter().all(|s| state.check(s.done.as_deref()))
}

fn clock(seconds: f64) -> (i64, i64, i64) {
    let s = seconds as i64;
    (s / 3600, s / 60 % 60, s % 60)
}

fn move_index(index: &mut usize, count: usize) -> bool {
    let before = *index;
    if input::pressed(GameAction::Up) {
        *index = (*index + count - 1) % count;
    }
    if input::pressed(GameAction::Down) {
        *index = (*index + 1) % count;
    }
    if before == *index {
        return false;
    }
    audio::sfx(Sound::MenuMove);
    true
}

fn close(ctx: &mut SceneCtx) {
    audio::sfx(Sound::MenuCancel);
    ctx.pop();
    input::consume();
}

pub struct MenuScene {
    tab: usize,
    row: usize,
    scroll: usize,
    in_list: bool,
    pick_slot: Option<String>,
    /// The picker's options as (item id, label); the first is always the empty choice.
    pick_choices: Vec<(String, String)>,
    pick_row: usize,
    pick_scroll: usize,
    time: f32,
    to_title: Rc<Cell<bool>>,
}

impl MenuScene {
    pub fn new() -> MenuScene {
        MenuScene {
            tab: 0,
            row: 0,
            scroll: 0,
            in_list: false,
            pick_slot: None,
            pick_choices: Vec::new(),
            pick_row: 0,
            pick_scroll: 0,
            time: 0.0,
            to_title: Rc::new(Cell::new(false)),
        }
    }

    fn tab_name(&self) -> &'static str {
        TABS[self.tab]
    }

    fn rows(&self, state: &GameState) -> Vec<Row> {
        match self.tab_name() {
            "Items" => item_rows(state),
            "Gear" => gear_rows(state),
            "Skills" => skill_rows(state),
            "Souls" => state
                .souls_owned
                .iter()
                .filter_map(|id| souls::get(id))
                .map(|soul| Row {
                    marked: state.soul.as_deref() == Some(soul.id),
                    ..Row::new(soul.id, soul.name, format!("Rank {}", state.soul_rank(soul.id)))
                })
                .collect(),
            "Weave" => vec![Row::new("weave", "Open the Weave", format!("{} pts", state.weave_points()))],
            "Quests" => quest_rows(state),
            "Foes" => {
                let mut foes: Vec<&EnemyDef> = state.kills.keys().filter_map(|id| data().enemies.get(id)).collect();
                foes.sort_by_key(|d| d.level);
                foes.into_iter()
                    .map(|d| Row::new(d.id.clone(), d.name.clone(), format!("x{}", state.kills[&d.id])))
                    .collect()
            }
            "System" => vec![Row::new("options", "Options", ""), Row::new("title", "Return to title", "")],
            _ => Vec::new(),
        }
    }

    fn activate(&mut self, world: &mut World, row: &Row) -> Option<Box<dyn Scene>> {
        let state = &mut world.state;
        match self.tab_name() {
            "Items" => use_item(world, &row.id, &mut self.in_list),
            "Gear" => {
                let kind = match row.id.as_str() {
                    "armor" => ItemKind::Armor,
                    "shield" => ItemKind::Shield,
                    _ => ItemKind::Accessory,
                };
                let mut choices: Vec<&String> =
                    state.equipment.iter().filter(|id| items::get(id).is_some_and(|i| i.kind == kind)).collect();
                choices.sort_by(|a, b| {
                    let power = |id: &str| items::get(id).map_or(0, |i| i.power);
                    power(a).cmp(&power(b)).then_with(|| items::name_of(a).cmp(&items::name_of(b)))
                });
                self.pick_choices = std::iter::once(("".to_string(), "(none)".to_string()))
                    .chain(choices.into_iter().map(|id| (id.clone(), items::name_of(id))))
                    .collect();
                self.pick_slot = Some(row.id.clone());
                self.pick_row = 0;
                self.pick_scroll = 0;
                audio::sfx(Sound::MenuSelect);
            }
            "Skills" => {
                if let Some(mode) = row.mode {
                    state.cycle_skill(mode);
                    audio::sfx(Sound::MenuSelect);
                } else if row.locked {
                    audio::sfx(Sound::MenuCancel);
                } else if let Some((mode, group, index)) = row.socket {
                    self.pick_choices = std::iter::once(("".to_string(), "(empty)".to_string()))
                        .chain(gem_choices(state, mode, group))
                        .collect();
                    self.pick_slot = Some(format!("socket:{}", GameState::socket_key(mode, group, index)));
                    self.pick_row = 0;
                    self.pick_scroll = 0;
                    audio::sfx(Sound::MenuSelect);
                }
            }
            "Souls" => {
                state.soul = if state.soul.as_deref() == Some(row.id.as_str()) { None } else { Some(row.id.clone()) };
                world.spawn_companion();
                audio::sfx(Sound::MenuSelect);
            }
            "Weave" => {
                audio::sfx(Sound::MenuSelect);
                return Some(Box::new(WeaveScene::new()));
            }
            "Quests" => {
                if let Some(quest) = data().quests.get(&row.id).filter(|q| !complete(state, q)) {
                    let tracked = state.tracked_quest.as_deref() == Some(quest.id.as_str());
                    state.tracked_quest = if tracked { None } else { Some(quest.id.clone()) };
                    audio::sfx(Sound::MenuSelect);
                }
            }
            "System" if row.id == "options" => {
                audio::sfx(Sound::MenuSelect);
                return Some(Box::new(OptionsScene::new()));
            }
            "System" => {
                audio::sfx(Sound::MenuSelect);
                let to_title = self.to_title.clone();
                return Some(Box::new(DialogueScene::ask(
                    "Return to the title screen? Progress since your last save will be lost.".into(),
                    String::new(),
                    Box::new(move |_, yes| to_title.set(yes)),
                )));
            }
            _ => {}
        }
        None
    }

    fn update_choice(&mut self, state: &mut GameState) {
        move_index(&mut self.pick_row, self.pick_choices.len());
        self.pick_scroll = self.pick_scroll.clamp((self.pick_row + 1).saturating_sub(VISIBLE_CHOICES), self.pick_row);
        if input::pressed(GameAction::Cancel) {
            self.pick_slot = None;
            audio::sfx(Sound::MenuCancel);
            return;
        }
        if !input::pressed(GameAction::Confirm) {
            return;
        }
        let id = if self.pick_row == 0 { None } else { Some(self.pick_choices[self.pick_row].0.clone()) };
        if let Some(key) = self.pick_slot.as_deref().and_then(|s| s.strip_prefix("socket:")) {
            state.set_socket(key, id.as_deref());
            audio::sfx(Sound::MenuSelect);
            self.pick_slot = None;
            return;
        }
        match self.pick_slot.as_deref() {
            Some("armor") => state.armor = id,
            Some("shield") => state.shield = id,
            other => {
                let slot = if other == Some("acc0") { 0 } else { 1 };
                if id.is_some() && state.accessories[1 - slot] == id {
                    state.accessories[1 - slot] = None;
                }
                state.accessories[slot] = id;
            }
        }
        state.hp = state.hp.min(state.max_hp());
        audio::sfx(Sound::MenuSelect);
        self.pick_slot = None;
    }

    fn describe(&self, state: &GameState, row: &Row) -> String {
        if let Some(item) = items::get(&row.id) {
            return if item.kind == ItemKind::Powerup {
                format!("Permanent. {}", item.description)
            } else {
                item.description.to_string()
            };
        }
        match self.tab_name() {
            "Gear" => "Choose equipment for this slot.".into(),
            "Souls" => match souls::get(&row.id) {
                Some(soul) => {
                    let verb = if state.soul.as_deref() == Some(row.id.as_str()) { "dismiss" } else { "bring along" };
                    format!("{} {} Confirm to {verb}.", soul.passive, soul.active)
                }
                None => String::new(),
            },
            "Quests" => match data().quests.get(&row.id) {
                Some(quest) => {
                    let stage = quest.stages.iter().find(|s| !state.check(s.done.as_deref()));
                    let mut text = stage.map_or("Completed.".to_string(), |s| s.text.clone());
                    if !quest.reward.is_empty() {
                        text.push_str(&format!(" Reward: {}.", quest.reward));
                    }
                    if stage.is_some() {
                        text.push_str(" Confirm to track.");
                    }
                    text
                }
                None => String::new(),
            },
            "Foes" => data().enemies.get(&row.id).map(|foe| describe_foe(state, foe)).unwrap_or_default(),
            "Weave" => "Spend Weave points on stats and passives.".into(),
            "Skills" if row.socket.is_some() => match state.sockets.get(row.id.trim_start_matches("socket|")) {
                Some(id) if !row.locked => {
                    items::get(id).map(|i| format!("{} Confirm to change.", i.description)).unwrap_or_default()
                }
                _ if row.locked => {
                    format!("This socket opens at mastery {} in this mode.", mastery::ADEPT)
                }
                _ if row.socket.is_some_and(|s| s.0 == Element::Aether) => {
                    "An empty socket for a Prism gem, from the Trial of the Prism.".into()
                }
                _ => "An empty socket. Support gems come from the golden Trial Rifts.".into(),
            },
            "Skills" => match row.mode.and_then(|mode| state.equipped_skill(mode).map(|s| (mode, s))) {
                Some((Element::Aether, skill)) => {
                    let level = state.aether_level();
                    format!(
                        "{} Lv{} {} Aether lasts {:.0}s; it grows with sigils and the blade's awakening. Confirm to swap.",
                        skill.description,
                        level,
                        skill.levels[(level - 1).clamp(0, 2) as usize],
                        state.aether_duration()
                    )
                }
                Some((mode, skill)) => {
                    let unlocked = state.skill_level(mode);
                    let levels: Vec<String> = skill.levels[..unlocked as usize]
                        .iter()
                        .enumerate()
                        .map(|(i, text)| format!("Lv{} {text}", i + 1))
                        .collect();
                    format!(
                        "{} {} Tap {} for Lv1; hold to charge. Confirm to swap.",
                        skill.description,
                        levels.join(" "),
                        input::glyph(GameAction::Skill)
                    )
                }
                None => String::new(),
            },
            _ => String::new(),
        }
    }

    fn draw_list(&self, batch: &mut Batch, state: &GameState) {
        let rows = self.rows(state);
        let content = CONTENT_BOX;
        let (cx, cy) = (content.x as f32, content.y as f32);
        let shadow = Some(palette::TEXT_SHADOW);
        if rows.is_empty() {
            font::draw(batch, "Nothing here yet.", Vec2::new(cx + 12.0, cy + 10.0), palette::TEXT_DIM, None);
            return;
        }
        for (i, row) in rows.iter().enumerate().skip(self.scroll).take(VISIBLE_ROWS) {
            let y = cy + 10.0 + (i - self.scroll) as f32 * 14.0;
            if row.header {
                if let Some(icon) = row.icon {
                    let pos = Vec2::new(cx + 10.0, y - 3.0);
                    batch.draw_ex(icon, pos, None, Rgba::WHITE, 0.0, Vec2::ZERO, Vec2::splat(0.7), false, false);
                }
                let x = cx + if row.icon.is_some() { 26.0 } else { 12.0 };
                font::draw(batch, &row.label, Vec2::new(x, y), palette::TEXT_DIM, shadow);
                continue;
            }
            if let Some(icon) = row.icon {
                batch.draw(icon, Vec2::new(cx + 20.0, y - 2.0), Rgba::WHITE);
            }
            let color = if row.marked { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, &row.label, Vec2::new(cx + 36.0, y), color, shadow);
            if !row.detail.is_empty() {
                let pos = Vec2::new((content.right() - 12 - font::measure(&row.detail)) as f32, y);
                let color = if self.tab_name() == "Items" { palette::TEXT_DIM } else { palette::HIGHLIGHT };
                font::draw(batch, &row.detail, pos, color, None);
            }
            if self.in_list && i == self.row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new(cx + 10.0, y), palette::HIGHLIGHT, None);
            }
        }
        let arrow_x = (content.right() - 12) as f32;
        if self.scroll > 0 {
            font::draw(batch, "^", Vec2::new(arrow_x, cy + 3.0), palette::TEXT_DIM, None);
        }
        if self.scroll + VISIBLE_ROWS < rows.len() {
            font::draw(batch, "v", Vec2::new(arrow_x, (content.bottom() - 12) as f32), palette::TEXT_DIM, None);
        }

        if self.in_list && self.row < rows.len() {
            let lines = font::wrap(&self.describe(state, &rows[self.row]), content.w - 20);
            let count = lines.len() as i32;
            let rule = IRect::new(content.x + 6, content.bottom() - 10 - count * 11, content.w - 12, 1);
            batch.fill(rule, palette::WINDOW_BORDER.mul(0.4));
            for (i, line) in lines.iter().enumerate() {
                let y = (content.bottom() - 6 - (count - i as i32) * 11) as f32;
                font::draw(batch, line, Vec2::new(cx + 10.0, y), palette::TEXT_DIM, None);
            }
        }
    }

    fn draw_choices(&self, batch: &mut Batch) {
        let shown = self.pick_choices.len().min(VISIBLE_CHOICES);
        let rect = IRect::new(CONTENT_BOX.x + 40, CONTENT_BOX.y + 70, 280, 24 + shown as i32 * 13);
        batch.draw_window(rect);
        let x = rect.x as f32;
        for (i, (id, label)) in self.pick_choices.iter().enumerate().skip(self.pick_scroll).take(shown) {
            let y = (rect.y + 8 + (i - self.pick_scroll) as i32 * 13) as f32;
            if i > 0 {
                batch.draw(props::icon(id), Vec2::new(x + 20.0, y - 2.0), Rgba::WHITE);
            }
            let color = if i == self.pick_row { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, label, Vec2::new(x + 36.0, y), color, Some(palette::TEXT_SHADOW));
            if i == self.pick_row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new(x + 8.0, y), palette::HIGHLIGHT, None);
            }
        }
        let arrow_x = (rect.right() - 14) as f32;
        if self.pick_scroll > 0 {
            font::draw(batch, "^", Vec2::new(arrow_x, (rect.y + 4) as f32), palette::TEXT_DIM, None);
        }
        if self.pick_scroll + shown < self.pick_choices.len() {
            font::draw(batch, "v", Vec2::new(arrow_x, (rect.bottom() - 14) as f32), palette::TEXT_DIM, None);
        }
        if self.pick_row > 0
            && let Some(item) = items::get(&self.pick_choices[self.pick_row].0)
        {
            for (i, line) in font::wrap(item.description, rect.w - 16).iter().enumerate() {
                let pos = Vec2::new(x + 8.0, (rect.bottom() + 4 + i as i32 * 11) as f32);
                font::draw(batch, line, pos, palette::TEXT_DIM, Some(palette::TEXT_SHADOW));
            }
        }
    }

    fn draw_status(&self, batch: &mut Batch, s: &GameState) {
        let mode = s.mode;
        let (x, y) = ((CONTENT_BOX.x + 14) as f32, (CONTENT_BOX.y + 10) as f32);
        let crystals: Vec<&str> = [("ember_crystal", "Ember"), ("tide_crystal", "Tide"), ("storm_crystal", "Storm")]
            .into_iter()
            .filter(|c| s.has(c.0))
            .map(|c| c.1)
            .collect();
        let movement: Vec<&str> = [
            s.can_jump().then_some("Rabbit Boots"),
            s.has_tether().then_some("Tether"),
            s.has("gale_dash").then_some("Gale Boots"),
        ]
        .into_iter()
        .flatten()
        .collect();
        let foc = s.foc(mode);
        let rows: [(&str, String, Rgba); 13] = [
            ("Level", s.level.to_string(), palette::TEXT),
            (
                "Experience",
                if s.level >= combat::MAX_LEVEL { "MAX".into() } else { format!("{} / {}", s.exp, s.exp_to_next()) },
                palette::EXPERIENCE,
            ),
            ("Life", format!("{} / {}", s.hp, s.max_hp()), palette::LIFE),
            ("SP", format!("{} / {}", s.sp as i32, s.max_sp() as i32), Rgba::rgb(120, 220, 255)),
            ("Mode", format!("{}  (mastery {})", mode.mode_label(), s.mastery_level(mode)), mode.glow()),
            ("Attack", format!("{}", s.atk(mode).round()), palette::TEXT),
            ("Defense", format!("{}", s.def(mode).round()), palette::TEXT),
            (
                "Focus",
                format!("{}  ({}% crit)", foc.round(), (combat::crit_chance(foc) * 100.0).round()),
                palette::TEXT,
            ),
            ("Dodges", s.dodge_charges().to_string(), palette::TEXT),
            ("Blade", if s.smith_level > 0 { format!("+{}", s.smith_level) } else { "Unforged".into() }, palette::TEXT),
            ("Movement", if movement.is_empty() { "-".into() } else { movement.join(", ") }, palette::TEXT),
            ("Life Shards", format!("{} / 4", s.life_shards() % 4), palette::LIFE),
            ("Crystals", if crystals.is_empty() { "None".into() } else { crystals.join(", ") }, palette::HIGHLIGHT),
        ];
        for (i, (label, value, color)) in rows.iter().enumerate() {
            let row_y = y + i as f32 * 13.0;
            font::draw(batch, label, Vec2::new(x, row_y), palette::TEXT_DIM, None);
            font::draw(batch, value, Vec2::new(x + 90.0, row_y), *color, Some(palette::TEXT_SHADOW));
        }
        let feet = Vec2::new((CONTENT_BOX.right() - 50) as f32, (CONTENT_BOX.y + 80) as f32);
        let frame = (self.time / 0.6) as i32 % 2;
        hero_sprite::draw(batch, feet, Direction::Down, HeroPose::Idle, frame, Rgba::WHITE, false, 3.0, 0.0);
    }
}

fn item_rows(state: &GameState) -> Vec<Row> {
    let mut owned: Vec<&'static items::ItemDef> = state.inventory.keys().filter_map(|id| items::get(id)).collect();
    owned.sort_by_key(|i| (i.kind as i32, i.name));
    let mut rows = Vec::new();
    let mut group = None;
    for item in owned {
        if group != Some(item.kind) {
            group = Some(item.kind);
            rows.push(Row::header(match item.kind {
                ItemKind::Consumable => "Supplies",
                ItemKind::Material => "Materials",
                ItemKind::Powerup => "Treasures",
                _ => "Key items",
            }));
        }
        let counted = matches!(item.kind, ItemKind::Consumable | ItemKind::Material)
            || item.id == "life_shard"
            || item.id == "weave_knot"
            || item.id == "aether_mote";
        let detail = if counted { format!("x{}", state.count(item.id)) } else { String::new() };
        rows.push(Row { icon: Some(props::icon(item.id)), ..Row::new(item.id, item.name, detail) });
    }
    rows
}

fn gear_rows(state: &GameState) -> Vec<Row> {
    let slot = |id: &str, label: &str, equipped: &Option<String>| match equipped {
        Some(item) => Row { icon: Some(props::icon(item)), ..Row::new(id, label, items::name_of(item)) },
        None => Row::new(id, label, "-"),
    };
    vec![
        slot("armor", "Armor", &state.armor),
        slot("shield", "Shield", &state.shield),
        slot("acc0", "Accessory", &state.accessories[0]),
        slot("acc1", "Accessory", &state.accessories[1]),
    ]
}

fn skill_rows(state: &GameState) -> Vec<Row> {
    let mut rows = Vec::new();
    for mode in state.blade_pages() {
        let shift = mode == Element::Aether;
        let heading = if shift {
            format!("Aether - level {}", state.aether_level())
        } else {
            let xp = state.mastery_xp(mode);
            let level = mastery::level(xp);
            if level >= mastery::MAX_MASTERY {
                format!("{} mode - mastered", mode.mode_label())
            } else {
                format!("{} mode - mastery {level} ({}%)", mode.mode_label(), (mastery::progress(xp) * 100.0) as i32)
            }
        };
        rows.push(Row { icon: Some(combat_art::emblem(mode)), ..Row::header(heading) });
        let detail = match state.equipped_skill(mode) {
            Some(skill) => format!("{}  Lv {}", skill.name, state.skill_level(mode)),
            None => "-".into(),
        };
        let label = if shift { "Form" } else { "Skill" };
        rows.push(Row { mode: Some(mode), ..Row::new(format!("skill|{}", mode.key()), label, detail) });
        let open = state.socket_count(mode);
        let sockets = if shift { 1 } else { 2 };
        for &group in GameState::socket_groups(mode) {
            for index in 0..sockets {
                let key = GameState::socket_key(mode, group, index);
                let locked = index >= open;
                let gem = state.sockets.get(&key).filter(|_| !locked);
                let detail = match gem {
                    Some(id) => items::name_of(id),
                    None if locked => format!("(mastery {})", mastery::ADEPT),
                    None => "-".into(),
                };
                rows.push(Row {
                    icon: gem.map(|id| props::icon(id)),
                    socket: Some((mode, group, index)),
                    locked,
                    ..Row::new(
                        format!("socket|{key}"),
                        if shift { "Prism gem".to_string() } else { format!("{} gem", group.label()) },
                        detail,
                    )
                });
            }
        }
    }
    rows
}

/// Owned gems that fit a socket, labelled with where they sit now if elsewhere.
fn gem_choices(state: &GameState, mode: Element, group: Group) -> Vec<(String, String)> {
    let aether = mode == Element::Aether;
    state
        .gems_owned()
        .into_iter()
        .filter(|g| g.fits_group(group) && g.prism == aether)
        .map(|g| {
            let place = state.sockets.iter().find(|(_, id)| *id == g.id).map(|(key, _)| socket_label(key));
            let label = match place {
                Some(place) => format!("{} ({place})", g.name),
                None => g.name.to_string(),
            };
            (g.id.to_string(), label)
        })
        .collect()
}

/// "Fire attack" for a socket key such as `fire.attack.0`.
fn socket_label(key: &str) -> String {
    let mut parts = key.split('.');
    let mode = Element::parse(parts.next());
    if mode == Element::Aether {
        return mode.mode_label().to_string();
    }
    format!("{} {}", mode.mode_label(), parts.next().unwrap_or(""))
}

fn quest_rows(state: &GameState) -> Vec<Row> {
    let started: Vec<&QuestDef> =
        QUESTS.iter().copied().filter(|q| state.flags.contains(&format!("quest:{}", q.id))).collect();
    let (done, active): (Vec<&QuestDef>, Vec<&QuestDef>) = started.into_iter().partition(|q| complete(state, q));
    let mut rows = Vec::new();
    if !active.is_empty() {
        rows.push(Row::header("In progress"));
    }
    rows.extend(active.iter().map(|q| Row {
        marked: state.tracked_quest.as_deref() == Some(q.id.as_str()),
        ..Row::new(q.id.clone(), q.title.clone(), q.region.clone())
    }));
    if !done.is_empty() {
        rows.push(Row::header("Completed"));
    }
    rows.extend(done.iter().map(|q| Row::new(q.id.clone(), q.title.clone(), "Done")));
    rows
}

fn describe_foe(state: &GameState, foe: &EnemyDef) -> String {
    let kills = state.kills.get(&foe.id).copied().unwrap_or(0);
    if kills < 5 {
        return format!("Defeat {} more to learn its weaknesses.", 5 - kills);
    }
    let elements = [Element::Wind, Element::Fire, Element::Water, Element::Thunder];
    let list = |test: &dyn Fn(f32) -> bool| {
        elements.iter().filter(|&&e| test(foe.resist_to(e))).map(|e| e.label()).collect::<Vec<_>>().join(", ")
    };
    let weak = list(&|m| m > 1.0);
    let resists = list(&|m| m > 0.0 && m < 1.0);
    let immune = list(&|m| m <= 0.0);
    let mut text = format!("Weak to: {}.", if weak.is_empty() { "nothing" } else { &weak });
    if !resists.is_empty() {
        text.push_str(&format!(" Resists: {resists}."));
    }
    if !immune.is_empty() {
        text.push_str(&format!(" Immune: {immune}."));
    }
    text
}

fn use_item(world: &mut World, id: &str, in_list: &mut bool) {
    let state = &world.state;
    let Some(item) = items::get(id).filter(|i| i.kind == ItemKind::Consumable && !pacts::active(state, "abstinence"))
    else {
        audio::sfx(Sound::MenuCancel);
        return;
    };
    let hurt = state.hp < state.max_hp();
    let low_sp = state.sp < state.max_sp();
    let ailing = world.player.as_ref().is_some_and(|p| p.status().any());
    let useful = match id {
        "spirit_tonic" => low_sp,
        "cooling_salve" => ailing,
        "elixir" => hurt || low_sp || ailing,
        _ => hurt,
    };
    if !useful {
        audio::sfx(Sound::MenuCancel);
        return;
    }
    let state = &mut world.state;
    state.take(id, 1);
    match id {
        "spirit_tonic" => state.sp = state.max_sp().min(state.sp + item.power as f32 * 100.0),
        "cooling_salve" => {
            if let Some(p) = world.player.as_mut() {
                p.cure_ailments();
            }
        }
        "elixir" => {
            state.hp = state.max_hp();
            state.sp = state.max_sp();
            if let Some(p) = world.player.as_mut() {
                p.cure_ailments();
            }
        }
        _ => state.hp = state.max_hp().min(state.hp + state.max_hp() * item.power / 100),
    }
    audio::sfx(Sound::Heal);
    if world.state.count(id) == 0 {
        *in_list = false;
    }
}

impl Scene for MenuScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
    }

    fn update(&mut self, ctx: &mut SceneCtx, dt: f32) {
        self.time += dt;
        if self.to_title.get() {
            ctx.replace(TitleScene::new());
            return;
        }
        let Some(world) = ctx.world() else { return };
        if self.pick_slot.is_some() {
            self.update_choice(&mut world.state);
            return;
        }
        // Enter maps to both Menu and Confirm; inside the menu it should confirm.
        if input::pressed(GameAction::Menu) && !input::pressed(GameAction::Confirm) && !self.in_list {
            close(ctx);
            return;
        }

        if !self.in_list {
            if move_index(&mut self.tab, TABS.len()) {
                self.row = 0;
                self.scroll = 0;
            }
            let rows = self.rows(&world.state);
            if input::pressed(GameAction::Confirm) && rows.iter().any(|r| !r.header) {
                self.in_list = true;
                self.row = rows.iter().position(|r| !r.header).unwrap_or(0);
                self.scroll = 0;
                audio::sfx(Sound::MenuSelect);
            } else if input::pressed(GameAction::Cancel) {
                close(ctx);
            }
            return;
        }

        let rows = self.rows(&world.state);
        if rows.is_empty() {
            self.in_list = false;
            return;
        }
        let count = rows.len();
        self.row = self.row.min(count - 1);
        let before = self.row;
        if move_index(&mut self.row, count) {
            let forward = self.row == (before + 1) % count;
            while rows[self.row].header {
                self.row = if forward { (self.row + 1) % count } else { (self.row + count - 1) % count };
            }
        }
        self.scroll = self.scroll.clamp((self.row + 1).saturating_sub(VISIBLE_ROWS), self.row);
        if input::pressed(GameAction::Cancel) {
            self.in_list = false;
            audio::sfx(Sound::MenuCancel);
        } else if input::pressed(GameAction::Confirm)
            && let Some(scene) = self.activate(world, &rows[self.row])
        {
            ctx.push_boxed(scene);
        }
    }

    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        let state = &world.state;
        batch.begin_screen();
        batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::BLACK.mul(0.5));
        batch.draw_window(TAB_BOX);
        let (tx, ty) = (TAB_BOX.x as f32, TAB_BOX.y as f32);
        for (i, tab) in TABS.iter().enumerate() {
            let color = if i == self.tab { palette::HIGHLIGHT } else { palette::TEXT };
            font::draw(batch, tab, Vec2::new(tx + 18.0, ty + 7.0 + i as f32 * 12.0), color, Some(palette::TEXT_SHADOW));
        }
        if !self.in_list || (self.time / 0.3) as i32 % 2 == 0 {
            let color = if self.in_list { palette::TEXT_DIM } else { palette::HIGHLIGHT };
            font::draw(batch, &CURSOR.to_string(), Vec2::new(tx + 7.0, ty + 7.0 + self.tab as f32 * 12.0), color, None);
        }

        let info = IRect::new(12, 138, 92, 70);
        batch.draw_window(info);
        let ix = (info.x + 6) as f32;
        for (i, line) in font::wrap(world.map_name(), info.w - 12).iter().take(2).enumerate() {
            font::draw(batch, line, Vec2::new(ix, (info.y + 5 + i as i32 * 10) as f32), palette::TEXT_DIM, None);
        }
        let (h, m, s) = clock(state.play_time);
        font::draw(batch, &format!("{h}:{m:02}:{s:02}"), Vec2::new(ix, (info.y + 32) as f32), palette::TEXT, None);
        font::draw(batch, &format!("{} G", state.gold), Vec2::new(ix, (info.y + 46) as f32), palette::GOLD, None);

        batch.draw_window(CONTENT_BOX);
        if self.tab_name() == "Status" {
            self.draw_status(batch, state);
        } else {
            self.draw_list(batch, state);
        }
        if self.pick_slot.is_some() {
            self.draw_choices(batch);
        }
    }
}
