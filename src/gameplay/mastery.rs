//! Element mastery: each mode grows from fighting in it, strengthening its blade and opening its support sockets.

use super::element::Element;

pub const MAX_MASTERY: i32 = 5;
const THRESHOLDS: [i32; MAX_MASTERY as usize] = [500, 3000, 10000, 30000, 70000];
/// The mastery at which a mode's second support sockets open and Aether can strike that element's weakness.
pub const ADEPT: i32 = 3;

pub fn level(xp: i32) -> i32 {
    THRESHOLDS.iter().take_while(|&&t| xp >= t).count() as i32
}

/// The experience at which a mastery level is reached.
pub fn xp_for(level: i32) -> i32 {
    if level <= 0 { 0 } else { THRESHOLDS[level.min(MAX_MASTERY) as usize - 1] }
}

/// Progress from the current level toward the next, from 0 to 1 (1 once mastered).
pub fn progress(xp: i32) -> f32 {
    let lvl = level(xp);
    if lvl >= MAX_MASTERY {
        return 1.0;
    }
    let from = if lvl == 0 { 0 } else { THRESHOLDS[lvl as usize - 1] };
    (xp - from) as f32 / (THRESHOLDS[lvl as usize] - from) as f32
}

/// Whether a mode earns mastery. Aether borrows its strength from the elements instead.
pub fn trains(mode: Element) -> bool {
    mode != Element::Aether
}

/// The blade's base power in a mode before the smith's work.
pub fn blade_power(player_level: i32, mastery: i32) -> f32 {
    (4.0 + player_level as f32) * (1.0 + 0.08 * mastery as f32)
}
