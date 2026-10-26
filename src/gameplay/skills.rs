//! Skills: one is equipped per mode and fired with the skill button. Tapping performs level 1; holding slows the world
//! and charges toward levels 2 and 3, each costing one more SP pip. Aether "skills" are forms instead: the equipped
//! one fires by itself when the Aether state wakes, at the level the blade has grown to.

use super::element::Element;

pub const MAX_LEVEL: i32 = 3;
/// Real seconds of holding needed to reach each level (index = level - 1).
pub const CHARGE_TIMES: [f32; 3] = [0.0, 0.4, 0.9];
/// Holding past this many real seconds releases the skill at the highest level reached.
pub const MAX_HOLD: f32 = 1.5;
/// A press shorter than this is a tap and never slows the world.
pub const TAP_WINDOW: f32 = 0.15;

#[derive(Clone, Copy, Debug)]
pub struct SkillDef {
    pub id: &'static str,
    pub name: &'static str,
    pub element: Element,
    pub description: &'static str,
    /// What each level adds, shown in the menu.
    pub levels: [&'static str; 3],
}

const fn skill(
    id: &'static str,
    name: &'static str,
    element: Element,
    description: &'static str,
    levels: [&'static str; 3],
) -> SkillDef {
    SkillDef { id, name, element, description, levels }
}

use Element as E;

/// The mastery an element's first, second and third skills are learned at.
pub const MASTERY_FOR: [i32; 3] = [0, 1, 3];

pub const ALL: [SkillDef; 15] = [
    skill(
        "gale_slash",
        "Gale Slash",
        E::Wind,
        "A crescent of wind that rolls through foes, hurling them back.",
        ["One crescent.", "A larger, stronger crescent.", "Three crescents in a fan."],
    ),
    skill(
        "cyclone",
        "Cyclone",
        E::Wind,
        "A slow tornado that drags enemies, fire and gas along with it.",
        ["One tornado.", "A larger tornado that lasts longer.", "Two tornadoes."],
    ),
    skill(
        "updraft",
        "Updraft",
        E::Wind,
        "A gust-dash that knocks aside whatever it passes.",
        ["A short gust.", "A longer gust.", "A long gust that leaves a tornado behind."],
    ),
    skill(
        "cinder_rush",
        "Cinder Rush",
        E::Fire,
        "Dash forward slashing, leaving a trail of fire.",
        ["Three slashes.", "Five slashes.", "Five slashes, ending in an explosion."],
    ),
    skill(
        "meteor",
        "Meteor",
        E::Fire,
        "Lob blazing orbs that burst where they land.",
        ["One orb.", "Three orbs.", "A shower of seven orbs."],
    ),
    skill(
        "flame_aegis",
        "Flame Aegis",
        E::Fire,
        "A ring of fire scorches nearby foes.",
        [
            "A small ring.",
            "A wider ring that burns away projectiles for a few seconds.",
            "A great ring that ignites the ground and burns projectiles longer.",
        ],
    ),
    skill(
        "frost_bloom",
        "Frost Bloom",
        E::Water,
        "Strike the ground: ice blooms outward, chilling foes and freezing water.",
        ["A small bloom.", "A wide bloom.", "A vast bloom that pulses twice."],
    ),
    skill(
        "glacier_lance",
        "Glacier Lance",
        E::Water,
        "A piercing lance that freezes the water and ground it crosses.",
        ["One lance.", "Three lances.", "One enormous lance."],
    ),
    skill(
        "ice_mirror",
        "Ice Mirror",
        E::Water,
        "An ice shell absorbs a hit and chills the attacker.",
        [
            "Absorbs one hit.",
            "Lasts longer; breaking releases a freezing nova.",
            "Absorbs two hits, each releasing a nova.",
        ],
    ),
    skill(
        "arc_cleave",
        "Arc Cleave",
        E::Thunder,
        "A charged slash whose lightning leaps between nearby foes.",
        ["Leaps four times.", "Leaps seven times, harder.", "Leaps ten times and calls lightning on the first foe."],
    ),
    skill(
        "storm_call",
        "Storm Call",
        E::Thunder,
        "Call lightning on the aimed point; it spreads through water and metal.",
        ["One strike.", "Three strikes.", "Five strikes."],
    ),
    skill(
        "static_field",
        "Static Field",
        E::Thunder,
        "An aura that shocks attackers and nearby foes.",
        ["A short aura.", "A longer, wider aura.", "A long, wide aura that hits harder."],
    ),
    skill(
        "prism_edge",
        "Prism Edge",
        E::Aether,
        "When Aether wakes, blades fly at the foes nearest you.",
        ["Three blades.", "Four blades.", "Six blades and a burst of pure aether."],
    ),
    skill(
        "prism_ray",
        "Prism Ray",
        E::Aether,
        "When Aether wakes, a beam pierces everything along your aim.",
        ["One beam.", "A wider, stronger beam.", "Three beams."],
    ),
    skill(
        "sanctum",
        "Sanctum",
        E::Aether,
        "When Aether wakes you heal, and while it lasts an aura softens blows, keeps ailments and blight off you, and pulses every 2s to cleanse you and strike nearby foes.",
        ["Heals 6%; blows land at 70%.", "Heals 10%; blows land at 60%.", "Heals 15%; blows land at 50%."],
    ),
];

pub fn get(id: &str) -> Option<&'static SkillDef> {
    ALL.iter().find(|s| s.id == id)
}

pub fn for_element(element: Element) -> impl Iterator<Item = &'static SkillDef> {
    ALL.iter().filter(move |s| s.element == element)
}

/// The level a charge has reached after `held` real seconds.
pub fn level_for_hold(held: f32) -> i32 {
    CHARGE_TIMES.iter().rposition(|&t| held >= t).map_or(1, |i| i as i32 + 1)
}
