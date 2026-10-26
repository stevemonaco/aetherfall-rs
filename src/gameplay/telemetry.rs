//! A balancing log appended as JSON lines, one file per playthrough under `telemetry/` beside the saves, every line
//! stamped with the game version and commit: the hero's level on first entering
//! each zone and on beating each unique encounter (with how long it took), and per-visit zone summaries of damage
//! dealt by element and by attack kind, XP and gold gained, kills, deaths and Aether uptime. Test runs and cutscene worlds log nothing.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};

use crate::core::storage;
use crate::gameplay::combat;
use crate::gameplay::game_state::{GameState, SAVES_DISABLED};

#[derive(Default)]
pub struct Telemetry {
    enabled: bool,
    /// The log file's storage key: the slot and the playthrough's run id.
    file: String,
    slot: i32,
    /// Play time and hero level as of the last update, so events need no state passed in.
    now: f64,
    level: i32,
    visit: Option<Visit>,
    /// Total experience and gold as of the last update; rises between updates count as gains (spending is ignored).
    exp: Option<i64>,
    gold: Option<i32>,
    /// When each running encounter (a rift, a golden breach, a boss) began, by key.
    started: HashMap<String, f64>,
}

#[derive(Default)]
struct Visit {
    map: String,
    area_level: i32,
    entered: f64,
    level: i32,
    kills: i32,
    deaths: i32,
    aether: f64,
    exp: i64,
    gold: i64,
    damage: BTreeMap<String, f64>,
    by_kind: BTreeMap<String, f64>,
}

impl Telemetry {
    pub fn new(enabled: bool, slot: i32, run_id: &str) -> Telemetry {
        let mut telemetry = Telemetry::default();
        telemetry.enabled = enabled && !SAVES_DISABLED.load(std::sync::atomic::Ordering::Relaxed);
        telemetry.file = format!("telemetry/slot{slot}_{run_id}");
        telemetry
    }

    /// A new run id: the time the playthrough began, in seconds since 1970.
    pub fn new_run_id() -> String {
        #[cfg(not(target_arch = "wasm32"))]
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        #[cfg(target_arch = "wasm32")]
        let now = (js_sys::Date::now() / 1000.0) as u64;
        now.to_string()
    }
    fn write(&self, event: &str, mut fields: Value) {
        if !self.enabled {
            return;
        }
        if let Some(map) = fields.as_object_mut() {
            map.insert("event".into(), json!(event));
            map.insert("slot".into(), json!(self.slot));
            map.insert("t".into(), json!((self.now * 10.0).round() / 10.0));
            map.insert("level".into(), json!(self.level));
            map.insert("version".into(), json!(env!("CARGO_PKG_VERSION")));
            map.insert("commit".into(), json!(env!("GAME_COMMIT")));
        }
        storage::append_line(&self.file, &fields.to_string());
    }

    pub fn tick(&mut self, state: &GameState, dt: f32, aether: bool) {
        self.slot = state.slot;
        self.now = state.play_time;
        self.level = state.level;
        let (exp, gold) = (combat::total_exp(state.level, state.exp), state.gold);
        let exp_gain = self.exp.map_or(0, |before| (exp - before).max(0));
        let gold_gain = self.gold.map_or(0, |before| (gold - before).max(0)) as i64;
        (self.exp, self.gold) = (Some(exp), Some(gold));
        if let Some(visit) = self.visit.as_mut() {
            visit.exp += exp_gain;
            visit.gold += gold_gain;
        }
        if aether && let Some(visit) = self.visit.as_mut() {
            visit.aether += dt as f64;
        }
    }

    /// Closes the current zone visit, if any, and opens one for `map`; `first` marks the hero's first time there.
    pub fn enter(&mut self, map: &str, area_level: i32, first: bool) {
        self.leave();
        self.started.clear();
        if first {
            self.write("zone_enter", json!({ "map": map, "area_level": area_level }));
        }
        self.visit =
            Some(Visit { map: map.to_string(), area_level, entered: self.now, level: self.level, ..Visit::default() });
    }

    pub fn leave(&mut self) {
        let Some(v) = self.visit.take() else { return };
        let seconds = self.now - v.entered;
        if seconds < 1.0 {
            return;
        }
        let round = |m: &BTreeMap<String, f64>| {
            m.iter().map(|(k, v)| (k.clone(), json!(v.round()))).collect::<serde_json::Map<_, _>>()
        };
        let fields = json!({
            "map": v.map,
            "area_level": v.area_level,
            "seconds": seconds.round(),
            "level_start": v.level,
            "kills": v.kills,
            "deaths": v.deaths,
            "exp_gained": v.exp,
            "gold_gained": v.gold,
            "aether_seconds": v.aether.round(),
            "damage": round(&v.damage),
            "damage_by_kind": round(&v.by_kind),
        });
        self.write("zone_summary", fields);
    }

    /// Damage the hero dealt to a foe, keyed by element ("aether" while the blade is awake) and attack kind.
    pub fn damage(&mut self, element: &str, kind: &str, amount: i32) {
        if let Some(v) = self.visit.as_mut() {
            *v.damage.entry(element.to_string()).or_default() += amount as f64;
            *v.by_kind.entry(kind.to_string()).or_default() += amount as f64;
        }
    }

    pub fn kill(&mut self) {
        if let Some(v) = self.visit.as_mut() {
            v.kills += 1;
        }
    }

    pub fn death(&mut self) {
        if let Some(v) = self.visit.as_mut() {
            v.deaths += 1;
        }
    }

    /// Marks an encounter as begun; beginning it again (after a collapse or a death) restarts its clock.
    pub fn start(&mut self, key: &str) {
        self.started.insert(key.to_string(), self.now);
    }

    /// Logs a unique encounter beaten: what it was, the hero's level, and how long it took if its start was seen.
    pub fn beaten(&mut self, kind: &str, id: &str, extra: Value) {
        let key = format!("{kind}:{id}");
        let seconds = self.started.remove(&key).map(|t| (self.now - t).round());
        let map = self.visit.as_ref().map_or(String::new(), |v| v.map.clone());
        let mut fields = json!({ "kind": kind, "id": id, "map": map, "seconds": seconds });
        if let (Some(f), Some(e)) = (fields.as_object_mut(), extra.as_object()) {
            f.extend(e.clone());
        }
        self.write("encounter", fields);
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        self.leave();
    }
}
