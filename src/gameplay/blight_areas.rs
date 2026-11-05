//! The Blighted expeditions reached through a Blight Gate, and how a finished run is graded.

use super::game_state::GameState;

/// A Blighted expedition reached through a Blight Gate: a self-contained remix with its own grade.
pub struct BlightAreaDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub unlock: &'static str,
    pub map: &'static str,
    pub x: i32,
    pub y: i32,
    pub level: i32,
    pub par_seconds: i32,
}

pub static ALL: [BlightAreaDef; 6] = [
    BlightAreaDef {
        id: "cinder_vault",
        name: "Cinder Vault",
        description: "The Ember Vault, choked with blightfire. What burns stays blighted.",
        unlock: "rift:ash_camp",
        map: "blight_vault",
        x: 19,
        y: 27,
        level: 46,
        par_seconds: 420,
    },
    BlightAreaDef {
        id: "drowned_temple",
        name: "Drowned Temple",
        description: "The Sunken Temple, fully flooded. The tide drowns its halls on a rhythm.",
        unlock: "rift:ash_camp",
        map: "blight_temple",
        x: 4,
        y: 15,
        level: 50,
        par_seconds: 480,
    },
    BlightAreaDef {
        id: "witherwood",
        name: "Witherwood",
        description: "Whisperwood in endless night. Its hunters cannot be seen outside the light.",
        unlock: "rift:ash_camp",
        map: "blight_wood",
        x: 41,
        y: 16,
        level: 54,
        par_seconds: 480,
    },
    BlightAreaDef {
        id: "inverted_spire",
        name: "Inverted Spire",
        description: "The Storm Spire, turned downward. Lightning falls on a beat.",
        unlock: "rift:ash_camp",
        map: "blight_spire",
        x: 19,
        y: 2,
        level: 58,
        par_seconds: 540,
    },
    BlightAreaDef {
        id: "black_tide",
        name: "Black Tide Cove",
        description: "Smuggler's Cove under a dead moon. The ghost ship's guns are still loaded.",
        unlock: "game_clear",
        map: "blight_cove",
        x: 4,
        y: 16,
        level: 64,
        par_seconds: 540,
    },
    BlightAreaDef {
        id: "hollow",
        name: "The Hollow Below",
        description: "Twenty floors beneath the Sleeper's Vault, where the blight began.",
        unlock: "game_clear",
        map: "hollow_1",
        x: 4,
        y: 13,
        level: 70,
        par_seconds: 2400,
    },
];

pub fn get(id: &str) -> Option<&'static BlightAreaDef> {
    ALL.iter().find(|a| a.id == id)
}

pub fn best_grade(state: &GameState, id: &str) -> Option<String> {
    let prefix = format!("grade:{id}=");
    state.flags.iter().find_map(|f| f.strip_prefix(&prefix)).map(str::to_string)
}

/// Grades a finished run from its time against par and the damage taken relative to max life.
pub fn grade(seconds: f32, par: i32, damage: i32, max_hp: i32, heat: i32) -> &'static str {
    let time = if seconds <= par as f32 {
        2
    } else if seconds <= par as f32 * 1.5 {
        1
    } else {
        0
    };
    let hurt = if damage <= max_hp {
        2
    } else if damage <= max_hp * 3 {
        1
    } else {
        0
    };
    match time + hurt + i32::from(heat >= 3) {
        4.. => "S",
        3 => "A",
        2 => "B",
        _ => "C",
    }
}

/// Floors in each leg of the Hollow, each closed by an Echo boss (or the Concord on the last).
pub const HOLLOW_LEG_FLOORS: i32 = 5;
pub const HOLLOW_LEG_PAR: i32 = 180;

/// The grade key for the Hollow leg starting at `floor`.
pub fn hollow_leg_id(floor: i32) -> String {
    format!("hollow@{floor}")
}

/// Hollow floors reached past an Echo boss, which the Blight Gate can start a leg from.
pub fn hollow_checkpoints(state: &GameState) -> Vec<i32> {
    let mut floors: Vec<i32> =
        state.flags.iter().filter_map(|f| f.strip_prefix("hollow_cp:")).filter_map(|n| n.parse().ok()).collect();
    floors.sort_unstable();
    floors
}

pub fn rank(grade: Option<&str>) -> i32 {
    match grade {
        Some("S") => 4,
        Some("A") => 3,
        Some("B") => 2,
        Some("C") => 1,
        _ => 0,
    }
}
