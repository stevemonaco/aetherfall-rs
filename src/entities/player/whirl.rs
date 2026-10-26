//! The Whirlwind gem: pressing attack goes straight into a spin that strikes everything around the hero on every beat
//! while attack is held and they keep moving. Whirling burns SP; with none left the hero swings normally.

use super::Player;
use crate::core::audio::{self, Sound};
use crate::core::direction::Direction;
use crate::core::input::{self, GameAction};
use crate::entities::combat_effects::SpinEffect;
use crate::gameplay::combat::HitKind;
use crate::gameplay::supports::Group;
use crate::scenes::world::World;

const BEAT: f32 = 0.22;
/// Quarter turns the hero spins through each second while whirling.
const SPIN: f32 = 16.0;
const RADIUS: f32 = 30.0;
/// SP burned each second of whirling.
const SP_DRAIN: f32 = 20.0;
/// Movement speed while whirling.
pub(super) const WHIRL_SPEED: f32 = 0.7;

impl Player {
    pub(super) fn whirling(&self) -> bool {
        self.whirl_time >= 0.0
    }

    /// Whether an attack press should whirl instead of swing: the Whirlwind gem is socketed and SP remains.
    pub(super) fn whirl_ready(world: &World) -> bool {
        world.state.sp > 0.0 && Self::supports(world, Group::Attack).whirlwind
    }

    pub(super) fn start_whirl(&mut self) {
        self.attack_time = -1.0;
        self.combo_step = 0;
        self.combo_buffered = false;
        self.whirl_time = 0.0;
        self.whirl_beat = 0.0;
        self.whirl_facing = self.facing;
    }

    pub(super) fn update_whirl(&mut self, world: &mut World, dt: f32) {
        if !input::held(GameAction::Attack) || world.state.sp <= 0.0 || !self.can_act() {
            self.whirl_time = -1.0;
            self.facing = self.whirl_facing;
            return;
        }
        self.whirl_time += dt;
        const TURN: [Direction; 4] = [Direction::Down, Direction::Left, Direction::Up, Direction::Right];
        self.facing = TURN[(self.whirl_time * SPIN) as usize % 4];
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
