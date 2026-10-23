//! Anything thrown, shot or lobbed. Player bolts bounce off walls; every projectile applies its element to the tile it
//! stops on (burning grass, electrifying water, and so on).

use bevy::math::Vec2;

use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, IRect, snap};
use crate::core::gfx::{self, Batch, TexId};
use crate::entities::effects::Particle;
use crate::entities::entity::{Entity, EntityBase, EntityId};
use crate::entity_base;
use crate::gameplay::combat::{CombatStats, Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::scenes::world::World;
use crate::world::chemistry::conductive;
use crate::world::map::TILE;

/// What happens when a projectile hits or expires, beyond the standard behaviour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectileHook {
    None,
    /// A player shot: hits feed the player's SP and overload.
    PlayerShot,
    /// The Meteor Art: bursts into flame where it lands.
    Meteor,
}

#[derive(Clone)]
pub struct Projectile {
    pub base: EntityBase,
    pub owner: Faction,
    pub attacker: CombatStats,
    pub power: f32,
    pub element: Element,
    pub kind: HitKind,
    pub poise: f32,
    pub status_power: f32,
    pub knockback: f32,
    pub velocity: Vec2,
    pub lifetime: f32,
    pub piercing: bool,
    pub bounces: i32,
    pub ignores_walls: bool,
    pub reflectable: bool,
    pub unblockable: bool,
    pub size: f32,
    /// Elevation above the ground shadow, so shots read as flying.
    pub z: f32,
    /// When set, the shot arcs to its target over its lifetime and only hits on landing.
    pub lob_height: f32,
    pub texture: TexId,
    pub frame_width: i32,
    pub frame_rate: f32,
    pub spin: f32,
    pub rotate_to_velocity: bool,
    pub tint: Rgba,
    pub trail_color: Rgba,
    pub scale: f32,
    /// Applies the element to every tile crossed, not just where it stops (charged shots, lances).
    pub paints_path: bool,
    pub hook: ProjectileHook,
    already_hit: Vec<EntityId>,
    age: f32,
    last_tile: (i32, i32),
}

impl Projectile {
    pub fn new(owner: Faction, attacker: CombatStats, pos: Vec2, velocity: Vec2) -> Projectile {
        Projectile {
            base: EntityBase::at(pos),
            owner,
            attacker,
            power: 1.0,
            element: Element::None,
            kind: HitKind::Attack,
            poise: 1.0,
            status_power: 1.0,
            knockback: 90.0,
            velocity,
            lifetime: 1.0,
            piercing: false,
            bounces: 0,
            ignores_walls: false,
            reflectable: true,
            unblockable: false,
            size: 6.0,
            z: 6.0,
            lob_height: 0.0,
            texture: props::orb(),
            frame_width: 0,
            frame_rate: 12.0,
            spin: 0.0,
            rotate_to_velocity: false,
            tint: Rgba::WHITE,
            trail_color: Rgba::TRANSPARENT,
            scale: 1.0,
            paints_path: false,
            hook: ProjectileHook::None,
            already_hit: Vec::new(),
            age: 0.0,
            last_tile: (-1, -1),
        }
    }

    /// A standard enemy shot of the given kind.
    pub fn enemy_shot(
        kind: &str,
        from: Vec2,
        direction: Vec2,
        attacker: CombatStats,
        power: f32,
        speed_scale: f32,
    ) -> Projectile {
        let (tint, speed, size, element) = match kind {
            "spore" => (Rgba::rgb(220, 140, 255), 70.0, 6.0, Element::None),
            "seed" => (Rgba::rgb(180, 255, 120), 100.0, 5.0, Element::None),
            "bone" => (Rgba::rgb(240, 236, 220), 95.0, 6.0, Element::None),
            "fire" => (Rgba::rgb(255, 150, 60), 90.0, 7.0, Element::Fire),
            "bubble" => (Rgba::rgb(150, 220, 255), 70.0, 7.0, Element::Water),
            "ice" => (Rgba::rgb(190, 240, 255), 105.0, 6.0, Element::Water),
            "spark" => (Rgba::rgb(255, 240, 120), 120.0, 6.0, Element::Thunder),
            "gust" => (Rgba::rgb(170, 250, 230), 110.0, 8.0, Element::Wind),
            "dark" => (Rgba::rgb(190, 120, 255), 90.0, 7.0, Element::None),
            "aether" => (Rgba::rgb(255, 255, 255), 95.0, 7.0, Element::Aether),
            _ => (Rgba::rgb(255, 120, 120), 85.0, 6.0, Element::None),
        };
        let mut shot = Projectile::new(Faction::Enemy, attacker, from, direction * speed * speed_scale);
        shot.power = power;
        shot.lifetime = 3.5;
        shot.size = size;
        shot.tint = tint;
        shot.element = element;
        shot.texture = if kind == "fire" { props::fireball() } else { props::orb() };
        shot.frame_width = if kind == "fire" { 12 } else { 0 };
        shot.trail_color = if matches!(kind, "fire" | "spark" | "aether") { tint } else { Rgba::TRANSPARENT };
        shot
    }

    pub fn damage_box(&self) -> Box2 {
        Box2::new(self.base.pos.x - self.size / 2.0, self.base.pos.y - self.size / 2.0 - self.z, self.size, self.size)
    }

    fn can_hit(&self, id: EntityId, faction: Faction) -> bool {
        faction != self.owner
            && !(faction == Faction::Neutral && self.owner != Faction::Player)
            && !self.already_hit.contains(&id)
    }

    /// Hits a target; returns whether it connected. Applies any reaction (reflect, expire) the target asked for.
    fn strike(&mut self, world: &mut World, id: EntityId, faction: Faction) -> bool {
        let mut hit = Hit {
            power: self.power,
            element: self.element,
            kind: self.kind,
            poise: self.poise,
            status_power: self.status_power,
            knockback: self.knockback,
            projectile: true,
            unblockable: self.unblockable,
            from: self.base.pos - self.velocity * 0.05,
            owner: Some(self.base.id),
            ..Hit::new(self.owner, self.attacker)
        };
        let connected = world.hit(id, &mut hit);
        if connected && self.hook == ProjectileHook::PlayerShot {
            world.with_player(|p, w| p.on_hit_dealt(w, faction, &hit));
        }
        if let Some((faction, stats, dir)) = hit.reflect {
            self.reflect(faction, stats, dir);
            return false;
        }
        if hit.expire_owner {
            self.expire(world);
        }
        connected
    }

    fn hit_around(&mut self, world: &mut World, radius: f32) {
        for (id, faction, hurtbox) in world.damageables() {
            if self.can_hit(id, faction)
                && hurtbox.center().distance(self.base.pos - Vec2::new(0.0, 4.0)) < radius + hurtbox.w / 2.0
            {
                self.already_hit.push(id);
                self.strike(world, id, faction);
            }
        }
    }

    /// Moves one frame, bouncing off walls; returns false if the projectile expired against one.
    fn advance(&mut self, world: &mut World, dt: f32) -> bool {
        let next = self.base.pos + self.velocity * dt;
        if self.ignores_walls || !blocked(world, next) {
            self.base.pos = next;
            return true;
        }
        if self.bounces <= 0 {
            self.base.pos = next;
            self.expire(world);
            return false;
        }
        self.bounces -= 1;
        let block_x = blocked(world, Vec2::new(next.x, self.base.pos.y));
        let block_y = blocked(world, Vec2::new(self.base.pos.x, next.y));
        if block_x || !block_y {
            self.velocity.x = -self.velocity.x;
        }
        if block_y || !block_x {
            self.velocity.y = -self.velocity.y;
        }
        self.already_hit.clear();
        audio::play(Sound::Clink, 0.35, 0.5);
        let color = if self.trail_color.a > 0 { self.trail_color } else { self.tint };
        Particle::burst(world, self.base.pos - Vec2::new(0.0, self.z), color, 4, 30.0, 0.2, false);
        true
    }

    /// Sends the shot back at whoever fired it, now owned by `new_owner`.
    pub fn reflect(&mut self, new_owner: Faction, stats: CombatStats, direction: Vec2) {
        self.owner = new_owner;
        self.attacker = stats;
        self.velocity = direction * self.velocity.length() * 1.2;
        self.already_hit.clear();
        self.age = 0.0;
    }

    pub fn expire(&mut self, world: &mut World) {
        if self.base.removed {
            return;
        }
        self.base.removed = true;
        let t = TILE as f32;
        let p = self.base.pos;
        let mut tile = ((p.x / t).floor() as i32, ((p.y - 2.0) / t).floor() as i32);
        if world.map.blocks_flying(tile.0, tile.1) {
            tile = (
                ((p.x - self.velocity.x * 0.02) / t).floor() as i32,
                ((p.y - self.velocity.y * 0.02 - 2.0) / t).floor() as i32,
            );
        }
        world.chem_apply(tile.0, tile.1, self.element, matches!(self.kind, HitKind::ChargedBolt | HitKind::Skill));
        let color = if self.trail_color.a > 0 { self.trail_color } else { self.tint };
        Particle::burst(world, p - Vec2::new(0.0, self.z), color, 5, 30.0, 0.25, false);
        if self.hook == ProjectileHook::Meteor {
            world.with_player(|player, w| player.meteor_burst(w, p));
        }
    }
}

fn blocked(world: &World, p: Vec2) -> bool {
    let t = TILE as f32;
    world.map.blocks_flying((p.x / t).floor() as i32, ((p.y - 2.0) / t).floor() as i32)
}

impl Entity for Projectile {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn light(&self, _world: &World) -> Option<(f32, Rgba)> {
        (self.trail_color.a > 0).then_some((18.0 * self.scale, self.trail_color))
    }

    fn added(&mut self, _world: &mut World) {
        self.base.width = self.size;
        self.base.height = self.size;
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.age += dt;
        if self.lob_height > 0.0 {
            let t = (self.age / self.lifetime).min(1.0);
            self.z = (t * std::f32::consts::PI).sin() * self.lob_height + 4.0;
            self.base.pos += self.velocity * dt;
            if t >= 1.0 {
                self.hit_around(world, self.size);
                self.expire(world);
            }
            return;
        }

        if !self.advance(world, dt) {
            return;
        }
        if self.age >= self.lifetime {
            self.expire(world);
            return;
        }

        let t = TILE as f32;
        let tile = ((self.base.pos.x / t).floor() as i32, ((self.base.pos.y - 2.0) / t).floor() as i32);
        if tile != self.last_tile && matches!(self.element, Element::Fire | Element::Water) {
            self.last_tile = tile;
            world.chem_brush(tile.0, tile.1, self.element, self.paints_path);
        }
        // Lightning grounds itself in the first water or metal it crosses, electrifying all of it.
        if self.element == Element::Thunder && conductive(world.map.at(tile.0, tile.1)) {
            let source = (self.owner == Faction::Player).then_some(self.attacker);
            world.chem_electrify(tile.0, tile.1, source);
            self.base.removed = true;
            return;
        }

        if self.trail_color.a > 0 && world.rng.next_f32() < 0.6 {
            let jitter = Vec2::new(world.rng.range(-2, 3) as f32, world.rng.range(-2, 3) as f32);
            world.add(Particle::new(
                self.base.pos - Vec2::new(0.0, self.z) + jitter,
                -self.velocity * 0.1,
                self.trail_color,
                0.3,
            ));
        }

        let damage_box = self.damage_box();
        for (id, faction, hurtbox) in world.damageables() {
            if !self.can_hit(id, faction) || !hurtbox.intersects(&damage_box) {
                continue;
            }
            self.already_hit.push(id);
            if self.strike(world, id, faction) && !self.piercing {
                self.expire(world);
                return;
            }
            if self.base.removed {
                return;
            }
        }
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        let shadow_alpha = if self.lob_height > 0.0 { 0.6 } else { 1.0 };
        batch.draw(props::shadow(8), snap(self.base.pos - Vec2::new(4.0, 1.0)), Rgba::WHITE.mul(shadow_alpha));
        let (tw, th) = gfx::size(self.texture);
        let frames = if self.frame_width > 0 { tw / self.frame_width } else { 1 };
        let fw = if self.frame_width > 0 { self.frame_width } else { tw };
        let src = IRect::new((self.age * self.frame_rate) as i32 % frames.max(1) * fw, 0, fw, th);
        let rotation =
            if self.rotate_to_velocity { self.velocity.y.atan2(self.velocity.x) } else { self.age * self.spin };
        batch.draw_ex(
            self.texture,
            snap(self.base.pos - Vec2::new(0.0, self.z)),
            Some(src),
            self.tint,
            rotation,
            Vec2::new(fw as f32 / 2.0, th as f32 / 2.0),
            Vec2::splat(self.scale),
            false,
            false,
        );
    }
}
