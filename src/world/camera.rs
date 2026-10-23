use bevy::math::Vec2;

use crate::core::geom::IRect;
use crate::core::rng::Rng;

pub struct Camera {
    pub position: Vec2,
    pub view_width: i32,
    pub view_height: i32,
    exact: Vec2,
    shake_time: f32,
    shake_strength: f32,
    rng: Rng,
    pub screen_shake: bool,
}

impl Camera {
    pub fn new(view_width: i32, view_height: i32) -> Camera {
        Camera {
            position: Vec2::ZERO,
            view_width,
            view_height,
            exact: Vec2::ZERO,
            shake_time: 0.0,
            shake_strength: 0.0,
            rng: Rng::new(17),
            screen_shake: true,
        }
    }

    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 {
        self.position + screen
    }

    pub fn view(&self) -> IRect {
        IRect::new(self.position.x as i32, self.position.y as i32, self.view_width, self.view_height)
    }

    /// The draw offset for world-space drawing, including shake.
    pub fn offset(&self) -> Vec2 {
        let shake = if self.shake_time > 0.0 {
            Vec2::new(self.rng.range(-1, 2) as f32, self.rng.range(-1, 2) as f32) * self.shake_strength
        } else {
            Vec2::ZERO
        };
        Vec2::new(-self.position.x.round() + shake.x, -self.position.y.round() + shake.y)
    }

    /// Eases toward the target; a zero `dt` snaps straight to it.
    pub fn follow(&mut self, target: Vec2, world_width: i32, world_height: i32, dt: f32) {
        let (vw, vh) = (self.view_width as f32, self.view_height as f32);
        let (ww, wh) = (world_width as f32, world_height as f32);
        let mut desired = target - Vec2::new(vw / 2.0, vh / 2.0);
        desired.x = if ww <= vw { (ww - vw) / 2.0 } else { desired.x.clamp(0.0, ww - vw) };
        desired.y = if wh <= vh { (wh - vh) / 2.0 } else { desired.y.clamp(0.0, wh - vh) };
        self.exact = if dt <= 0.0 { desired } else { self.exact.lerp(desired, 1.0 - (-dt * 9.0).exp()) };
        if self.exact.distance_squared(desired) < 0.25 {
            self.exact = desired;
        }
        self.position = Vec2::new(self.exact.x.round(), self.exact.y.round());
        self.shake_time = (self.shake_time - dt).max(0.0);
    }

    pub fn shake(&mut self, seconds: f32, strength: f32) {
        if !self.screen_shake {
            return;
        }
        self.shake_time = self.shake_time.max(seconds);
        self.shake_strength = strength;
    }
}
