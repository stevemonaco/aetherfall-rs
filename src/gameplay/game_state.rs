//! Everything that persists in a save file.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use bevy::math::Vec2;
use serde::{Deserialize, Serialize};

use super::combat::{self, CombatStats};
use super::element::{Element, MODES};
use super::game_data::data;
use super::items::{self, AccessoryStats, ItemKind};
use super::mastery;
use super::skills::{self, SkillDef};
use super::souls;
use super::supports::{self, Group, SupportDef, SupportMods};
use super::weave::{self, WeaveMods, WeaveNode};
use crate::core::direction::Direction;
use crate::core::storage;

pub const SAVE_VERSION: i32 = 8;
/// The oldest save version that can still be migrated.
const OLDEST_SAVE: i32 = 3;
pub const SLOT_COUNT: i32 = 8;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "PascalCase")]
pub struct GameState {
    pub version: i32,
    pub slot: i32,
    pub map_id: String,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub facing: Direction,
    pub level: i32,
    pub exp: i32,
    pub hp: i32,
    pub sp: f32,
    pub gold: i32,
    /// Element keys of the sigils set into the blade.
    pub sigils: Vec<String>,
    /// Mastery experience per mode key.
    pub mastery: BTreeMap<String, i32>,
    pub mode: Element,
    pub smith_level: i32,
    pub inventory: BTreeMap<String, i32>,
    pub equipment: Vec<String>,
    pub armor: Option<String>,
    pub shield: Option<String>,
    pub accessories: [Option<String>; 2],
    pub learned_skills: BTreeSet<String>,
    /// The skill equipped in each mode, by mode key.
    pub equipped_skills: BTreeMap<String, String>,
    /// Support gems by socket key (`mode.group.index`).
    pub sockets: BTreeMap<String, String>,
    pub flags: BTreeSet<String>,
    #[serde(alias = "CircuitNodes")]
    pub weave_nodes: BTreeSet<String>,
    pub souls_owned: Vec<String>,
    /// The guardian spirit's current form, or None for its own shape.
    pub soul: Option<String>,
    pub spirit_charms: [Option<String>; souls::CHARM_SLOTS],
    /// Gemcutter tiers by gem id.
    pub gem_tiers: BTreeMap<String, i32>,
    pub tracked_quest: Option<String>,
    pub pacts: BTreeSet<String>,
    pub kills: BTreeMap<String, i32>,
    pub play_time: f64,
    /// Blight soaked up from blighted ground (0 to 50): a percentage taken off max life until cleansed.
    #[serde(skip)]
    pub corruption: f32,
    #[serde(skip)]
    mods: RefCell<Option<WeaveMods>>,
}

impl Default for GameState {
    fn default() -> Self {
        GameState {
            version: SAVE_VERSION,
            slot: 1,
            map_id: "vault".into(),
            x: None,
            y: None,
            facing: Direction::Down,
            level: 1,
            exp: 0,
            hp: 0,
            sp: 0.0,
            gold: 0,
            sigils: Vec::new(),
            mastery: BTreeMap::new(),
            mode: Element::None,
            smith_level: 0,
            inventory: BTreeMap::new(),
            equipment: Vec::new(),
            armor: None,
            shield: None,
            accessories: [None, None],
            learned_skills: BTreeSet::new(),
            equipped_skills: BTreeMap::new(),
            sockets: BTreeMap::new(),
            flags: BTreeSet::new(),
            weave_nodes: BTreeSet::new(),
            souls_owned: Vec::new(),
            soul: None,
            spirit_charms: [None, None],
            gem_tiers: BTreeMap::new(),
            tracked_quest: None,
            pacts: BTreeSet::new(),
            kills: BTreeMap::new(),
            play_time: 0.0,
            corruption: 0.0,
            mods: RefCell::new(None),
        }
    }
}

/// Set for scripted and fuzzed test runs so they never touch the player's save slots.
pub static SAVES_DISABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn slot_key(slot: i32) -> String {
    format!("slot{slot}")
}

impl GameState {
    pub fn position(&self) -> Option<Vec2> {
        match (self.x, self.y) {
            (Some(x), Some(y)) => Some(Vec2::new(x, y)),
            _ => None,
        }
    }

    pub fn set_position(&mut self, p: Option<Vec2>) {
        self.x = p.map(|p| p.x);
        self.y = p.map(|p| p.y);
    }

    pub fn acc(&self) -> AccessoryStats {
        let mut stats = AccessoryStats::NONE;
        for id in self.accessories.iter().flatten().chain(self.shield.iter()) {
            if let Some(item) = items::get(id) {
                stats = AccessoryStats::combine(&stats, &item.accessory);
            }
        }
        stats
    }

    pub fn mods(&self) -> std::cell::Ref<'_, WeaveMods> {
        if self.mods.borrow().is_none() {
            *self.mods.borrow_mut() = Some(weave::compute(self.weave_nodes.iter()));
        }
        std::cell::Ref::map(self.mods.borrow(), |m| m.as_ref().unwrap())
    }

    pub fn weave_points(&self) -> i32 {
        let spent = self.weave_nodes.iter().filter(|id| weave::by_id(id).is_some()).count() as i32;
        self.level - 1 + self.count("insight_tome") - spent
    }

    pub fn knots_free(&self) -> i32 {
        let used = self.weave_nodes.iter().filter(|id| weave::by_id(id).is_some_and(|n| n.needs_knot())).count() as i32;
        self.count("weave_knot") - used
    }

    pub fn exp_to_next(&self) -> i32 {
        combat::exp_to_next(self.level)
    }

    pub fn life_shards(&self) -> i32 {
        self.count("life_shard")
    }

    pub fn max_hp(&self) -> i32 {
        ((combat::player_base_hp(self.level) + hp_of(self.armor.as_deref()) as f32)
            * (1.0 + 0.1 * (self.life_shards() / 4) as f32)
            * self.acc().hp
            * self.mods().hp
            * (1.0 - self.corruption / 100.0)) as i32
    }

    pub fn sp_pips(&self) -> i32 {
        (3 + self.count("spirit_pip")).min(6)
    }

    pub fn max_sp(&self) -> f32 {
        self.sp_pips() as f32 * 100.0
    }

    pub fn dodge_charges(&self) -> i32 {
        if self.mods().has("stillwater") {
            return 1;
        }
        3 + self.acc().dodge_charges + self.mods().dodge_charges
    }

    pub fn can_jump(&self) -> bool {
        self.has("rabbit_boots")
    }

    pub fn has_tether(&self) -> bool {
        self.has("aether_tether")
    }

    pub fn crystal_count(&self) -> i32 {
        ["ember_crystal", "tide_crystal", "storm_crystal"].iter().filter(|c| self.has(c)).count() as i32
    }

    /// The blade's power in a mode: grows with level and that mode's mastery, improved by the smith.
    pub fn weapon_atk(&self, mode: Element) -> f32 {
        mastery::blade_power(self.level, self.mastery_level(mode)) * (1.0 + 0.08 * self.smith_level as f32)
    }

    pub fn has_sigil(&self, element: Element) -> bool {
        self.sigils.iter().any(|s| s == element.key())
    }

    pub fn aether_awakened(&self) -> bool {
        self.flags.contains("aether_awakened")
    }

    /// Before the first sigil the blade cannot be wielded at all.
    pub fn dormant(&self) -> bool {
        self.sigils.is_empty()
    }

    /// The opening Aether form's strength: level 1 with one or two sigils, 2 with three or four, 3 once awakened.
    pub fn aether_level(&self) -> i32 {
        if self.flags.contains("debug:skills") || self.aether_awakened() {
            3
        } else if self.sigils.len() >= 3 {
            2
        } else if !self.dormant() {
            1
        } else {
            0
        }
    }

    /// Seconds an Aether state lasts.
    pub fn aether_duration(&self) -> f32 {
        let mut seconds = if self.aether_awakened() { 8.0 } else { 6.0 } + self.mods().aether_time;
        if self.prism("everlight") {
            seconds += 3.0;
        }
        if self.prism("prism_lens") {
            seconds -= 1.5;
        }
        if self.mods().has("aether_bond") {
            seconds *= 0.5;
        }
        seconds.max(2.0)
    }

    /// How much the Aether meter needs to fill: it grows with every sigil and the awakening, and each Aether Conduit
    /// shrinks it.
    pub fn aether_capacity(&self) -> f32 {
        let base = 40.0 + 20.0 * self.sigils.len() as f32 + if self.aether_awakened() { 20.0 } else { 0.0 };
        base * (1.0 - 0.08 * self.count("aether_conduit") as f32).max(0.5)
    }

    /// Seconds without being hit before Aether progress locks in.
    pub fn aether_lock_delay(&self) -> f32 {
        (4.0 - self.acc().aether_lock - if self.mods().has("anchor") { 1.0 } else { 0.0 }).max(1.0)
    }

    /// The chance an Aether hit pierces to a foe's weakness of `element` instead of landing in the current mode's.
    pub fn pierce_chance(&self, element: Element) -> f32 {
        if self.mods().has("aether_bond") {
            return 1.0;
        }
        let chance = 0.25
            + 0.15 * self.mastery_level(element) as f32
            + if self.aether_awakened() { 0.25 } else { 0.0 }
            + if self.has("prism_band") { 0.15 } else { 0.0 }
            + if self.prism("prism_lens") { 0.2 } else { 0.0 }
            + self.mods().pierce;
        chance.min(1.0)
    }

    /// Modes the player can fight in: one per sigil.
    pub fn owned_modes(&self) -> Vec<Element> {
        MODES.iter().copied().filter(|&m| self.has_sigil(m)).collect()
    }

    /// The owned modes plus the Aether page (its opening form and Prism socket), as the Skills menu shows them.
    pub fn blade_pages(&self) -> Vec<Element> {
        let mut pages = self.owned_modes();
        if !self.dormant() {
            pages.push(Element::Aether);
        }
        pages
    }

    pub fn mastery_xp(&self, mode: Element) -> i32 {
        self.mastery.get(mode.key()).copied().unwrap_or(0)
    }

    pub fn mastery_level(&self, mode: Element) -> i32 {
        mastery::level(self.mastery_xp(mode))
    }

    /// Adds mastery experience to a mode and returns its new level if it went up.
    pub fn add_mastery(&mut self, mode: Element, xp: i32) -> Option<i32> {
        if !mastery::trains(mode) || xp <= 0 {
            return None;
        }
        let before = self.mastery_level(mode);
        *self.mastery.entry(mode.key().to_string()).or_default() += xp;
        let after = self.mastery_level(mode);
        (after > before).then_some(after)
    }

    pub fn atk(&self, mode: Element) -> f32 {
        let mods = self.mods();
        (combat::player_base_atk(self.level) + self.weapon_atk(mode))
            * self.acc().atk
            * combat::mode_mods(mode).atk
            * mods.atk
    }

    pub fn def(&self, mode: Element) -> f32 {
        (combat::player_base_def(self.level)
            + power_of(self.armor.as_deref()) as f32
            + power_of(self.shield.as_deref()) as f32)
            * self.acc().def
            * combat::mode_mods(mode).def
            * self.mods().def
    }

    pub fn foc(&self, mode: Element) -> f32 {
        combat::player_base_foc(self.level) * self.acc().foc * combat::mode_mods(mode).foc * self.mods().foc
    }

    pub fn stats(&self, mode: Element) -> CombatStats {
        CombatStats::new(self.level, self.atk(mode), self.def(mode), self.foc(mode))
    }

    pub fn slot_exists(slot: i32) -> bool {
        storage::exists(&slot_key(slot))
    }

    pub fn any_save() -> bool {
        (1..=SLOT_COUNT).any(GameState::slot_exists)
    }

    pub fn new_game(slot: i32) -> GameState {
        let mut state = GameState { slot, ..GameState::default() };
        state.give("dormant_blade", 1);
        state.hp = state.max_hp();
        state
    }

    pub fn load(slot: i32) -> Option<GameState> {
        let text = storage::read(&slot_key(slot))?;
        let mut state: GameState = serde_json::from_str(&text).ok()?;
        if !(OLDEST_SAVE..=SAVE_VERSION).contains(&state.version) {
            return None;
        }
        state.slot = slot;
        state.migrate();
        state.hp = state.hp.clamp(1, state.max_hp().max(1));
        Some(state)
    }

    /// Brings a save from before the Weave and the Aether rework up to date: renamed items, rings turned into orbs,
    /// the neutral mode and its skills gone, and skills matched to mastery. The Weave changed in versions 4 to 7, so
    /// older saves get every point back; version 8 doubled Gemcutter tiers.
    fn migrate(&mut self) {
        if self.version < 7 {
            self.reset_weave();
        }
        // Gemcutter tiers went from two steps to four of half the size; doubling keeps every honed gem as strong.
        if self.version < 8 {
            for tier in self.gem_tiers.values_mut() {
                *tier = (*tier * 2).min(supports::MAX_TIER);
            }
        }
        self.version = SAVE_VERSION;
        for (old, new) in [
            ("circuit_shard", "weave_knot"),
            ("circuit_point", "insight_tome"),
            ("wind_orb", "gale_orb"),
            ("flame_ring", "flame_orb"),
            ("tide_ring", "tide_orb"),
            ("storm_ring", "storm_orb"),
        ] {
            if let Some(n) = self.inventory.remove(old) {
                *self.inventory.entry(new.to_string()).or_default() += n;
            }
        }
        self.mastery.remove(Element::None.key());
        self.learned_skills.retain(|id| skills::get(id).is_some());
        self.equipped_skills.retain(|_, id| skills::get(id).is_some());
        self.sockets.retain(|key, _| !key.starts_with("none."));
        if self.dormant() {
            self.mode = Element::None;
            if !self.has("dormant_blade") {
                self.give("dormant_blade", 1);
            }
        } else if !self.has_sigil(self.mode) {
            self.mode = self.owned_modes()[0];
        }
        self.sync_skills();
    }

    pub fn save(&self) {
        if SAVES_DISABLED.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            storage::write(&slot_key(self.slot), &text);
        }
    }

    /// Adds experience and returns how many levels were gained.
    pub fn add_exp(&mut self, amount: i32) -> i32 {
        if self.level >= combat::MAX_LEVEL {
            return 0;
        }
        self.exp += amount;
        let mut gained = 0;
        while self.level < combat::MAX_LEVEL && self.exp >= self.exp_to_next() {
            self.exp -= self.exp_to_next();
            self.level += 1;
            gained += 1;
        }
        if gained > 0 {
            self.hp = self.max_hp();
        }
        if self.level >= combat::MAX_LEVEL {
            self.exp = 0;
        }
        gained
    }

    pub fn debug_set_level(&mut self, level: i32) {
        self.level = level.clamp(1, combat::MAX_LEVEL);
        self.hp = self.max_hp();
    }

    pub fn count(&self, item: &str) -> i32 {
        self.inventory.get(item).copied().unwrap_or(0)
    }

    /// Adds an item and returns how many actually fit (consumables are capped).
    pub fn give(&mut self, id: &str, count: i32) -> i32 {
        if let Some(sigil) = items::sigil(id) {
            if !self.has_sigil(sigil.element) {
                self.sigils.push(sigil.element.key().to_string());
            }
            self.take("dormant_blade", 1);
            self.mode = sigil.element;
            self.sync_skills();
            return 1;
        }
        if let Some(soul) = id.strip_prefix("soul_").filter(|s| souls::get(s).is_some()) {
            if !self.souls_owned.iter().any(|s| s == soul) {
                self.souls_owned.push(soul.to_string());
            }
            if self.soul.is_none() {
                self.soul = Some(soul.to_string());
            }
            return 1;
        }
        if skills::get(id).is_some() {
            self.learn(id);
            return 1;
        }
        if let Some(item) = items::get(id) {
            match item.kind {
                ItemKind::Armor | ItemKind::Shield | ItemKind::Accessory => {
                    if !self.equipment.iter().any(|e| e == id) {
                        self.equipment.push(id.to_string());
                    }
                    if item.kind == ItemKind::Armor && power_of(self.armor.as_deref()) < item.power {
                        self.armor = Some(id.to_string());
                    }
                    if item.kind == ItemKind::Shield && power_of(self.shield.as_deref()) < item.power {
                        self.shield = Some(id.to_string());
                    }
                    if item.kind == ItemKind::Accessory
                        && !self.accessories.iter().any(|a| a.as_deref() == Some(id))
                        && let Some(free) = self.accessories.iter_mut().find(|a| a.is_none())
                    {
                        *free = Some(id.to_string());
                    }
                    return 1;
                }
                ItemKind::Consumable => {
                    let fits = (items::consumable_cap(id) - self.count(id)).clamp(0, count);
                    if fits > 0 {
                        *self.inventory.entry(id.to_string()).or_default() += fits;
                    }
                    return fits;
                }
                _ => {}
            }
        }
        *self.inventory.entry(id.to_string()).or_default() += count;
        count
    }

    /// Learns a skill, equipping it if its mode has none yet.
    pub fn learn(&mut self, skill_id: &str) {
        let Some(skill) = skills::get(skill_id) else { return };
        if !self.learned_skills.insert(skill_id.to_string()) {
            return;
        }
        self.equipped_skills.entry(skill.element.key().to_string()).or_insert_with(|| skill_id.to_string());
    }

    pub fn equipped_skill(&self, mode: Element) -> Option<&'static SkillDef> {
        self.equipped_skills.get(mode.key()).and_then(|id| skills::get(id))
    }

    /// The highest level the given mode's skills can be charged to: one, plus that element's orb and bracer. For the
    /// Aether page, the level its opening form strikes at.
    pub fn skill_level(&self, mode: Element) -> i32 {
        if mode == Element::Aether {
            return self.aether_level();
        }
        if self.flags.contains("debug:skills") {
            return skills::MAX_LEVEL;
        }
        let Some((orb, bracer)) = items::relics(mode) else { return 1 };
        1 + self.has(orb) as i32 + self.has(bracer) as i32
    }

    /// Learns every skill the blade has grown into and returns the newly learned ones: an element's skills come at
    /// mastery 0, 1 and 3, and the Aether forms with the first sigil, the third sigil and the awakening.
    pub fn sync_skills(&mut self) -> Vec<&'static SkillDef> {
        let mut learned = Vec::new();
        for page in self.blade_pages() {
            let grown = if page == Element::Aether { self.aether_level() } else { self.mastery_level(page) };
            for (i, skill) in skills::for_element(page).enumerate() {
                let needed = if page == Element::Aether { i as i32 + 1 } else { skills::MASTERY_FOR[i] };
                if grown >= needed && !self.learned_skills.contains(skill.id) {
                    self.learn(skill.id);
                    learned.push(skill);
                }
            }
        }
        learned
    }

    /// Equips the next learned skill of a mode, wrapping around.
    pub fn cycle_skill(&mut self, mode: Element) {
        let learned: Vec<&str> =
            skills::for_element(mode).filter(|s| self.learned_skills.contains(s.id)).map(|s| s.id).collect();
        if learned.is_empty() {
            return;
        }
        let current = self.equipped_skills.get(mode.key()).and_then(|id| learned.iter().position(|l| l == id));
        let next = current.map_or(0, |i| (i + 1) % learned.len());
        self.equipped_skills.insert(mode.key().to_string(), learned[next].to_string());
    }

    /// Weaves a node into the lattice.
    pub fn take_node(&mut self, node_id: &str) {
        if weave::by_id(node_id).is_some() && self.weave_nodes.insert(node_id.to_string()) {
            *self.mods.borrow_mut() = None;
        }
    }

    pub fn weave_has(&self, id: &str) -> bool {
        id == weave::HUB || self.weave_nodes.contains(id)
    }

    pub fn can_take(&self, node: &WeaveNode) -> bool {
        !self.weave_has(&node.id)
            && self.weave_points() >= 1
            && (!node.needs_knot() || self.knots_free() > 0)
            && weave::neighbours(node).any(|n| self.weave_has(&n.id))
    }

    /// Unpicks the whole Weave, refunding every point.
    /// Whether a woven node can be unpicked without cutting any other woven node off from the hub.
    pub fn can_drop(&self, node: &WeaveNode) -> bool {
        if !self.weave_nodes.contains(&node.id) {
            return false;
        }
        let hub = weave::by_id(weave::HUB).expect("the Weave has a hub");
        let mut reached = std::collections::HashSet::from([hub.id.as_str()]);
        let mut queue = vec![hub];
        while let Some(at) = queue.pop() {
            for next in weave::neighbours(at) {
                if next.id != node.id && self.weave_nodes.contains(&next.id) && reached.insert(next.id.as_str()) {
                    queue.push(next);
                }
            }
        }
        self.weave_nodes.iter().all(|id| id == &node.id || reached.contains(id.as_str()))
    }

    pub fn drop_node(&mut self, node_id: &str) {
        if self.weave_nodes.remove(node_id) {
            *self.mods.borrow_mut() = None;
        }
    }

    pub fn reset_weave(&mut self) {
        self.weave_nodes.clear();
        *self.mods.borrow_mut() = None;
    }

    pub fn socket_key(mode: Element, group: Group, index: i32) -> String {
        format!("{}.{}.{index}", mode.key(), group.key())
    }

    /// The socket groups a page has: a mode's attack, ranged and special groups, or the Aether page's Prism socket.
    pub fn socket_groups(mode: Element) -> &'static [Group] {
        if mode == Element::Aether { &[Group::Special] } else { &Group::ALL }
    }

    /// Sockets open in a mode's group: one, and a second once the mode reaches adept mastery.
    pub fn socket_count(&self, mode: Element) -> i32 {
        if mode == Element::Aether || self.mastery_level(mode) < mastery::ADEPT { 1 } else { 2 }
    }

    pub fn socketed(&self, mode: Element, group: Group) -> Vec<&'static SupportDef> {
        (0..self.socket_count(mode))
            .filter_map(|i| self.sockets.get(&GameState::socket_key(mode, group, i)))
            .filter_map(|id| supports::get(id))
            .collect()
    }

    pub fn support_mods(&self, mode: Element, group: Group) -> SupportMods {
        SupportMods::from_gems(self.socketed(mode, group).into_iter().map(|g| (g, self.gem_tier(g.id))))
    }

    pub fn gem_tier(&self, id: &str) -> i32 {
        self.gem_tiers.get(id).copied().unwrap_or(0)
    }

    /// A gem's name with its Gemcutter tier.
    /// An item's description; a support gem's numbers include its Gemcutter tier.
    pub fn item_description(&self, id: &str) -> String {
        match supports::get(id) {
            Some(gem) => supports::describe(gem, self.gem_tier(id)),
            None => items::get(id).map_or(String::new(), |i| i.description.to_string()),
        }
    }

    pub fn gem_label(&self, id: &str) -> String {
        supports::get(id).map_or_else(|| items::name_of(id), |g| supports::tier_name(g, self.gem_tier(id)))
    }

    /// Puts a gem in a socket (taking it out of any other first), or empties the socket.
    pub fn set_socket(&mut self, key: &str, gem: Option<&str>) {
        if let Some(gem) = gem {
            self.sockets.retain(|_, g| g != gem);
            self.sockets.insert(key.to_string(), gem.to_string());
        } else {
            self.sockets.remove(key);
        }
    }

    /// Support gems the player owns, in definition order.
    pub fn gems_owned(&self) -> Vec<&'static SupportDef> {
        supports::ALL.iter().filter(|g| self.count(g.id) > 0).collect()
    }

    /// Whether a Prism gem is socketed on the Aether page.
    pub fn prism(&self, id: &str) -> bool {
        self.socketed(Element::Aether, Group::Special).iter().any(|s| s.id == id)
    }

    /// Whether a charm is set into the spirit.
    pub fn charm(&self, id: &str) -> bool {
        self.spirit_charms.iter().any(|c| c.as_deref() == Some(id))
    }

    /// How much of a town's restoration is done, from 0 to 1.
    pub fn restoration(&self, town: &str) -> f32 {
        match data().towns.get(town) {
            Some(def) if !def.rifts.is_empty() => {
                def.rifts.iter().filter(|r| self.flags.contains(&format!("rift:{r}"))).count() as f32
                    / def.rifts.len() as f32
            }
            _ => 0.0,
        }
    }

    pub fn take(&mut self, id: &str, count: i32) -> bool {
        if self.count(id) < count {
            return false;
        }
        let left = self.count(id) - count;
        if left <= 0 {
            self.inventory.remove(id);
        } else {
            self.inventory.insert(id.to_string(), left);
        }
        true
    }

    pub fn has(&self, id: &str) -> bool {
        self.count(id) > 0
            || items::sigil(id).is_some_and(|s| self.has_sigil(s.element))
            || self.equipment.iter().any(|e| e == id)
            || self.learned_skills.contains(id)
            || id.strip_prefix("soul_").is_some_and(|s| self.souls_owned.iter().any(|o| o == s))
    }

    /// Evaluates a condition such as `"met_elder,!has:wind_sigil"`: comma-separated terms that must all hold. A bare word is a
    /// story flag; `has:` checks the inventory, `level:` a minimum level, `gold:` a minimum purse and `crystals:` how
    /// many great crystals are held.
    pub fn check(&self, condition: Option<&str>) -> bool {
        let Some(condition) = condition.filter(|c| !c.trim().is_empty()) else { return true };
        for raw in condition.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let (negate, term) = match raw.strip_prefix('!') {
                Some(t) => (true, t),
                None => (false, raw),
            };
            let int = |s: &str| s.parse::<i32>().unwrap_or(0);
            let result = if let Some(item) = term.strip_prefix("has:") {
                self.has(item)
            } else if let Some(level) = term.strip_prefix("level:") {
                self.level >= int(level)
            } else if let Some(gold) = term.strip_prefix("gold:") {
                self.gold >= int(gold)
            } else if term.starts_with("count:") {
                let c: Vec<&str> = term.split(':').collect();
                c.len() > 2 && self.count(c[1]) >= int(c[2])
            } else if let Some(n) = term.strip_prefix("crystals:") {
                self.crystal_count() >= int(n)
            } else if term.starts_with("restored:") {
                let r: Vec<&str> = term.split(':').collect();
                r.len() > 2 && self.restoration(r[1]) * 100.0 >= int(r[2]) as f32 - 0.01
            } else {
                self.flags.contains(term)
            };
            if result == negate {
                return false;
            }
        }
        true
    }
}

fn power_of(id: Option<&str>) -> i32 {
    id.and_then(items::get).map(|i| i.power).unwrap_or(0)
}

fn hp_of(id: Option<&str>) -> i32 {
    id.and_then(items::get).map(|i| i.hp).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weave_grows_along_threads() {
        let mut state = GameState::new_game(1);
        state.level = 10;
        for id in ["w0_-1", "w0_-2", "w0_-3", "executioner"] {
            let node = weave::by_id(id).unwrap();
            assert!(state.can_take(node), "{id} should be reachable");
            state.take_node(id);
        }
        assert!(!state.can_take(weave::by_id("overcharge").unwrap()), "keystones need a knot");
        state.reset_weave();
        assert_eq!(state.weave_points(), 9);
    }

    #[test]
    fn old_saves_migrate() {
        let mut state = GameState { sigils: vec!["wind".into(), "fire".into()], ..GameState::default() };
        state.inventory.insert("wind_orb".into(), 1);
        state.inventory.insert("circuit_shard".into(), 2);
        state.learned_skills.insert("whirl".into());
        state.equipped_skills.insert("none".into(), "whirl".into());
        state.mastery.insert("none".into(), 5000);
        state.migrate();
        assert_eq!(state.mode, Element::Wind);
        assert_eq!(state.skill_level(Element::Wind), 2);
        assert_eq!(state.knots_free(), 2);
        assert!(!state.learned_skills.contains("whirl") && !state.equipped_skills.contains_key("none"));
        assert_eq!(state.equipped_skill(Element::Fire).map(|s| s.id), Some("cinder_rush"));
    }

    #[test]
    fn the_blade_wakes_with_its_first_sigil() {
        let mut state = GameState::new_game(1);
        assert!(state.dormant() && state.has("dormant_blade"));
        state.give("fire_sigil", 1);
        assert!(!state.has("dormant_blade"));
        assert_eq!(state.mode, Element::Fire);
        assert_eq!(state.equipped_skill(Element::Fire).map(|s| s.id), Some("cinder_rush"));
        assert_eq!(state.skill_level(Element::Fire), 1);
        state.give("flame_orb", 1);
        assert_eq!(state.skill_level(Element::Fire), 2);
        state.mastery.insert("fire".into(), mastery::xp_for(1));
        assert_eq!(state.sync_skills().len(), 1);
    }
}
