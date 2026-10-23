//! Everything that persists in a save file.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use bevy::math::Vec2;
use serde::{Deserialize, Serialize};

use super::arts::{self, ArtDef, ArtSlot};
use super::circuit::{self, CircuitMods, CircuitNode};
use super::combat::{self, CombatStats};
use super::element::{Element, MODES};
use super::game_data::data;
use super::items::{self, AccessoryStats, ItemKind};
use super::souls;
use crate::core::direction::Direction;
use crate::core::storage;

pub const SAVE_VERSION: i32 = 2;
pub const SLOT_COUNT: i32 = 3;

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
    pub swords: Vec<String>,
    pub mode: Element,
    pub smith_level: i32,
    pub inventory: BTreeMap<String, i32>,
    pub equipment: Vec<String>,
    pub armor: Option<String>,
    pub shield: Option<String>,
    pub accessories: [Option<String>; 2],
    pub learned_arts: BTreeSet<String>,
    pub art_slots: BTreeMap<String, String>,
    pub flags: BTreeSet<String>,
    pub circuit_nodes: BTreeSet<String>,
    pub souls_owned: Vec<String>,
    pub soul: Option<String>,
    pub soul_xp: BTreeMap<String, i32>,
    pub tracked_quest: Option<String>,
    pub pacts: BTreeSet<String>,
    pub kills: BTreeMap<String, i32>,
    pub play_time: f64,
    /// Blight soaked up from blighted ground (0 to 50): a percentage taken off max life until cleansed.
    #[serde(skip)]
    pub corruption: f32,
    #[serde(skip)]
    mods: RefCell<Option<CircuitMods>>,
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
            swords: Vec::new(),
            mode: Element::None,
            smith_level: 0,
            inventory: BTreeMap::new(),
            equipment: Vec::new(),
            armor: None,
            shield: None,
            accessories: [None, None],
            learned_arts: BTreeSet::new(),
            art_slots: BTreeMap::new(),
            flags: BTreeSet::new(),
            circuit_nodes: BTreeSet::new(),
            souls_owned: Vec::new(),
            soul: None,
            soul_xp: BTreeMap::new(),
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

    pub fn mods(&self) -> std::cell::Ref<'_, CircuitMods> {
        if self.mods.borrow().is_none() {
            *self.mods.borrow_mut() = Some(circuit::compute(self.circuit_nodes.iter()));
        }
        std::cell::Ref::map(self.mods.borrow(), |m| m.as_ref().unwrap())
    }

    pub fn circuit_points(&self) -> i32 {
        let spent: i32 = self.circuit_nodes.iter().filter_map(|id| circuit::by_id(id)).map(|n| n.cost).sum();
        self.level - 1 + self.count("circuit_point") - spent
    }

    pub fn shards_free(&self) -> i32 {
        let used = self.circuit_nodes.iter().filter(|id| circuit::by_id(id).is_some_and(|n| n.shard)).count() as i32;
        self.count("circuit_shard") - used
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

    /// The blade's power: the best sword owned, improved by the smith.
    pub fn weapon_atk(&self) -> f32 {
        let best = self.swords.iter().filter_map(|id| items::sword(id)).map(|s| s.atk).max().unwrap_or(3);
        best as f32 * (1.0 + 0.08 * self.smith_level as f32)
    }

    pub fn available_modes(&self) -> Vec<Element> {
        MODES
            .iter()
            .copied()
            .filter(|&m| m == Element::None || self.swords.iter().any(|s| items::sword(s).is_some_and(|d| d.element == m)))
            .collect()
    }

    pub fn atk(&self, mode: Element) -> f32 {
        let mods = self.mods();
        (combat::player_base_atk(self.level) + self.weapon_atk())
            * self.acc().atk
            * combat::mode_mods(mode).atk
            * mods.atk
            * if mods.has(&format!("mode_atk:{}", mode.key())) { 1.05 } else { 1.0 }
    }

    pub fn def(&self, mode: Element) -> f32 {
        (combat::player_base_def(self.level) + power_of(self.armor.as_deref()) as f32 + power_of(self.shield.as_deref()) as f32)
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

    /// Resonance rings speed up an element's ailments and slow its overload.
    pub fn resonates(&self, element: Element) -> bool {
        self.has("prism_band")
            || (element != Element::None && items::all().iter().any(|i| i.ring == element && self.has(i.id)))
    }

    pub fn slot_exists(slot: i32) -> bool {
        storage::exists(&slot_key(slot))
    }

    pub fn any_save() -> bool {
        (1..=SLOT_COUNT).any(GameState::slot_exists)
    }

    pub fn new_game(slot: i32) -> GameState {
        let mut state = GameState { slot, ..GameState::default() };
        state.hp = state.max_hp();
        state
    }

    pub fn load(slot: i32) -> Option<GameState> {
        let text = storage::read(&slot_key(slot))?;
        let mut state: GameState = serde_json::from_str(&text).ok()?;
        if state.version != SAVE_VERSION {
            return None;
        }
        state.slot = slot;
        state.hp = state.hp.clamp(1, state.max_hp().max(1));
        Some(state)
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
        if let Some(sword) = items::sword(id) {
            if !self.swords.iter().any(|s| s == id) {
                self.swords.push(id.to_string());
            }
            let melee: Vec<&str> = arts::for_element(sword.element).filter(|a| a.slot == ArtSlot::Melee).map(|a| a.id).collect();
            for art in melee {
                self.learn(art);
            }
            let node = format!("{}_melee", sword.element.key());
            if circuit::by_id(&node).is_some() {
                self.unlock(&node);
            }
            self.mode = sword.element;
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
        if arts::get(id).is_some() {
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
                    let fits = (items::CONSUMABLE_CAP - self.count(id)).clamp(0, count);
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

    pub fn learn(&mut self, art_id: &str) {
        let Some(art) = arts::get(art_id) else { return };
        if !self.learned_arts.insert(art_id.to_string()) {
            return;
        }
        self.art_slots.entry(arts::slot_key(art.element, art.slot)).or_insert_with(|| art_id.to_string());
    }

    /// Unlocks a Circuit node and applies its effect (learning Arts immediately).
    pub fn unlock(&mut self, node_id: &str) {
        let Some(node) = circuit::by_id(node_id) else { return };
        if !self.circuit_nodes.insert(node_id.to_string()) {
            return;
        }
        *self.mods.borrow_mut() = None;
        for effect in node.effect.split(',') {
            if let Some(art) = effect.strip_prefix("art:") {
                self.learn(art);
            }
        }
    }

    pub fn can_unlock(&self, node: &CircuitNode) -> bool {
        !self.circuit_nodes.contains(&node.id)
            && self.circuit_points() >= node.cost
            && (!node.shard || self.shards_free() > 0)
            && node.requires.iter().all(|r| self.circuit_nodes.contains(r))
            && (node.tree == Element::None || self.available_modes().contains(&node.tree))
    }

    pub fn soul_rank(&self, soul: &str) -> i32 {
        souls::rank(self.soul_xp.get(soul).copied().unwrap_or(0))
    }

    /// How much of a town's restoration is done, from 0 to 1.
    pub fn restoration(&self, town: &str) -> f32 {
        match data().towns.get(town) {
            Some(def) if !def.rifts.is_empty() => {
                def.rifts.iter().filter(|r| self.flags.contains(&format!("rift:{r}"))).count() as f32 / def.rifts.len() as f32
            }
            _ => 0.0,
        }
    }

    pub fn equipped_art(&self, mode: Element, slot: ArtSlot) -> Option<&'static ArtDef> {
        self.art_slots.get(&arts::slot_key(mode, slot)).and_then(|id| arts::get(id))
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
            || self.swords.iter().any(|s| s == id)
            || self.equipment.iter().any(|e| e == id)
            || self.learned_arts.contains(id)
            || id.strip_prefix("soul_").is_some_and(|s| self.souls_owned.iter().any(|o| o == s))
    }

    pub fn flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    /// Evaluates a condition such as `"met_elder,!has:wind"`: comma-separated terms that must all hold. A bare word is a
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
