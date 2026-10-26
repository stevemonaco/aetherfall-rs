//! What each map and region holds for the collection tracker: its chests (and which hold a Gemcutter's Prism or a
//! key treasure) and its rifts, matched against the flags the player has set.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use super::game_data::{WorldNode, data};
use super::game_state::GameState;
use super::items::{self, ItemKind};
use crate::world::map::MapData;

#[derive(Clone, Default)]
struct MapContents {
    chests: Vec<(String, Option<String>)>,
    rifts: Vec<String>,
}

static CONTENTS: LazyLock<Mutex<HashMap<String, MapContents>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn contents(map_id: &str) -> MapContents {
    let mut cache = CONTENTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = cache.get(map_id) {
        return found.clone();
    }
    let mut found = MapContents::default();
    if MapData::exists(map_id) {
        for e in MapData::load(map_id).entities {
            if e.kind == "chest" {
                let flag =
                    e.str("flag").map(str::to_string).unwrap_or_else(|| format!("chest:{map_id}:{}_{}", e.x, e.y));
                found.chests.push((flag, e.str("item").map(str::to_string)));
            } else if e.kind == "rift" {
                let id = e.str("id").map(str::to_string).unwrap_or_else(|| format!("{}_{}", e.x, e.y));
                found.rifts.push(format!("rift:{id}"));
            }
        }
    }
    cache.insert(map_id.to_string(), found.clone());
    found
}

/// Found and total counts of what a map or region holds.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Tally {
    pub chests: (i32, i32),
    pub prisms: (i32, i32),
    pub treasures: (i32, i32),
    pub rifts: (i32, i32),
}

impl Tally {
    fn add(&mut self, other: Tally) {
        let sum = |a: &mut (i32, i32), b: (i32, i32)| *a = (a.0 + b.0, a.1 + b.1);
        sum(&mut self.chests, other.chests);
        sum(&mut self.prisms, other.prisms);
        sum(&mut self.treasures, other.treasures);
        sum(&mut self.rifts, other.rifts);
    }

    pub fn is_empty(&self) -> bool {
        self.chests.1 == 0 && self.rifts.1 == 0
    }

    /// Share of chests opened and rifts sealed, from 0 to 1.
    pub fn progress(&self) -> f32 {
        let total = self.chests.1 + self.rifts.1;
        if total == 0 { 1.0 } else { (self.chests.0 + self.rifts.0) as f32 / total as f32 }
    }
}

/// Whether a chest's item is a key treasure: a sigil, a powerup such as boots or orbs, or a key item.
fn treasure(item: &str) -> bool {
    items::sigil(item).is_some()
        || items::get(item).is_some_and(|i| matches!(i.kind, ItemKind::Powerup | ItemKind::Key | ItemKind::Charm))
        || item.starts_with("soul_")
}

pub fn map_tally(state: &GameState, map_id: &str) -> Tally {
    let found = contents(map_id);
    let mut tally = Tally::default();
    for (flag, item) in &found.chests {
        let opened = state.flags.contains(flag) as i32;
        tally.chests.0 += opened;
        tally.chests.1 += 1;
        if item.as_deref() == Some("gemcutter_prism") {
            tally.prisms.0 += opened;
            tally.prisms.1 += 1;
        } else if item.as_deref().is_some_and(treasure) {
            tally.treasures.0 += opened;
            tally.treasures.1 += 1;
        }
    }
    for flag in &found.rifts {
        tally.rifts.0 += state.flags.contains(flag) as i32;
        tally.rifts.1 += 1;
    }
    tally
}

pub fn region_tally(state: &GameState, node: &WorldNode) -> Tally {
    let mut tally = Tally::default();
    for map in &node.maps {
        tally.add(map_tally(state, map));
    }
    tally
}

/// The regions the player has seen that hold anything to collect, in world-file order.
pub fn regions(state: &GameState) -> Vec<&'static WorldNode> {
    data()
        .world
        .iter()
        .filter(|n| n.maps.iter().any(|m| state.flags.contains(&format!("seen:{m}"))))
        .filter(|n| !region_tally(state, n).is_empty())
        .collect()
}

/// How many of an item the game holds in all: chest and dig contents in every map plus every `give:` action.
pub fn item_total(item: &str) -> i32 {
    static TOTALS: LazyLock<Mutex<HashMap<String, i32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&n) = totals.get(item) {
        return n;
    }
    let mut total = 0;
    let give = format!("give:{item}");
    for (path, text) in crate::gameplay::game_data::files() {
        if path.starts_with("data/maps/") {
            for e in crate::gameplay::game_data::parse::<MapData>(path).entities {
                if e.str("item") == Some(item) {
                    total += e.int("count", 1);
                }
            }
        }
        for (at, _) in text.match_indices(&give) {
            let rest = &text[at + give.len()..];
            if rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            total += rest
                .strip_prefix('*')
                .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok())
                .unwrap_or(1);
        }
    }
    totals.insert(item.to_string(), total);
    total
}
