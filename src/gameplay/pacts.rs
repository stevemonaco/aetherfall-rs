//! Optional curses taken at a Pact shrine. Each adds Heat; Heat multiplies experience, drops and medals.

use super::game_state::GameState;

pub struct PactDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub heat: i32,
    pub unlock: &'static str,
}

pub const ALL: [PactDef; 8] = [
    PactDef { id: "haste", name: "Haste", description: "Enemy wind-ups are 30% shorter.", heat: 1, unlock: "boss_warlock" },
    PactDef {
        id: "volatility",
        name: "Volatility",
        description: "Elemental overload builds twice as fast.",
        heat: 1,
        unlock: "boss_warlock",
    },
    PactDef { id: "abstinence", name: "Abstinence", description: "Healing items cannot be used.", heat: 1, unlock: "boss_serpent" },
    PactDef {
        id: "swarm",
        name: "Swarm",
        description: "Rifts pour out half again as many foes; every third is elite.",
        heat: 2,
        unlock: "boss_serpent",
    },
    PactDef {
        id: "brittle",
        name: "Brittle",
        description: "No perfect guards; only a normal guard remains.",
        heat: 2,
        unlock: "boss_golem",
    },
    PactDef {
        id: "kindling",
        name: "Kindling",
        description: "Fire spreads twice as far, and your burns hurt twice as much.",
        heat: 1,
        unlock: "boss_golem",
    },
    PactDef { id: "ironhide", name: "Ironhide", description: "Enemies have 50% more life.", heat: 2, unlock: "boss_warden" },
    PactDef { id: "glass", name: "Glass Bones", description: "You take 50% more damage.", heat: 3, unlock: "boss_warden" },
];

pub fn heat(state: &GameState) -> i32 {
    ALL.iter().filter(|p| state.pacts.contains(p.id)).map(|p| p.heat).sum()
}

/// Experience and drop multiplier for the current Heat.
pub fn reward_multiplier(state: &GameState) -> f32 {
    1.0 + 0.15 * heat(state) as f32
}

pub fn active(state: &GameState, id: &str) -> bool {
    state.pacts.contains(id)
}
