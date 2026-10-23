use bevy::math::Vec2;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    #[default]
    Down,
    Up,
    Left,
    Right,
}

impl Direction {
    pub fn to_vec(self) -> Vec2 {
        match self {
            Direction::Up => -Vec2::Y,
            Direction::Down => Vec2::Y,
            Direction::Left => -Vec2::X,
            Direction::Right => Vec2::X,
        }
    }

    pub fn from_vec(v: Vec2, fallback: Direction) -> Direction {
        if v == Vec2::ZERO {
            return fallback;
        }
        if v.x.abs() > v.y.abs() {
            if v.x < 0.0 { Direction::Left } else { Direction::Right }
        } else if v.y < 0.0 {
            Direction::Up
        } else {
            Direction::Down
        }
    }

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }

    pub fn rotation(self) -> f32 {
        use std::f32::consts::{FRAC_PI_2, PI};
        match self {
            Direction::Up => -FRAC_PI_2,
            Direction::Down => FRAC_PI_2,
            Direction::Left => PI,
            Direction::Right => 0.0,
        }
    }

    pub fn parse(s: Option<&str>) -> Direction {
        match s.map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("up" | "north") => Direction::Up,
            Some("left" | "west") => Direction::Left,
            Some("right" | "east") => Direction::Right,
            _ => Direction::Down,
        }
    }

    pub fn from_index(i: i32) -> Direction {
        match i.rem_euclid(4) {
            0 => Direction::Down,
            1 => Direction::Up,
            2 => Direction::Left,
            _ => Direction::Right,
        }
    }
}
