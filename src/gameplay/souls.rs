//! The guardian spirit that wakes with the hero. It fights at their side on its own; every beast freed from a rift
//! teaches it a new form (an auto-attack and an exploration perk), and charms set into it change every form.

use crate::core::color::Rgba;

#[derive(Clone, Copy, Debug)]
pub struct SoulDef {
    pub id: &'static str,
    pub name: &'static str,
    pub attack: &'static str,
    pub perk: &'static str,
    /// Seconds between attacks before any Weave bonus.
    pub interval: f32,
    pub glow: Rgba,
}

/// The spirit's own shape, before it has learned any beast.
pub const SPIRIT: SoulDef = SoulDef {
    id: "spirit",
    name: "Guardian Spirit",
    attack: "Zaps the nearest foe now and then.",
    perk: "No perk of its own; free beasts from rifts to teach it new forms.",
    interval: 2.0,
    glow: Rgba::rgb(200, 220, 255),
};

pub const ALL: [SoulDef; 4] = [
    SoulDef {
        id: "hound",
        name: "Hound",
        attack: "Bounds over and bites the nearest foe.",
        perk: "Sniffs out buried treasure; dig where it barks.",
        interval: 1.6,
        glow: Rgba::rgb(240, 210, 140),
    },
    SoulDef {
        id: "hawk",
        name: "Hawk",
        attack: "Dives at a foe and marks it to take more damage.",
        perk: "Sees paths no one else can: hidden crossings appear while it flies with you.",
        interval: 2.4,
        glow: Rgba::rgb(230, 170, 110),
    },
    SoulDef {
        id: "lantern",
        name: "Lantern Wisp",
        attack: "Flings motes of light at the nearest foe.",
        perk: "Lights the dark around you and reveals cloaked foes.",
        interval: 1.4,
        glow: Rgba::rgb(255, 230, 150),
    },
    SoulDef {
        id: "ember",
        name: "Ember Sprite",
        attack: "Spits fireballs that set foes and grass alight.",
        perk: "Burns away thorny thickets you walk up to; fire ailments build faster.",
        interval: 1.8,
        glow: Rgba::rgb(255, 140, 60),
    },
];

/// Charms set into the spirit, and the freed souls that leave each one behind.
pub const CHARM_SLOTS: usize = 2;

/// A form by id; "spirit" (or nothing) is the spirit's own shape.
pub fn get(id: &str) -> Option<&'static SoulDef> {
    if id == SPIRIT.id {
        return Some(&SPIRIT);
    }
    ALL.iter().find(|s| s.id == id)
}
