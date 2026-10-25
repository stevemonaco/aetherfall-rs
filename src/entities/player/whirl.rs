//! The Whirlwind gem: holding attack past the first swing turns it into a spin that strikes everything around the hero
//! on every beat while they keep moving. Whirling burns SP and stops when it runs out.

use super::Player;
use crate::core::audio::{self, Sound};
use crate::core::input::{self, GameAction};
use crate::entities::combat_effects::SpinEffect;
use crate::gameplay::combat::HitKind;
use crate::gameplay::supports::Group;
use crate::scenes::world::World;

/// Seconds a swing must be held before it becomes a whirl.
const HOLD: f32 = 0.18;
const BEAT: f32 = 0.22;
const RADIUS: f32 = 30.0;
/// SP burned each second of whirling.
const SP_DRAIN: f32 = 20.0;
/// Movement speed while whirling.
pub(super) const WHIRL_SPEED: f32 = 0.7;

impl Player {
    pub(super) fn whirling(&self) -> bool {
        self.whirl_time >= 0.0
    }

    /// Turns a held swing into a whirl when the Whirlwind gem is socketed.
    pub(super) fn try_whirl(&mut self, world: &World) {
        let ready = self.attack_time >= HOLD && input::held(GameAction::Attack) && world.state.sp > 0.0;
        if !ready || !Self::supports(world, Group::Attack).whirlwind {
            return;
        }
        self.attack_time = -1.0;
        self.combo_step = 0;
        self.combo_buffered = false;
        self.whirl_time = 0.0;
        self.whirl_beat = 0.0;
    }

    pub(super) fn update_whirl(&mut self, world: &mut World, dt: f32) {
        if !input::held(GameAction::Attack) || world.state.sp <= 0.0 || !self.can_act() {
            self.whirl_time = -1.0;
            return;
        }
        self.whirl_time += dt;
        world.state.sp = (world.state.sp - SP_DRAIN * dt).max(0.0);
        self.whirl_beat -= dt;
        if self.whirl_beat > 0.0 {
            return;
        }
        self.whirl_beat = BEAT;
        let center = self.center();
        world.add(SpinEffect::new(center, Self::mode(world), BEAT));
        audio::play(Sound::Swing, 0.7, 0.3);
        self.mow_grass(world, RADIUS);
        self.strike_area(world, center, RADIUS, None, |p, w| p.make_hit(w, 0.55, HitKind::Melee, None, 0.5, 0.5, 80.0));
    }
}
