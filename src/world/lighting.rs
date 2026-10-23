//! A light map multiplied over the world: the area's ambient colour plus soft additive pools for every light source.
//! Areas with a white ambient skip the pass entirely. The platform layer composites the map each frame.

use std::sync::{LazyLock, Mutex};

use bevy::math::Vec2;

use crate::core::color::Rgba;

/// Theme defaults: caves are dim, everything else is lit unless the map says otherwise.
pub fn ambient_for(theme: &str, dark: bool, ambient: Option<&str>) -> Rgba {
    if let Some(hex) = ambient.and_then(|a| u32::from_str_radix(a.trim_start_matches('#'), 16).ok()) {
        return Rgba::hex(hex);
    }
    if dark {
        return Rgba::rgb(28, 26, 48);
    }
    match theme {
        "cave" => Rgba::rgb(120, 118, 160),
        "temple" => Rgba::rgb(150, 175, 190),
        "tower" => Rgba::rgb(150, 140, 190),
        "ash" => Rgba::rgb(225, 195, 175),
        "ruin" => Rgba::rgb(170, 155, 190),
        _ => Rgba::WHITE,
    }
}

/// One frame's light map: ambient plus lights in screen pixels (centre, radius, colour already scaled).
#[derive(Clone, Default)]
pub struct LightFrame {
    pub ambient: Option<Rgba>,
    pub lights: Vec<(Vec2, f32, Rgba, f32)>,
}

static FRAME: LazyLock<Mutex<LightFrame>> = LazyLock::new(|| Mutex::new(LightFrame::default()));

/// Publishes this frame's lighting for the platform layer; `None` ambient means fully lit.
pub fn publish(frame: LightFrame) {
    *FRAME.lock().unwrap_or_else(|e| e.into_inner()) = frame;
}

pub fn current() -> LightFrame {
    FRAME.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
