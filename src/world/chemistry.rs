//! The elemental rules of the world, applied per tile: tall grass burns and spreads, water and metal carry electricity,
//! water freezes into walkable ice and fire melts it. Everything that burns, conducts or freezes goes through here, so
//! combat and puzzles share one rule set. The per-tile state lives in [`Chemistry`]; the rules are `World` methods
//! because they reach into entities.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::math::Vec2;

use super::map::{TILE, Terrain, TileMap, tile_center};
use crate::art::{bosses, combat as art_combat, props};
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, IRect};
use crate::core::gfx::Batch;
use crate::core::noise;
use crate::entities::effects::Particle;
use crate::entities::entity::{Entity, EntityId, PLAYER_ID};
use crate::entities::hazards::SteamCloud;
use crate::entities::pickup::Pickup;
use crate::gameplay::combat::{self, CombatStats, Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::gameplay::pacts;
use crate::scenes::world::World;

const TICK_SECONDS: f32 = 0.1;
const BURN_SECONDS: f32 = 2.6;
const REGROW_SECONDS: f32 = 50.0;

pub struct Chemistry {
    width: i32,
    fire: Vec<f32>,
    shock: Vec<f32>,
    scorched: Vec<f32>,
    cut: Vec<f32>,
    frozen: HashMap<(i32, i32), Terrain>,
    burning: Vec<usize>,
    tick: f32,
    wind: Vec2,
    wind_time: f32,
    pub zone_level: i32,
}

impl Chemistry {
    pub fn new(width: i32, height: i32) -> Chemistry {
        let n = (width * height) as usize;
        Chemistry {
            width,
            fire: vec![0.0; n],
            shock: vec![0.0; n],
            scorched: vec![0.0; n],
            cut: vec![0.0; n],
            frozen: HashMap::new(),
            burning: Vec::new(),
            tick: 0.0,
            wind: Vec2::ZERO,
            wind_time: 0.0,
            zone_level: 1,
        }
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn is_burning(&self, map: &TileMap, x: i32, y: i32) -> bool {
        map.in_bounds(x, y) && self.fire[self.index(x, y)] > 0.0
    }

    pub fn is_electrified(&self, map: &TileMap, x: i32, y: i32) -> bool {
        map.in_bounds(x, y) && self.shock[self.index(x, y)] > 0.0
    }

    pub fn is_frozen(&self, x: i32, y: i32) -> bool {
        self.frozen.contains_key(&(x, y))
    }

    pub fn flammable(&self, map: &TileMap, x: i32, y: i32) -> bool {
        map.in_bounds(x, y)
            && map.at(x, y) == Terrain::TallGrass
            && self.scorched[self.index(x, y)] <= 0.0
            && self.cut[self.index(x, y)] <= 0.0
    }

    /// Burning tiles light up dark areas.
    pub fn lights(&self, map: &TileMap) -> Vec<(Vec2, f32, Rgba)> {
        self.burning
            .iter()
            .map(|&i| (tile_center(i as i32 % map.width, i as i32 / map.width), 30.0, Rgba::rgb(255, 150, 60)))
            .collect()
    }
}

pub fn conductive(t: Terrain) -> bool {
    matches!(t, Terrain::Water | Terrain::Shallows | Terrain::Bog | Terrain::Metal)
}

impl World {
    pub fn is_burning(&self, x: i32, y: i32) -> bool {
        self.chem.is_burning(&self.map, x, y)
    }

    /// Mows standing tall grass; sometimes something was hiding in it.
    pub fn chem_cut(&mut self, x: i32, y: i32) {
        if !self.chem.flammable(&self.map, x, y) || self.is_burning(x, y) {
            return;
        }
        let i = self.chem.index(x, y);
        self.chem.cut[i] = REGROW_SECONDS;
        let at = tile_center(x, y);
        for _ in 0..4 {
            let pos = at + Vec2::new(self.rng.range(-6, 7) as f32, self.rng.range(-4, 5) as f32);
            let vel = Vec2::new(self.rng.range(-30, 31) as f32, -30.0 - self.rng.below(20) as f32);
            self.add(Particle::new(pos, vel, Rgba::rgb(120, 200, 80), 0.4));
        }
        let roll = self.rng.next_f32();
        if roll < 0.06 {
            let amount = (self.state.max_hp() / 12).max(4);
            self.add(Pickup::heart(at + Vec2::new(0.0, 6.0), amount));
        } else if roll < 0.12 {
            let amount = 1 + self.chem.zone_level;
            self.add(Pickup::gold(at + Vec2::new(0.0, 6.0), amount));
        }
    }

    /// Purges blighted ground back to plain earth.
    pub fn chem_cleanse(&mut self, x: i32, y: i32) {
        if !self.map.in_bounds(x, y) || self.map.at(x, y) != Terrain::Blight {
            return;
        }
        self.map.set_terrain(x, y, Terrain::Path);
        Particle::burst(self, tile_center(x, y), Rgba::rgb(240, 230, 255), 6, 40.0, 0.4, false);
    }

    /// Applies an element to a tile, as when a shot lands or an Art strikes the ground.
    pub fn chem_apply(&mut self, x: i32, y: i32, element: Element, strong: bool) {
        if !self.map.in_bounds(x, y) {
            return;
        }
        match element {
            Element::Fire => self.chem_ignite(x, y),
            Element::Water => {
                self.chem_douse(x, y);
                if strong {
                    self.chem_freeze(x, y);
                }
            }
            Element::Thunder => {
                if conductive(self.map.at(x, y)) {
                    self.chem_electrify(x, y, None);
                }
            }
            Element::Wind => self.chem_fan(x, y, None),
            Element::Aether => self.chem_cleanse(x, y),
            Element::None => {}
        }
    }

    /// The lighter touch of a shot passing over a tile: it lights grass and freezes or melts, but makes no steam.
    pub fn chem_brush(&mut self, x: i32, y: i32, element: Element, strong: bool) {
        if !self.map.in_bounds(x, y) {
            return;
        }
        match element {
            Element::Aether => self.chem_cleanse(x, y),
            Element::Fire => {
                if self.chem.is_frozen(x, y) {
                    self.chem_melt(x, y);
                } else if self.chem.flammable(&self.map, x, y) || self.flammables_on(x, y).next().is_some() {
                    self.chem_ignite(x, y);
                }
            }
            Element::Water if strong => {
                self.chem_freeze(x, y);
            }
            Element::Water
                if self.is_burning(x, y) => {
                    self.chem_douse(x, y);
                }
            _ => {}
        }
    }

    fn flammables_on(&self, x: i32, y: i32) -> impl Iterator<Item = EntityId> + '_ {
        self.entities().filter(move |e| e.flammable() && e.base().tile_pos() == (x, y)).map(|e| e.base().id)
    }

    pub fn chem_ignite(&mut self, x: i32, y: i32) {
        if !self.map.in_bounds(x, y) {
            return;
        }
        if self.chem.is_frozen(x, y) {
            self.chem_melt(x, y);
            return;
        }
        if matches!(self.map.at(x, y), Terrain::Water | Terrain::Shallows | Terrain::Bog) {
            if self.rng.next_f32() < 0.5 {
                self.add(SteamCloud::new(tile_center(x, y) + Vec2::new(0.0, 6.0), false));
            }
            return;
        }
        let ids: Vec<EntityId> = self.flammables_on(x, y).collect();
        for id in ids {
            self.with_entity(id, |e, w| e.ignite(w));
        }
        let i = self.chem.index(x, y);
        if self.chem.flammable(&self.map, x, y) && self.chem.fire[i] <= 0.0 {
            self.chem.fire[i] = BURN_SECONDS + self.rng.next_f32() * 0.8;
            self.chem.burning.push(i);
            audio::play(Sound::Burn, 0.35, self.rng.next_f32() * 0.4);
        }
    }

    pub fn chem_douse(&mut self, x: i32, y: i32) {
        for oy in -1..=1 {
            for ox in -1..=1 {
                let (nx, ny) = (x + ox, y + oy);
                if !self.is_burning(nx, ny) {
                    continue;
                }
                let i = self.chem.index(nx, ny);
                self.chem.fire[i] = 0.0;
                self.add(SteamCloud::new(tile_center(nx, ny) + Vec2::new(0.0, 6.0), false));
            }
        }
    }

    /// Turns water into walkable ice until something melts it or the map reloads.
    pub fn chem_freeze(&mut self, x: i32, y: i32) -> bool {
        if !self.map.in_bounds(x, y) || !matches!(self.map.at(x, y), Terrain::Water | Terrain::Shallows) {
            return false;
        }
        self.chem.frozen.insert((x, y), self.map.at(x, y));
        self.map.set_terrain(x, y, Terrain::Ice);
        audio::play(Sound::Clink, 0.35, 0.6);
        Particle::burst(self, tile_center(x, y), Rgba::rgb(220, 245, 255), 4, 25.0, 0.3, false);
        true
    }

    pub fn chem_melt(&mut self, x: i32, y: i32) {
        let Some(original) = self.chem.frozen.remove(&(x, y)) else { return };
        self.map.set_terrain(x, y, original);
        self.add(SteamCloud::new(tile_center(x, y) + Vec2::new(0.0, 6.0), false));
        audio::play(Sound::Burn, 0.4, 0.3);
    }

    /// Sends electricity through every connected conductive tile, hurting what stands on them.
    pub fn chem_electrify(&mut self, x: i32, y: i32, source: Option<CombatStats>) {
        if !self.map.in_bounds(x, y) || !conductive(self.map.at(x, y)) {
            return;
        }
        let mut seen = HashSet::from([self.chem.index(x, y)]);
        let mut queue = VecDeque::from([(x, y)]);
        let mut tiles = Vec::new();
        while let Some(p) = queue.pop_front() {
            if tiles.len() >= 160 {
                break;
            }
            tiles.push(p);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (p.0 + dx, p.1 + dy);
                if self.map.in_bounds(nx, ny) && conductive(self.map.at(nx, ny)) && seen.insert(self.chem.index(nx, ny)) {
                    queue.push_back((nx, ny));
                }
            }
        }

        let stats = source.unwrap_or_else(|| combat::environment(self.chem.zone_level));
        let mut struck = HashSet::new();
        for &(tx, ty) in &tiles {
            let i = self.chem.index(tx, ty);
            self.chem.shock[i] = 0.7;
            if self.rng.next_f32() < 0.35 {
                let pos = tile_center(tx, ty) + Vec2::new(self.rng.range(-6, 7) as f32, self.rng.range(-6, 7) as f32);
                self.add(Particle::new(pos, Vec2::ZERO, Rgba::rgb(255, 245, 160), 0.3).spark());
            }
            for id in self.entities_near(tx, ty) {
                if !struck.insert(id) {
                    continue;
                }
                if self.get(id).is_some_and(|e| e.conductor()) {
                    self.with_entity(id, |e, w| e.energize(w));
                }
                let grounded = self.get(id).is_some_and(|e| e.faction().is_some() && e.grounded());
                if grounded {
                    let mut hit = Hit {
                        power: 0.5,
                        element: Element::Thunder,
                        kind: HitKind::Environment,
                        status_power: 1.4,
                        from: tile_center(tx, ty),
                        knockback: 40.0,
                        can_crit: false,
                        ..Hit::new(if source.is_some() { Faction::Player } else { Faction::Neutral }, stats)
                    };
                    self.hit(id, &mut hit);
                }
            }
        }
        audio::play(Sound::Clink, 0.8, -0.6);
        self.flash(Rgba::rgb(255, 250, 200), 0.1);
    }

    /// Wind fans flames onward and blows gas away.
    pub fn chem_fan(&mut self, x: i32, y: i32, direction: Option<Vec2>) {
        if let Some(d) = direction {
            self.chem.wind = d;
            self.chem.wind_time = 2.0;
        }
        for oy in -1..=1 {
            for ox in -1..=1 {
                if self.is_burning(x + ox, y + oy) {
                    let i = self.chem.index(x + ox, y + oy);
                    self.chem.fire[i] += 0.6;
                }
            }
        }
    }

    pub fn update_chemistry(&mut self, dt: f32) {
        self.chem.wind_time = (self.chem.wind_time - dt).max(0.0);
        for s in self.chem.shock.iter_mut().filter(|s| **s > 0.0) {
            *s -= dt;
        }
        self.chem.tick += dt;
        if self.chem.tick < TICK_SECONDS {
            return;
        }
        self.chem.tick -= TICK_SECONDS;
        for i in 0..self.chem.scorched.len() {
            if self.chem.scorched[i] > 0.0 {
                self.chem.scorched[i] -= TICK_SECONDS;
            }
            if self.chem.cut[i] > 0.0 {
                self.chem.cut[i] -= TICK_SECONDS;
            }
        }
        for index in self.chem.burning.clone() {
            let (x, y) = (index as i32 % self.map.width, index as i32 / self.map.width);
            self.chem.fire[index] -= TICK_SECONDS;
            if self.chem.fire[index] <= 0.0 {
                self.chem.fire[index] = 0.0;
                self.chem.scorched[index] = REGROW_SECONDS;
                self.chem.burning.retain(|&b| b != index);
                if self.map_blighted() && self.rng.next_f32() < 0.6 {
                    self.map.set_terrain(x, y, Terrain::Blight);
                }
                continue;
            }
            self.spread_fire(x, y);
            self.burn_occupants(x, y);
            if self.rng.next_f32() < 0.5 {
                let pos = tile_center(x, y) + Vec2::new(self.rng.range(-6, 7) as f32, self.rng.range(-2, 6) as f32);
                let color = if self.rng.next_f32() < 0.5 { Rgba::rgb(255, 200, 70) } else { Rgba::rgb(255, 110, 40) };
                self.add(Particle::new(pos, Vec2::new(0.0, -24.0), color, 0.5));
            }
        }
    }

    fn spread_fire(&mut self, x: i32, y: i32) {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let mut chance = if pacts::active(&self.state, "kindling") { 0.18 } else { 0.09 };
            if self.chem.wind_time > 0.0 {
                chance *= 1.0 + self.chem.wind.dot(Vec2::new(dx as f32, dy as f32)) * 2.5;
            }
            if self.rng.next_f32() >= chance {
                continue;
            }
            let (nx, ny) = (x + dx, y + dy);
            if self.chem.flammable(&self.map, nx, ny) && !self.is_burning(nx, ny) {
                self.chem_ignite(nx, ny);
            }
            let ids: Vec<EntityId> = self.flammables_on(nx, ny).collect();
            for id in ids {
                self.with_entity(id, |e, w| e.ignite(w));
            }
        }
    }

    fn burn_occupants(&mut self, x: i32, y: i32) {
        let stats = combat::environment(self.chem.zone_level);
        for id in self.entities_on_tile(x, y) {
            if self.get(id).is_some_and(|e| e.flammable()) {
                self.with_entity(id, |e, w| e.ignite(w));
            }
            let grounded = self.get(id).is_some_and(|e| e.faction().is_some() && e.grounded());
            if grounded && self.rng.next_f32() < 0.35 {
                let mut hit = Hit {
                    power: 0.25,
                    element: Element::Fire,
                    kind: HitKind::Environment,
                    status_power: 0.7,
                    from: tile_center(x, y),
                    knockback: 0.0,
                    can_crit: false,
                    ..Hit::new(Faction::Neutral, stats)
                };
                self.hit(id, &mut hit);
            }
        }
    }

    fn entities_near(&self, x: i32, y: i32) -> Vec<EntityId> {
        let area = Box2::new((x * TILE - 2) as f32, (y * TILE - 2) as f32, (TILE + 4) as f32, (TILE + 4) as f32);
        let mut out: Vec<EntityId> = self
            .entities()
            .filter(|e| e.base().tile_pos() == (x, y) || (e.conductor() && e.base().hitbox().intersects(&area)))
            .map(|e| e.base().id)
            .collect();
        if self.player.as_ref().is_some_and(|p| p.base().tile_pos() == (x, y)) {
            out.push(PLAYER_ID);
        }
        out
    }

    pub fn draw_chemistry(&self, batch: &mut Batch, view: IRect) {
        let (map, chem, time) = (&self.map, &self.chem, self.time);
        let x0 = (view.x / TILE - 1).max(0);
        let x1 = (view.right() / TILE + 1).min(map.width - 1);
        let y0 = (view.y / TILE - 1).max(0);
        let y1 = (view.bottom() / TILE + 1).min(map.height - 1);
        for &(px, py) in chem.frozen.keys() {
            if px >= x0 && px <= x1 && py >= y0 && py <= y1 {
                batch.draw(props::ice_tile(), Vec2::new((px * TILE) as f32, (py * TILE) as f32), Rgba::WHITE);
            }
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                let i = chem.index(x, y);
                let at = Vec2::new((x * TILE) as f32, (y * TILE) as f32);
                let (fire, scorched, cut) = (chem.fire[i], chem.scorched[i], chem.cut[i]);
                if map.at(x, y) == Terrain::TallGrass {
                    if scorched > 0.0 || fire > 0.0 {
                        let alpha = if fire > 0.0 { 1.0 } else { (scorched / 5.0).min(1.0) };
                        batch.draw(art_combat::scorch(), at, Rgba::WHITE.mul(alpha));
                    }
                    let variant = (noise::hash(x, y, 9) % 3) as i32 * 16;
                    if cut > 0.0 && fire <= 0.0 && scorched <= 0.0 {
                        batch.draw_src(art_combat::tall_grass(), at + Vec2::new(0.0, 8.0), Some(IRect::new(variant, 14, 16, 6)), Rgba::WHITE);
                    } else if fire <= 0.0 && scorched <= 0.0 {
                        let sway = ((time as f32 * 2.2 + x as f32 * 0.7 + y as f32 * 0.3).sin() * 1.2) as i32;
                        batch.draw_src(art_combat::tall_grass(), at + Vec2::new(sway as f32, -4.0), Some(IRect::new(variant, 0, 16, 20)), Rgba::WHITE);
                    }
                }
                if fire > 0.0 {
                    let frame = ((time * 10.0) as u32).wrapping_add(noise::hash(x, y, 3)) % 4;
                    batch.draw_src(art_combat::flames(), at + Vec2::new(0.0, -6.0), Some(IRect::new(frame as i32 * 16, 0, 16, 22)), Rgba::WHITE);
                }
                if map.at(x, y) == Terrain::Blight {
                    let frame = ((time * 4.0) as u32).wrapping_add(noise::hash(x, y, 5)) % 4;
                    batch.draw_src(bosses::blight_tile(), at, Some(IRect::new(frame as i32 * 16, 0, 16, 16)), Rgba::WHITE);
                }
                if chem.shock[i] > 0.0 && (time * 30.0) as i32 + x + y >= 0 && ((time * 30.0) as i32 + x + y) % 3 == 0 {
                    batch.draw(art_combat::spark_tile(), at, Rgba::WHITE.mul((chem.shock[i] * 2.0).min(1.0)));
                }
            }
        }
    }
}
