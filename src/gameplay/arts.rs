//! Combat Arts: performed by holding the Art button with an action while in the Art's element mode.

use super::element::Element;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ArtSlot {
    Melee,
    Throw,
    Dodge,
    Guard,
}

impl ArtSlot {
    pub fn key(self) -> &'static str {
        match self {
            ArtSlot::Melee => "melee",
            ArtSlot::Throw => "throw",
            ArtSlot::Dodge => "dodge",
            ArtSlot::Guard => "guard",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ArtDef {
    pub id: &'static str,
    pub name: &'static str,
    pub element: Element,
    pub slot: ArtSlot,
    pub cost: i32,
    pub description: &'static str,
}

const fn art(
    id: &'static str,
    name: &'static str,
    element: Element,
    slot: ArtSlot,
    cost: i32,
    description: &'static str,
) -> ArtDef {
    ArtDef { id, name, element, slot, cost, description }
}

use ArtSlot::*;
use Element as E;

pub const ALL: [ArtDef; 24] = [
    art("whirl", "Whirlwind", E::None, Melee, 1, "Spin, striking everything around you."),
    art("triple_shot", "Triple Shot", E::None, Throw, 1, "Throw three bolts in a fan."),
    art("phantom_step", "Phantom Step", E::None, Dodge, 1, "A long dodge that leaves a decoy enemies attack."),
    art(
        "counter_stance",
        "Counter Stance",
        E::None,
        Guard,
        1,
        "Brace; the next attack you guard is answered with a heavy counter.",
    ),
    art(
        "gale_slash",
        "Gale Slash",
        E::Wind,
        Melee,
        1,
        "A crescent of wind that rolls through foes, hurling them back.",
    ),
    art("cyclone", "Cyclone", E::Wind, Throw, 1, "A slow tornado that drags enemies, fire and gas along with it."),
    art("updraft", "Updraft", E::Wind, Dodge, 1, "A long gust-dash that knocks aside whatever it passes."),
    art(
        "tempest_wall",
        "Tempest Wall",
        E::Wind,
        Guard,
        1,
        "A swirling ward that reflects projectiles for a few seconds.",
    ),
    art("cinder_rush", "Cinder Rush", E::Fire, Melee, 1, "Dash forward slashing three times, leaving a trail of fire."),
    art("meteor", "Meteor", E::Fire, Throw, 1, "Lob a blazing orb that bursts where it lands."),
    art("blaze_step", "Blaze Step", E::Fire, Dodge, 1, "Dodge through a line of flame that ignites the ground."),
    art("flame_aegis", "Flame Aegis", E::Fire, Guard, 1, "Guard inside a ring of fire that scorches nearby foes."),
    art(
        "frost_bloom",
        "Frost Bloom",
        E::Water,
        Melee,
        1,
        "Strike the ground: ice blooms outward, chilling foes and freezing water.",
    ),
    art(
        "glacier_lance",
        "Glacier Lance",
        E::Water,
        Throw,
        1,
        "A piercing lance that freezes the water and ground it crosses.",
    ),
    art("mist_step", "Mist Step", E::Water, Dodge, 1, "Vanish in mist. Enemies lose sight of you for a moment."),
    art("ice_mirror", "Ice Mirror", E::Water, Guard, 1, "An ice shell absorbs one big hit and chills the attacker."),
    art("arc_cleave", "Arc Cleave", E::Thunder, Melee, 1, "A charged slash whose lightning leaps between nearby foes."),
    art(
        "storm_call",
        "Storm Call",
        E::Thunder,
        Throw,
        1,
        "Call lightning down on the aimed point; it spreads through water and metal.",
    ),
    art(
        "arc_step",
        "Arc Step",
        E::Thunder,
        Dodge,
        1,
        "Blink forward as lightning, shocking everything you pass through.",
    ),
    art("static_field", "Static Field", E::Thunder, Guard, 1, "An aura that shocks attackers and nearby foes."),
    art("prism_edge", "Prism Edge", E::Aether, Melee, 3, "Five blades of every element converge on your target."),
    art("prism_ray", "Prism Ray", E::Aether, Throw, 3, "A beam of pure aether that pierces everything in a line."),
    art("flicker", "Flicker", E::Aether, Dodge, 2, "Step outside time: enemies slow to a crawl for three seconds."),
    art("sanctum", "Sanctum", E::Aether, Guard, 3, "A sanctuary that heals you and cleanses blight."),
];

pub fn get(id: &str) -> Option<&'static ArtDef> {
    ALL.iter().find(|a| a.id == id)
}

pub fn for_element(element: Element) -> impl Iterator<Item = &'static ArtDef> {
    ALL.iter().filter(move |a| a.element == element)
}

pub fn slot_key(element: Element, slot: ArtSlot) -> String {
    format!("{}.{}", element.key(), slot.key())
}
