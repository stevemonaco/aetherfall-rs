//! Trial mods: hardships rolled each time the hero steps into a Trial Rift. Every mod makes the trial harder and its
//! foes more rewarding.

use crate::core::rng::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrialMod {
    /// Foes move and attack faster.
    Haste,
    /// Foes burst when they die.
    Volatile,
    /// The ground erupts under the hero every few seconds.
    Eruptions,
    /// Foes have more life.
    Ironhide,
    /// Every fourth foe a rift pours out is an elite.
    EliteTide,
    /// Foes heal when they hit the hero.
    Bloodthirst,
}

const ALL: [TrialMod; 6] = [
    TrialMod::Haste,
    TrialMod::Volatile,
    TrialMod::Eruptions,
    TrialMod::Ironhide,
    TrialMod::EliteTide,
    TrialMod::Bloodthirst,
];

/// Extra EXP and gold each active mod adds.
pub const REWARD_PER_MOD: f32 = 0.25;

impl TrialMod {
    pub fn label(self) -> &'static str {
        match self {
            TrialMod::Haste => "Haste",
            TrialMod::Volatile => "Volatile",
            TrialMod::Eruptions => "Eruptions",
            TrialMod::Ironhide => "Ironhide",
            TrialMod::EliteTide => "Elite Tide",
            TrialMod::Bloodthirst => "Bloodthirst",
        }
    }
}

/// Picks `count` different mods.
pub fn roll(rng: &Rng, count: usize) -> Vec<TrialMod> {
    let mut pool = ALL.to_vec();
    rng.shuffle(&mut pool);
    pool.truncate(count);
    pool
}
