//! The hero and monsters: movement against the tile grid, the sword swing, simple chase AI and contact damage.

use bevy::prelude::*;

use crate::art::{HeroPose, PersonSprite};
use crate::data::EnemyDef;
use crate::level::Level;
use crate::map::{TILE, TileMap};

const HERO_SPEED: f32 = 92.0;
const SLASH_TIME: f32 = 0.3;
const HURT_TIME: f32 = 0.6;
pub const HERO_LIFE: i32 = 8;

/// An actor's feet position in world pixels (y down) and its collision box size.
#[derive(Component)]
pub struct Body {
    pub feet: Vec2,
    pub width: f32,
    pub height: f32,
}

impl Body {
    pub fn hitbox(&self) -> Rect {
        Rect::new(
            self.feet.x - self.width / 2.0,
            self.feet.y - self.height,
            self.feet.x + self.width / 2.0,
            self.feet.y,
        )
    }
}

/// How far above the feet the sprite's centre sits.
#[derive(Component)]
pub struct SpriteOffset(pub f32);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Down,
    Up,
    Left,
    Right,
}

impl Facing {
    fn vector(self) -> Vec2 {
        match self {
            Facing::Down => Vec2::Y,
            Facing::Up => -Vec2::Y,
            Facing::Left => -Vec2::X,
            Facing::Right => Vec2::X,
        }
    }
}

#[derive(Component)]
pub struct Player {
    pub life: i32,
    attack: Option<f32>,
    swing: u32,
    anim: f32,
    hurt: f32,
    knock: Vec2,
    moving: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self { life: HERO_LIFE, attack: None, swing: 0, anim: 0.0, hurt: 0.0, knock: Vec2::ZERO, moving: false }
    }
}

#[derive(Component)]
pub struct Enemy {
    movement: String,
    speed: f32,
    flying: bool,
    sight: f32,
    life: i32,
    clock: f32,
    hurt: f32,
    knock: Vec2,
    last_swing: u32,
}

impl Enemy {
    pub fn new(def: &EnemyDef) -> Enemy {
        Enemy {
            movement: def.movement.clone(),
            speed: def.speed,
            flying: def.flying,
            sight: def.sight.max(96.0),
            life: (def.hp * 3.0).round().max(1.0) as i32,
            clock: 0.0,
            hurt: 0.0,
            knock: Vec2::ZERO,
            last_swing: 0,
        }
    }
}

pub fn control_player(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    level: Res<Level>,
    player: Single<(&mut Player, &mut Facing, &mut Body)>,
) {
    let dt = time.delta_secs().min(0.05);
    let (mut player, mut facing, mut body) = player.into_inner();
    let pressed = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let mut input = Vec2::ZERO;
    if pressed(KeyCode::ArrowLeft, KeyCode::KeyA) {
        input.x -= 1.0;
    }
    if pressed(KeyCode::ArrowRight, KeyCode::KeyD) {
        input.x += 1.0;
    }
    if pressed(KeyCode::ArrowUp, KeyCode::KeyW) {
        input.y -= 1.0;
    }
    if pressed(KeyCode::ArrowDown, KeyCode::KeyS) {
        input.y += 1.0;
    }

    if player.attack.is_none() && (keys.just_pressed(KeyCode::KeyJ) || keys.just_pressed(KeyCode::KeyZ)) {
        player.attack = Some(0.0);
        player.swing += 1;
    }
    if let Some(t) = player.attack {
        player.attack = (t + dt < SLASH_TIME).then_some(t + dt);
    }

    if input != Vec2::ZERO && player.attack.is_none() {
        *facing = if input.x.abs() > input.y.abs() {
            if input.x < 0.0 { Facing::Left } else { Facing::Right }
        } else if input.y < 0.0 {
            Facing::Up
        } else {
            Facing::Down
        };
    }

    let speed = HERO_SPEED * if player.attack.is_some() { 0.25 } else { 1.0 };
    let knock = player.knock;
    move_body(&level.map, &mut body, input.normalize_or_zero() * speed * dt + knock * dt, false, true);
    player.knock *= (-dt * 10.0).exp();
    player.hurt = (player.hurt - dt).max(0.0);
    player.anim += dt;
    player.moving = input != Vec2::ZERO;
}

pub fn update_enemies(
    mut commands: Commands,
    time: Res<Time>,
    level: Res<Level>,
    mut player: Single<(&mut Player, &Facing, &mut Body), Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Body, &mut Sprite)>,
) {
    let dt = time.delta_secs().min(0.05);
    let (ref mut hero, facing, ref mut hero_body) = *player;
    let slash = hero.attack.filter(|t| (0.05..0.2).contains(t)).map(|_| {
        let center = hero_body.feet - Vec2::new(0.0, 6.0) + facing.vector() * 14.0;
        Rect::from_center_size(center, Vec2::new(24.0, 20.0))
    });

    for (entity, mut enemy, mut body, mut sprite) in &mut enemies {
        enemy.clock += dt;
        enemy.hurt = (enemy.hurt - dt).max(0.0);
        let to_hero = hero_body.feet - body.feet;
        let chasing = to_hero.length() < enemy.sight && enemy.hurt <= 0.0;
        let step = match enemy.movement.as_str() {
            "static" => 0.0,
            // Hoppers dash in short bursts and rest between them.
            "hop" if enemy.clock % 1.2 < 0.35 => enemy.speed * 3.0,
            "hop" => 0.0,
            _ => enemy.speed,
        };
        let mut delta = enemy.knock * dt;
        if chasing {
            delta += to_hero.normalize_or_zero() * step * dt;
            sprite.flip_x = to_hero.x > 0.0;
        }
        let flying = enemy.flying;
        move_body(&level.map, &mut body, delta, flying, false);
        enemy.knock *= (-dt * 8.0).exp();
        sprite.texture_atlas.as_mut().unwrap().index = (enemy.clock / 0.25) as usize % 2;
        sprite.color = if enemy.hurt > 0.0 { Color::srgb(1.0, 0.45, 0.45) } else { Color::WHITE };

        if let Some(slash) = slash
            && enemy.last_swing != hero.swing
            && !slash.intersect(body.hitbox()).is_empty()
        {
            enemy.last_swing = hero.swing;
            enemy.life -= 1;
            enemy.hurt = 0.25;
            enemy.knock = (-to_hero).normalize_or_zero() * 140.0;
            if enemy.life <= 0 {
                info!("defeated an enemy at {:?}", body.feet.round());
                commands.entity(entity).despawn();
                continue;
            }
        }

        if hero.hurt <= 0.0 && !hero_body.hitbox().intersect(body.hitbox()).is_empty() {
            hero.life -= 1;
            hero.hurt = HURT_TIME;
            hero.knock = to_hero.normalize_or_zero() * 160.0;
            if hero.life <= 0 {
                hero.life = HERO_LIFE;
                hero_body.feet = level.spawn;
            }
        }
    }
}

pub fn animate_player(player: Single<(&Player, &Facing, &mut Sprite, &mut Visibility)>) {
    let (player, facing, mut sprite, mut visibility) = player.into_inner();
    let (pose, frame) = if let Some(t) = player.attack {
        (HeroPose::Slash, (t / 0.1) as i32)
    } else if player.hurt > HURT_TIME - 0.2 {
        (HeroPose::Hurt, 0)
    } else if player.knock.length() < 5.0 && player.moving {
        (HeroPose::Run, (player.anim / 0.09) as i32 % 6)
    } else {
        (HeroPose::Idle, (player.anim / 0.5) as i32 % 2)
    };
    let row = match facing {
        Facing::Down => 0,
        Facing::Up => 1,
        Facing::Left | Facing::Right => 2,
    };
    sprite.texture_atlas.as_mut().unwrap().index =
        (row * PersonSprite::columns() + PersonSprite::column(pose, frame)) as usize;
    sprite.flip_x = *facing == Facing::Left;
    *visibility = if player.hurt > 0.0 && (player.hurt * 20.0) as i32 % 2 == 0 {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
}

/// Places every actor's sprite from its feet position, rounded to whole pixels and depth-sorted by y.
pub fn sync_sprites(mut actors: Query<(&Body, &SpriteOffset, &mut Transform), Changed<Body>>) {
    for (body, offset, mut transform) in &mut actors {
        let feet = body.feet.round();
        transform.translation = Vec3::new(feet.x, -(feet.y - offset.0), 10.0 + feet.y * 0.001);
    }
}

/// Moves one pixel at a time on each axis so actors never tunnel, sliding around corners for the hero.
fn move_body(map: &TileMap, body: &mut Body, delta: Vec2, flying: bool, corner_assist: bool) {
    move_axis(map, body, delta.x, true, flying, corner_assist && delta.y == 0.0);
    move_axis(map, body, delta.y, false, flying, corner_assist && delta.x == 0.0);
}

fn move_axis(map: &TileMap, body: &mut Body, mut amount: f32, horizontal: bool, flying: bool, corner_assist: bool) {
    let axis = |v: f32| {
        if horizontal { Vec2::new(v, 0.0) } else { Vec2::new(0.0, v) }
    };
    let across = |v: f32| {
        if horizontal { Vec2::new(0.0, v) } else { Vec2::new(v, 0.0) }
    };
    while amount.abs() > 0.0001 {
        let step = amount.clamp(-1.0, 1.0);
        if !blocked(map, body, axis(step), flying) {
            body.feet += axis(step);
            amount -= step;
            continue;
        }
        if corner_assist {
            for nudge in 1..=6 {
                for sign in [-1.0, 1.0] {
                    let side = across(nudge as f32 * sign);
                    if !blocked(map, body, side + axis(step), flying) && !blocked(map, body, side, flying) {
                        let shift = across(sign * step.abs());
                        if !blocked(map, body, shift, flying) {
                            body.feet += shift;
                        }
                        return;
                    }
                }
            }
        }
        return;
    }
}

// Tiles the mover already overlaps never block it, so it can always step out of a spot it was pushed into.
fn blocked(map: &TileMap, body: &Body, offset: Vec2, flying: bool) -> bool {
    let current = tiles(body.hitbox());
    let moved = body.hitbox();
    let (x0, y0, x1, y1) = tiles(Rect::from_corners(moved.min + offset, moved.max + offset));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let inside = x >= current.0 && x <= current.2 && y >= current.1 && y <= current.3;
            let blocks = if flying { map.blocks_flying(x, y) } else { map.blocks_walking(x, y) };
            if blocks && !inside {
                return true;
            }
        }
    }
    false
}

fn tiles(r: Rect) -> (i32, i32, i32, i32) {
    let t = TILE as f32;
    (
        (r.min.x / t).floor() as i32,
        (r.min.y / t).floor() as i32,
        ((r.max.x - 0.001) / t).floor() as i32,
        ((r.max.y - 0.001) / t).floor() as i32,
    )
}
