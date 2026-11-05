//! Mods etched onto gear with Aether Glyphs: one per item, rerolled by etching it again.

use super::items::{AccessoryStats, ItemKind};
use crate::core::rng::Rng;

pub const GLYPH: &str = "aether_glyph";

pub struct GearMod {
    pub id: &'static str,
    pub name: &'static str,
    pub stats: AccessoryStats,
}

const N: AccessoryStats = AccessoryStats::NONE;

pub static ALL: [GearMod; 12] = [
    GearMod { id: "keen", name: "ATK +8%", stats: AccessoryStats { atk: 1.08, ..N } },
    GearMod { id: "warded", name: "DEF +10%", stats: AccessoryStats { def: 1.1, ..N } },
    GearMod { id: "hale", name: "Life +10%", stats: AccessoryStats { hp: 1.1, ..N } },
    GearMod { id: "focused", name: "FOC +12%", stats: AccessoryStats { foc: 1.12, ..N } },
    GearMod { id: "fleet", name: "Move +6%", stats: AccessoryStats { speed: 1.06, ..N } },
    GearMod { id: "attuned", name: "SP gain +12%", stats: AccessoryStats { sp_gain: 1.12, ..N } },
    GearMod { id: "resonant", name: "Aether +15%", stats: AccessoryStats { aether: 1.15, ..N } },
    GearMod { id: "gilded", name: "Gold +30%", stats: AccessoryStats { gold: 1.3, ..N } },
    GearMod { id: "studious", name: "EXP +12%", stats: AccessoryStats { exp: 1.12, ..N } },
    GearMod { id: "mending", name: "Regen 0.5%/s", stats: AccessoryStats { regen: 0.005, ..N } },
    GearMod { id: "leeching", name: "2% life per kill", stats: AccessoryStats { life_on_kill: 0.02, ..N } },
    GearMod { id: "cleansed", name: "Blight taken -25%", stats: AccessoryStats { blight_taken: 0.75, ..N } },
];

pub fn get(id: &str) -> Option<&'static GearMod> {
    ALL.iter().find(|m| m.id == id)
}

pub fn can_etch(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Armor | ItemKind::Shield | ItemKind::Accessory)
}

/// Picks a mod at random, never the one the item already has.
pub fn roll(rng: &Rng, current: Option<&str>) -> &'static GearMod {
    let pool: Vec<&GearMod> = ALL.iter().filter(|m| Some(m.id) != current).collect();
    pool[rng.below(pool.len() as i32) as usize]
}

/// Chance a cleared post-act boss drops a glyph: 2% at level 60 rising to 20% at level 100.
pub fn glyph_chance(level: i32) -> f32 {
    (0.02 + (level - 60) as f32 * 0.0045).clamp(0.01, 0.2)
}
