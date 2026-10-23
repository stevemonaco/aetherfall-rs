//! Rectangles and small vector helpers shared by gameplay and drawing.

use bevy::math::Vec2;

/// An axis-aligned float rectangle for hitboxes.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Box2 {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Box2 {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Box2 {
        Box2 { x, y, w, h }
    }

    pub fn from_center(center: Vec2, w: f32, h: f32) -> Box2 {
        Box2::new(center.x - w / 2.0, center.y - h / 2.0, w, h)
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn intersects(&self, o: &Box2) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    pub fn offset(&self, d: Vec2) -> Box2 {
        Box2::new(self.x + d.x, self.y + d.y, self.w, self.h)
    }

    pub fn inflate(&self, amount: f32) -> Box2 {
        Box2::new(self.x - amount, self.y - amount, self.w + amount * 2.0, self.h + amount * 2.0)
    }

    pub fn to_irect(self) -> IRect {
        IRect::new(self.x.floor() as i32, self.y.floor() as i32, self.w.ceil() as i32, self.h.ceil() as i32)
    }
}

/// An integer rectangle (MonoGame's Rectangle).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> IRect {
        IRect { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn intersects(&self, o: &IRect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub fn inflate(&self, dx: i32, dy: i32) -> IRect {
        IRect::new(self.x - dx, self.y - dy, self.w + dx * 2, self.h + dy * 2)
    }
}

pub fn safe_normalize(v: Vec2) -> Vec2 {
    if v.length_squared() > 0.0001 { v.normalize() } else { Vec2::Y }
}

pub fn safe_dir(v: Vec2, fallback: Vec2) -> Vec2 {
    if v.length_squared() > 0.001 { v.normalize() } else { fallback }
}

pub fn snap(v: Vec2) -> Vec2 {
    Vec2::new(v.x.round(), v.y.round())
}

pub fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
