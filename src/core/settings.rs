//! Player preferences, stored beside the save slots.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

use super::{audio, storage};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default, rename_all = "PascalCase")]
pub struct Settings {
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub screen_shake: bool,
    pub fullscreen: bool,
    pub damage_numbers: bool,
    /// Name of the gamepad to read; empty picks whichever pad was used last.
    pub controller: String,
    /// With the mouse aiming, melee swings at the cursor instead of the way the hero last moved.
    pub melee_toward_cursor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            music_volume: 0.55,
            sfx_volume: 0.8,
            screen_shake: true,
            fullscreen: false,
            damage_numbers: true,
            controller: String::new(),
            melee_toward_cursor: false,
        }
    }
}

static CURRENT: LazyLock<Mutex<Settings>> = LazyLock::new(|| Mutex::new(Settings::default()));

/// Set for test runs so they never overwrite the player's settings.
pub static SAVES_DISABLED: AtomicBool = AtomicBool::new(false);

pub fn current() -> Settings {
    CURRENT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn set(settings: Settings) {
    *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = settings;
}

pub fn load() {
    let loaded = storage::read("settings").and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    set(loaded);
    apply();
}

pub fn save() {
    if !SAVES_DISABLED.load(Ordering::Relaxed)
        && let Ok(text) = serde_json::to_string_pretty(&current())
    {
        storage::write("settings", &text);
    }
    apply();
}

fn apply() {
    let s = current();
    audio::set_volumes(s.sfx_volume, s.music_volume);
}
