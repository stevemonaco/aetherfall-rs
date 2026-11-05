//! The pause menu: items, equipment, skill loadouts, status and system options.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::LazyLock;

use bevy::math::Vec2;

use super::dialogue::DialogueScene;
use super::menu_pages;
use super::options::OptionsScene;
use super::scene::{Scene, SceneCtx, world_below};
use super::title::TitleScene;
use super::weave::WeaveScene;
use super::world::{SCREEN_HEIGHT, SCREEN_WIDTH, World};
use crate::art::{combat as combat_art, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::{Batch, TexId};
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::core::rng::Rng;
use crate::entities::entity::Entity;
use crate::entities::puzzles::SaveCrystal;
use crate::gameplay::element::Element;
use crate::gameplay::game_data::{self, EnemyDef, QuestDef, data};
use crate::gameplay::game_state::GameState;
use crate::gameplay::items::{self, ItemKind};
use crate::gameplay::mastery;
use crate::gameplay::supports::{self, Group};
use crate::gameplay::{collection, gear_mods, pacts, souls};
use crate::scene_any;
use crate::ui::widgets;

const TABS: [&str; 10] = ["Items", "Gear", "Skills", "Spirit", "Weave", "Quests", "Atlas", "Foes", "Status", "System"];
const TAB_BOX: IRect = IRect::new(12, 12, 92, 132);
/// Where the split tabs (Quests, Atlas, Foes) put their list and their detail panel.
const LIST_W: i32 = 150;
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
    /// The colour of this row's icon frame (an item's kind, a form's glow).
    accent: Option<Rgba>,
    /// The part of `icon` to show, for icons cut from a sprite strip.
    icon_src: Option<IRect>,
}

fn crystal_map_name(map: &str) -> String {
    super::map_scene::map_name(map).unwrap_or_default()
}

/// Whether the hero stands at a save crystal, where Weave nodes can be unpicked.
fn at_save_crystal(world: &World) -> bool {
    world.entities_of::<SaveCrystal>().any(|c| c.base().pos.distance(world.hero.pos) < 48.0)
}

/// Cuts `text` with an ellipsis so it fits in `width` pixels.
fn fit(text: &str, width: i32) -> String {
    if font::measure(text) <= width {
        return text.to_string();
    }
    let mut cut: String = text.to_string();
    while !cut.is_empty() && font::measure(&format!("{cut}...")) > width {
        cut.pop();
    }
    format!("{cut}...")
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
    /// The blade page open on the Skills tab: one per owned mode, then Aether.
    page: usize,
    time: f32,
    to_title: Rc<Cell<bool>>,
    to_crystal: Rc<Cell<bool>>,
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
            page: 0,
            time: 0.0,
            to_title: Rc::new(Cell::new(false)),
            to_crystal: Rc::new(Cell::new(false)),
        }
    }

    fn tab_name(&self) -> &'static str {
        TABS[self.tab]
    }

    fn page_mode(&self, state: &GameState) -> Element {
        let pages = state.blade_pages();
        pages.get(self.page.min(pages.len().saturating_sub(1))).copied().unwrap_or(Element::Wind)
    }

    fn page_count(&self, state: &GameState) -> usize {
        match self.tab_name() {
            "Skills" => state.blade_pages().len(),
            "Status" => menu_pages::status_pages(state).len(),
            _ => 0,
        }
    }

    fn split(&self) -> bool {
        matches!(self.tab_name(), "Quests" | "Atlas" | "Foes")
    }

    fn visible_rows(&self) -> usize {
        match self.tab_name() {
            "Skills" => 9,
            "Items" | "Spirit" | "Gear" => 9,
            _ if self.split() => 13,
            _ => VISIBLE_ROWS,
        }
    }

    /// L and R (or left and right) turn the pages of the Skills and Status tabs.
    fn turn_page(&mut self, state: &GameState) {
        if self.page_count(state) == 0 {
            return;
        }
        let step = match input::page_turn() {
            0 if input::pressed(GameAction::Left) => -1,
            0 if input::pressed(GameAction::Right) => 1,
            0 => return,
            turn => turn,
        };
        let count = self.page_count(state).max(1) as i32;
        self.page = (self.page.min(count as usize - 1) as i32 + step).rem_euclid(count) as usize;
        self.row = 0;
        self.scroll = 0;
        audio::sfx(Sound::MenuMove);
    }

    fn rows(&self, state: &GameState) -> Vec<Row> {
        match self.tab_name() {
            "Items" => item_rows(state),
            "Gear" => gear_rows(state),
            "Skills" => skill_rows(state, self.page_mode(state)),
            "Spirit" => spirit_rows(state),
            "Weave" => Vec::new(),
            "Quests" => quest_rows(state),
            "Atlas" => collection::regions(state)
                .into_iter()
                .map(|n| {
                    let done = (collection::region_tally(state, n).progress() * 100.0).round() as i32;
                    Row::new(n.id.clone(), n.name.clone(), format!("{done}%"))
                })
                .collect(),
            "Foes" => {
                let mut foes: Vec<&EnemyDef> = state.kills.keys().filter_map(|id| data().enemies.get(id)).collect();
                foes.sort_by_key(|d| d.level);
                foes.into_iter()
                    .map(|d| Row::new(d.id.clone(), d.name.clone(), format!("x{}", state.kills[&d.id])))
                    .collect()
            }
            "System" => {
                let mut rows = vec![Row::new("options", "Options", "")];
                if let Some((map, _, _)) = &state.last_crystal {
                    rows.push(Row::new("crystal", "Return to last crystal", crystal_map_name(map)));
                }
                rows.push(Row::new("title", "Return to title", ""));
                rows
            }
            _ => Vec::new(),
        }
    }

    fn activate(&mut self, world: &mut World, row: &Row) -> Option<Box<dyn Scene>> {
        let state = &mut world.state;
        match self.tab_name() {
            "Items" if row.id == "gemcutter_prism" => {
                let gems =
                    state.gems_owned().into_iter().filter(|g| !g.prism && state.gem_tier(g.id) < supports::MAX_TIER);
                self.pick_choices = std::iter::once(("".to_string(), "(cancel)".to_string()))
                    .chain(gems.map(|g| {
                        let cost = supports::prisms_for(state.gem_tier(g.id));
                        (
                            g.id.to_string(),
                            format!("{} ({cost} Prism{})", state.gem_label(g.id), if cost > 1 { "s" } else { "" }),
                        )
                    }))
                    .collect();
                self.pick_slot = Some("prism".into());
                self.pick_row = 0;
                self.pick_scroll = 0;
                audio::sfx(Sound::MenuSelect);
            }
            "Items" if row.id == gear_mods::GLYPH => {
                self.pick_choices = etch_choices(state);
                self.pick_slot = Some("etch".into());
                self.pick_row = 0;
                self.pick_scroll = 0;
                audio::sfx(Sound::MenuSelect);
            }
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
            "Spirit" => {
                if let Some(slot) = row.id.strip_prefix("charm") {
                    let owned = items::all().filter(|i| i.kind == ItemKind::Charm && state.count(i.id) > 0);
                    self.pick_choices = std::iter::once(("".to_string(), "(empty)".to_string()))
                        .chain(owned.map(|i| (i.id.to_string(), i.name.to_string())))
                        .collect();
                    self.pick_slot = Some(format!("charm:{slot}"));
                    self.pick_row = 0;
                    self.pick_scroll = 0;
                } else {
                    state.soul = (row.id != souls::SPIRIT.id).then(|| row.id.clone());
                    world.spawn_companion();
                }
                audio::sfx(Sound::MenuSelect);
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
            "System" if row.id == "crystal" => {
                if world.boss.is_some() {
                    audio::sfx(Sound::MenuCancel);
                    return None;
                }
                audio::sfx(Sound::MenuSelect);
                let to_crystal = self.to_crystal.clone();
                return Some(Box::new(DialogueScene::ask(
                    "Return to the last save crystal you used? You keep everything you carry.".into(),
                    String::new(),
                    Box::new(move |_, yes| to_crystal.set(yes)),
                )));
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
        if self.pick_slot.as_deref() == Some("etch") {
            let Some(item) = id else {
                self.pick_slot = None;
                audio::sfx(Sound::MenuCancel);
                return;
            };
            if !state.take(gear_mods::GLYPH, 1) {
                self.pick_slot = None;
                audio::sfx(Sound::MenuCancel);
                return;
            }
            let rolled = gear_mods::roll(&Rng::from_time(), state.item_mods.get(&item).map(String::as_str));
            state.item_mods.insert(item, rolled.id.to_string());
            state.hp = state.hp.min(state.max_hp());
            audio::sfx(Sound::LevelUp);
            if state.count(gear_mods::GLYPH) > 0 {
                self.pick_choices = etch_choices(state);
            } else {
                self.pick_slot = None;
                self.in_list = false;
            }
            return;
        }
        if self.pick_slot.as_deref() == Some("prism") {
            self.pick_slot = None;
            let Some(gem) = id else {
                audio::sfx(Sound::MenuCancel);
                return;
            };
            let cost = supports::prisms_for(state.gem_tier(&gem));
            if state.count("gemcutter_prism") >= cost && state.take("gemcutter_prism", cost) {
                *state.gem_tiers.entry(gem).or_default() += 1;
                audio::sfx(Sound::LevelUp);
            } else {
                audio::sfx(Sound::MenuCancel);
            }
            return;
        }
        if let Some(slot) = self.pick_slot.as_deref().and_then(|s| s.strip_prefix("charm:")) {
            let slot: usize = slot.parse().unwrap_or(0);
            for (i, charm) in state.spirit_charms.iter_mut().enumerate() {
                if i != slot && id.is_some() && *charm == id {
                    *charm = None;
                }
            }
            state.spirit_charms[slot] = id;
            audio::sfx(Sound::MenuSelect);
            self.pick_slot = None;
            return;
        }
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
                state.item_description(&row.id)
            };
        }
        match self.tab_name() {
            "Gear" => "Choose equipment for this slot.".into(),
            "Spirit" => match souls::get(&row.id) {
                Some(form) => format!(
                    "{} {} Confirm to take this form, or cycle forms with the Soul button.",
                    form.attack, form.perk
                ),
                None => {
                    let slot: usize = row.id.strip_prefix("charm").and_then(|s| s.parse().ok()).unwrap_or(0);
                    match state.spirit_charms[slot].as_deref().and_then(items::get) {
                        Some(charm) => format!("{} Confirm to change.", charm.description),
                        None => "An empty charm slot. Charms change every form the spirit takes.".into(),
                    }
                }
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
            "Skills" if row.socket.is_some() => match state.sockets.get(row.id.trim_start_matches("socket|")) {
                Some(id) if !row.locked => {
                    format!("{} Confirm to change.", state.item_description(id))
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
                        "{} Lv{} {} Aether lasts {:.0}s; progress locks after {:.0}s unhurt. Confirm to swap.",
                        skill.description,
                        level,
                        skill.levels[(level - 1).clamp(0, 2) as usize],
                        state.aether_duration(),
                        state.aether_lock_delay()
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

    /// The Skills tab's header: a strip of page emblems with L/R hints, the page's name and its mastery.
    fn draw_skill_page(&self, batch: &mut Batch, state: &GameState) {
        let pages = state.blade_pages();
        let mode = self.page_mode(state);
        let (cx, cy) = (CONTENT_BOX.x as f32, CONTENT_BOX.y as f32);
        let shadow = Some(palette::TEXT_SHADOW);
        let spacing = 26.0;
        let start = cx + (CONTENT_BOX.w as f32 - pages.len() as f32 * spacing) / 2.0;
        for (i, &page) in pages.iter().enumerate() {
            let x = start + i as f32 * spacing;
            let frame = IRect::new(x as i32, cy as i32 + 6, 22, 22);
            let (fill, border, tint) = if page == mode {
                (page.dark(), page.glow(), Rgba::WHITE)
            } else {
                (palette::WINDOW_FILL_LIGHT.mul(0.5), palette::WINDOW_BORDER.mul(0.5), Rgba::WHITE.mul(0.45))
            };
            batch.fill(frame, fill);
            batch.draw_rect(frame, border);
            batch.draw(combat_art::emblem(page), Vec2::new(x + 2.0, cy + 8.0), tint);
        }
        if pages.len() > 1 {
            let (prev, next) = input::page_glyphs();
            font::draw(
                batch,
                prev,
                Vec2::new(start - 8.0 - font::measure(prev) as f32, cy + 13.0),
                palette::TEXT_DIM,
                None,
            );
            font::draw(
                batch,
                next,
                Vec2::new(start + pages.len() as f32 * spacing + 4.0, cy + 13.0),
                palette::TEXT_DIM,
                None,
            );
        }
        let title = if mode == Element::Aether {
            format!("Aether - level {}", state.aether_level())
        } else {
            format!("{} mode", mode.mode_label())
        };
        font::draw(batch, &title, Vec2::new(cx + 12.0, cy + 36.0), mode.glow(), shadow);
        if mode != Element::Aether {
            let xp = state.mastery_xp(mode);
            let level = mastery::level(xp);
            let bar = IRect::new(CONTENT_BOX.right() - 132, cy as i32 + 38, 120, 5);
            let text = if level >= mastery::MAX_MASTERY { "Mastered".to_string() } else { format!("Mastery {level}") };
            let tx = (bar.x - 6 - font::measure(&text)) as f32;
            font::draw(batch, &text, Vec2::new(tx, cy + 36.0), palette::TEXT, shadow);
            let progress = if level >= mastery::MAX_MASTERY { 1.0 } else { mastery::progress(xp) };
            batch.fill(bar, palette::WINDOW_FILL_LIGHT.mul(0.6));
            batch.fill(IRect::new(bar.x, bar.y, (bar.w as f32 * progress) as i32, bar.h), mode.glow());
            batch.draw_rect(IRect::new(bar.x - 1, bar.y - 1, bar.w + 2, bar.h + 2), palette::WINDOW_BORDER.mul(0.6));
        }
        batch.fill(
            IRect::new(CONTENT_BOX.x + 6, cy as i32 + 50, CONTENT_BOX.w - 12, 1),
            palette::WINDOW_BORDER.mul(0.4),
        );
    }

    fn draw_list(&self, batch: &mut Batch, state: &GameState) {
        let rows = self.rows(state);
        let content = CONTENT_BOX;
        let (cx, cy) = (content.x as f32, content.y as f32);
        let shadow = Some(palette::TEXT_SHADOW);
        let skills = self.tab_name() == "Skills";
        let slots = matches!(self.tab_name(), "Skills" | "Items" | "Spirit" | "Gear");
        let split = self.split();
        let (top, spacing) = if skills {
            (60.0, 21.0)
        } else if slots {
            (12.0, 21.0)
        } else {
            (10.0, 14.0)
        };
        let right = if split { content.x + LIST_W } else { content.right() - 12 };
        if rows.is_empty() {
            font::draw(batch, "Nothing here yet.", Vec2::new(cx + 12.0, cy + 10.0), palette::TEXT_DIM, None);
            return;
        }
        for (i, row) in rows.iter().enumerate().skip(self.scroll).take(self.visible_rows()) {
            let y = cy + top + (i - self.scroll) as f32 * spacing;
            if row.header {
                if i + 1 < self.scroll + self.visible_rows() {
                    widgets::section(batch, &row.label, Vec2::new(cx + 10.0, y), right);
                }
                continue;
            }
            let selected = self.in_list && i == self.row;
            if selected {
                let band = IRect::new(
                    content.x + 6,
                    y as i32 - if slots { 6 } else { 2 },
                    right - content.x - 4,
                    if slots { 20 } else { 12 },
                );
                batch.fill(band, palette::WINDOW_FILL_LIGHT.mul(0.8));
            }
            let mut text_x = cx + 20.0;
            if slots {
                let accent = row.accent.or_else(|| skills.then(|| self.page_mode(state).glow()));
                widgets::icon_slot(batch, Vec2::new(cx + 18.0, y - 5.0), None, accent, row.locked);
                if row.locked && skills {
                    font::draw(batch, "x", Vec2::new(cx + 25.0, y), palette::TEXT_DIM, None);
                }
                if let Some(icon) = row.icon {
                    let (w, h) = crate::core::gfx::size(icon);
                    let src = row.icon_src.unwrap_or(IRect::new(0, 0, w, h));
                    let scale = (16.0 / src.w.max(src.h) as f32).min(1.0);
                    let at = Vec2::new(
                        cx + 18.0 + (18.0 - src.w as f32 * scale) / 2.0,
                        y - 5.0 + (18.0 - src.h as f32 * scale) / 2.0,
                    );
                    batch.draw_ex(icon, at, Some(src), Rgba::WHITE, 0.0, Vec2::ZERO, Vec2::splat(scale), false, false);
                }
                text_x = cx + 44.0;
            } else if self.tab_name() == "Foes"
                && let Some(foe) = data().enemies.get(&row.id)
            {
                menu_pages::foe_icon(batch, foe, Vec2::new(cx + 16.0, y - 4.0));
                text_x = cx + 36.0;
            } else if self.tab_name() == "Atlas"
                && let Some(node) = menu_pages::region_by_id(&row.id)
            {
                batch.fill(IRect::new(cx as i32 + 18, y as i32 + 1, 5, 5), menu_pages::kind_color(&node.kind));
                text_x = cx + 28.0;
            } else if self.tab_name() == "Quests" && row.marked {
                font::draw(batch, "*", Vec2::new(cx + 18.0, y), palette::HIGHLIGHT, None);
                text_x = cx + 28.0;
            }
            let color = if row.marked { palette::HIGHLIGHT } else { palette::TEXT };
            let room = right - text_x as i32 - if row.detail.is_empty() { 0 } else { font::measure(&row.detail) + 6 };
            let label = fit(&row.label, room);
            font::draw(batch, &label, Vec2::new(text_x, y), color, shadow);
            if !row.detail.is_empty() {
                let color = if matches!(self.tab_name(), "Items" | "Foes" | "Atlas" | "Quests")
                    || row.locked
                    || row.detail == "empty"
                {
                    palette::TEXT_DIM
                } else {
                    palette::HIGHLIGHT
                };
                widgets::right_text(batch, &row.detail, right as f32, y, color);
            }
            if selected {
                font::draw(batch, &CURSOR.to_string(), Vec2::new(cx + 8.0, y), palette::HIGHLIGHT, None);
            }
        }
        let arrow_x = (right - 4) as f32;
        if self.scroll > 0 {
            font::draw(batch, "^", Vec2::new(arrow_x, cy + 3.0), palette::TEXT_DIM, None);
        }
        if self.scroll + self.visible_rows() < rows.len() {
            font::draw(batch, "v", Vec2::new(arrow_x, (content.bottom() - 12) as f32), palette::TEXT_DIM, None);
        }

        let current = rows.get(self.row).filter(|r| !r.header && (self.in_list || split));
        if split {
            batch.fill(
                IRect::new(content.x + LIST_W + 8, content.y + 8, 1, content.h - 16),
                palette::WINDOW_BORDER.mul(0.4),
            );
            let area = IRect::new(
                content.x + LIST_W + 18,
                content.y + 10,
                content.right() - 10 - (content.x + LIST_W + 18),
                content.h - 20,
            );
            let row = current.or_else(|| rows.iter().find(|r| !r.header));
            if let Some(row) = row {
                self.draw_detail(batch, state, row, area);
            }
            return;
        }
        if let Some(row) = current {
            let lines = font::wrap(&self.describe(state, row), content.w - 20);
            let count = lines.len() as i32;
            let rule = content.bottom() - 10 - count * 11;
            widgets::rule(batch, content.x + 6, rule, content.w - 12);
            for (i, line) in lines.iter().enumerate() {
                let y = (content.bottom() - 6 - (count - i as i32) * 11) as f32;
                font::draw(batch, line, Vec2::new(cx + 10.0, y), palette::TEXT_DIM, None);
            }
        }
    }

    /// The right-hand panel of the split tabs: a foe's bestiary page, a region's atlas page or a quest's stages.
    fn draw_detail(&self, batch: &mut Batch, state: &GameState, row: &Row, area: IRect) {
        match self.tab_name() {
            "Foes" => {
                if let Some(foe) = data().enemies.get(&row.id) {
                    menu_pages::draw_foe(batch, state, foe, area, self.time);
                }
            }
            "Atlas" => {
                if let Some(node) = menu_pages::region_by_id(&row.id) {
                    menu_pages::draw_region(batch, state, node, area);
                }
            }
            "Quests" => {
                let Some(quest) = data().quests.get(&row.id) else { return };
                let (x, mut y) = (area.x as f32, area.y as f32);
                for line in font::wrap(&quest.title, area.w).iter().take(2) {
                    font::draw(batch, line, Vec2::new(x, y), palette::HIGHLIGHT, Some(palette::TEXT_SHADOW));
                    y += 11.0;
                }
                font::draw(
                    batch,
                    &format!("{}  -  {}", quest.giver, quest.region),
                    Vec2::new(x, y),
                    palette::TEXT_DIM,
                    None,
                );
                y += 16.0;
                let mut reached = true;
                for stage in &quest.stages {
                    if !reached {
                        break;
                    }
                    let done = state.check(stage.done.as_deref());
                    reached = done;
                    let mark = IRect::new(area.x, y as i32 + 1, 6, 6);
                    batch.fill(mark, if done { palette::EXPERIENCE } else { Rgba::BLACK.mul(0.4) });
                    batch.draw_rect(mark, palette::WINDOW_BORDER.mul(0.6));
                    let color = if done { palette::TEXT_DIM } else { palette::TEXT };
                    for line in font::wrap(&stage.text, area.w - 12) {
                        font::draw(batch, &line, Vec2::new(x + 12.0, y), color, None);
                        y += 11.0;
                    }
                    y += 3.0;
                }
                if !quest.reward.is_empty() {
                    widgets::section(batch, "Reward", Vec2::new(x, y + 4.0), area.right());
                    for (i, line) in font::wrap(&quest.reward, area.w).iter().enumerate() {
                        font::draw(batch, line, Vec2::new(x, y + 17.0 + i as f32 * 11.0), palette::GOLD, None);
                    }
                }
                if !complete(state, quest) {
                    let tracked = state.tracked_quest.as_deref() == Some(quest.id.as_str());
                    let hint = if tracked { "Tracked".to_string() } else { "Confirm to track".to_string() };
                    font::draw(batch, &hint, Vec2::new(x, (area.bottom() - 10) as f32), palette::TEXT_DIM, None);
                }
            }
            _ => {}
        }
    }
    fn draw_choices(&self, state: &GameState, batch: &mut Batch) {
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
        if self.pick_row > 0 && items::get(&self.pick_choices[self.pick_row].0).is_some() {
            let text = state.item_description(&self.pick_choices[self.pick_row].0);
            for (i, line) in font::wrap(&text, rect.w - 16).iter().enumerate() {
                let pos = Vec2::new(x + 8.0, (rect.bottom() + 4 + i as i32 * 11) as f32);
                font::draw(batch, line, pos, palette::TEXT_DIM, Some(palette::TEXT_SHADOW));
            }
        }
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
            || matches!(
                item.id,
                "life_shard" | "weave_knot" | "aether_mote" | "aether_conduit" | "insight_tome" | "spirit_pip"
            );
        let detail = if counted { format!("x{}", state.count(item.id)) } else { String::new() };
        let accent = match item.kind {
            ItemKind::Consumable => Rgba::rgb(120, 220, 120),
            ItemKind::Material => Rgba::rgb(120, 200, 230),
            ItemKind::Powerup => Rgba::rgb(210, 150, 255),
            _ => palette::GOLD,
        };
        rows.push(Row {
            icon: Some(props::icon(item.id)),
            accent: Some(accent),
            ..Row::new(item.id, item.name, detail)
        });
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

/// One blade page: its skill (or Aether form), then its gem sockets group by group.
fn skill_rows(state: &GameState, mode: Element) -> Vec<Row> {
    let shift = mode == Element::Aether;
    let detail = match state.equipped_skill(mode) {
        Some(skill) => format!("{}  Lv {}", skill.name, state.skill_level(mode)),
        None => "-".into(),
    };
    let label = if shift { "Form" } else { "Skill" };
    let mut rows = vec![Row {
        mode: Some(mode),
        icon: Some(combat_art::emblem(mode)),
        ..Row::new(format!("skill|{}", mode.key()), label, detail)
    }];
    let open = state.socket_count(mode);
    let sockets = if shift { 1 } else { 2 };
    for &group in GameState::socket_groups(mode) {
        for index in 0..sockets {
            let key = GameState::socket_key(mode, group, index);
            let locked = index >= open;
            let gem = state.sockets.get(&key).filter(|_| !locked);
            let detail = match gem {
                Some(id) => state.gem_label(id),
                None if locked => format!("opens at mastery {}", mastery::ADEPT),
                None => "empty".into(),
            };
            let label =
                if shift { "Prism".to_string() } else { format!("{} {}", group.label(), ["I", "II"][index as usize]) };
            rows.push(Row {
                icon: gem.map(|id| props::icon(id)),
                socket: Some((mode, group, index)),
                locked,
                ..Row::new(format!("socket|{key}"), label, detail)
            });
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
                Some(place) => format!("{} ({place})", state.gem_label(g.id)),
                None => state.gem_label(g.id),
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
        ..Row::new(q.id.clone(), q.title.clone(), "")
    }));
    if !done.is_empty() {
        rows.push(Row::header("Completed"));
    }
    rows.extend(done.iter().map(|q| Row::new(q.id.clone(), q.title.clone(), "")));
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

/// Gear a glyph can etch, each labelled with the mod it carries now.
fn etch_choices(state: &GameState) -> Vec<(String, String)> {
    let gear = state.equipment.iter().filter(|id| items::get(id).is_some_and(|i| gear_mods::can_etch(i.kind)));
    std::iter::once(("".to_string(), "(cancel)".to_string()))
        .chain(gear.map(|id| {
            let label = match state.item_mods.get(id).and_then(|m| gear_mods::get(m)) {
                Some(m) => format!("{}  [{}]", items::name_of(id), m.name),
                None => items::name_of(id),
            };
            (id.clone(), label)
        }))
        .collect()
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
    world.state.take(item.id, 1);
    world.use_consumable(item.id);
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
        if self.to_crystal.take() {
            close(ctx);
            if let Some(world) = ctx.world()
                && let Some((map, x, y)) = world.state.last_crystal.clone()
            {
                world.begin_warp(&map, (x, y + 1), Direction::Down, true);
            }
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

        self.turn_page(&world.state);
        if !self.in_list {
            if move_index(&mut self.tab, TABS.len()) {
                self.row = 0;
                self.scroll = 0;
                self.page = 0;
            }
            let rows = self.rows(&world.state);
            if self.tab_name() == "Weave" && input::pressed(GameAction::Confirm) {
                audio::sfx(Sound::MenuSelect);
                let scene = if at_save_crystal(world) { WeaveScene::at_crystal() } else { WeaveScene::new() };
                ctx.push(scene);
                return;
            }
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
        self.scroll = self.scroll.clamp((self.row + 1).saturating_sub(self.visible_rows()), self.row);
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

        let info = IRect::new(12, 150, 92, 70);
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
            let pages = menu_pages::status_pages(state);
            let page = self.page.min(pages.len() - 1);
            menu_pages::page_strip(
                batch,
                (CONTENT_BOX.x + CONTENT_BOX.w / 2) as f32,
                (CONTENT_BOX.y + 6) as f32,
                &pages,
                page,
            );
            widgets::rule(batch, CONTENT_BOX.x + 6, CONTENT_BOX.y + 34, CONTENT_BOX.w - 12);
            let area = IRect::new(CONTENT_BOX.x + 4, CONTENT_BOX.y + 42, CONTENT_BOX.w - 8, CONTENT_BOX.h - 48);
            match page.checked_sub(1).and_then(|i| state.owned_modes().get(i).copied()) {
                Some(mode) => menu_pages::draw_build(batch, state, mode, area),
                None => menu_pages::draw_hero(batch, state, area, self.time),
            }
        } else {
            if self.tab_name() == "Skills" {
                self.draw_skill_page(batch, state);
            }
            if self.tab_name() == "Weave" {
                let area = IRect::new(CONTENT_BOX.x + 4, CONTENT_BOX.y + 8, CONTENT_BOX.w - 8, CONTENT_BOX.h - 14);
                menu_pages::draw_weave(batch, state, area, at_save_crystal(world));
            } else {
                self.draw_list(batch, state);
            }
        }
        if self.pick_slot.is_some() {
            self.draw_choices(state, batch);
        }
    }
}

/// The spirit's forms (its own shape first, then each beast it has learned), then its charm slots.
fn spirit_rows(state: &GameState) -> Vec<Row> {
    let current = state.soul.as_deref().unwrap_or(souls::SPIRIT.id);
    let forms = std::iter::once(&souls::SPIRIT).chain(state.souls_owned.iter().filter_map(|id| souls::get(id)));
    let mut rows = vec![Row::header("Forms")];
    rows.extend(
        forms
            .map(|form| {
                let (icon, src) = crate::entities::companion::form_icon(form.id, form.glow);
                (form, icon, src)
            })
            .map(|(form, icon, src)| Row {
                marked: form.id == current,
                icon: Some(icon),
                icon_src: Some(src),
                accent: Some(form.glow),
                ..Row::new(form.id, form.name, if form.id == current { "Active" } else { "" })
            }),
    );
    rows.push(Row::header("Charms"));
    for (slot, charm) in state.spirit_charms.iter().enumerate() {
        let name = charm.as_deref().map_or("empty".to_string(), items::name_of);
        rows.push(Row {
            icon: charm.as_deref().map(props::icon),
            accent: Some(palette::GOLD),
            ..Row::new(format!("charm{slot}"), format!("Charm {}", ["I", "II"][slot]), name)
        });
    }
    rows
}
