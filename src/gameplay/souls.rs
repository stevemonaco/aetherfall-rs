//! Freed souls that travel with the player: an always-on passive and an active on a cooldown.

use crate::core::color::Rgba;

#[derive(Clone, Copy, Debug)]
pub struct SoulDef {
    pub id: &'static str,
    pub name: &'static str,
    pub passive: &'static str,
    pub active: &'static str,
    pub cooldown: f32,
    pub glow: Rgba,
}

pub const ALL: [SoulDef; 7] = [
    SoulDef {
        id: "hound",
        name: "Hound",
        passive: "Sniffs out buried treasure; dig where it barks.",
        active: "Sic 'em: lunges at the nearest foe.",
        cooldown: 12.0,
        glow: Rgba::rgb(240, 210, 140),
    },
    SoulDef {
        id: "hawk",
        name: "Hawk",
        passive: "Spots hidden passages and secret walls.",
        active: "Mark Prey: foes in sight take more damage for 8 seconds.",
        cooldown: 20.0,
        glow: Rgba::rgb(230, 170, 110),
    },
    SoulDef {
        id: "lantern",
        name: "Lantern Wisp",
        passive: "Lights the dark around you and reveals cloaked foes.",
        active: "Flash: blinds and staggers nearby foes.",
        cooldown: 14.0,
        glow: Rgba::rgb(255, 230, 150),
    },
    SoulDef {
        id: "guardian",
        name: "Guardian",
        passive: "Turns aside one projectile every 10 seconds.",
        active: "Aegis: projectiles can't touch you for 3 seconds.",
        cooldown: 22.0,
        glow: Rgba::rgb(170, 210, 255),
    },
    SoulDef {
        id: "ember",
        name: "Ember Sprite",
        passive: "Your dodges set grass alight and fire ailments build faster.",
        active: "Flare: a burst of fire around the sprite.",
        cooldown: 12.0,
        glow: Rgba::rgb(255, 140, 60),
    },
    SoulDef {
        id: "stormling",
        name: "Stormling",
        passive: "Thunder ailments build faster.",
        active: "Strike: calls lightning onto the nearest foe.",
        cooldown: 10.0,
        glow: Rgba::rgb(250, 235, 120),
    },
    SoulDef {
        id: "echo",
        name: "Echo",
        passive: "Remembers the last Combat Art you performed.",
        active: "Reprise: performs it again at no SP cost.",
        cooldown: 40.0,
        glow: Rgba::rgb(220, 190, 255),
    },
];

pub fn get(id: &str) -> Option<&'static SoulDef> {
    ALL.iter().find(|s| s.id == id)
}

pub fn rank(xp: i32) -> i32 {
    if xp >= 30 {
        3
    } else if xp >= 10 {
        2
    } else {
        1
    }
}
