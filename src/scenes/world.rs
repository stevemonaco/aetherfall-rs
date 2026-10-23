//! The live game world: the map, its entities and the player, plus every service entities use (movement and
//! collision, messages, dialogue actions, rewards, warps). WorldScene wraps it as the bottom of the scene stack.

use std::collections::VecDeque;

use bevy::math::Vec2;

use super::dialogue::{Ask, DialogueScene, Then};
use super::scene::Scene;
use super::spawn;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::direction::Direction;
use crate::core::geom::{Box2, IRect};
use crate::core::gfx::{self, Batch, TexId};
use crate::core::input::{self, GameAction};
use crate::core::rng::Rng;
use crate::core::{font, palette, settings};
use crate::entities::effects::{FloatingText, Particle, Puff};
use crate::entities::entity::{Entity, EntityId, PLAYER_ID};
use crate::entities::pickup::Pickup;
use crate::entities::player::Player;
use crate::gameplay::arts;
use crate::gameplay::combat::{self, Faction, Hit};
use crate::gameplay::element::Element;
use crate::gameplay::game_data::{EnemyDef, data};
use crate::gameplay::game_state::GameState;
use crate::gameplay::items::{self, ItemKind};
use crate::gameplay::pacts;
use crate::gameplay::rank::CombatRank;
use crate::gameplay::souls::{self, SoulDef};
use crate::world::ambience::Ambience;
use crate::world::baker;
use crate::world::camera::Camera;
use crate::world::chemistry::Chemistry;
use crate::world::lighting::{self, LightFrame};
use crate::world::map::{MapData, TILE, TileMap, tile_center, tile_feet};
use crate::world::theme::TerrainTheme;
use crate::world::water::WaterLayer;

pub const SCREEN_WIDTH: i32 = 480;
pub const SCREEN_HEIGHT: i32 = 270;
const FADE_SECONDS: f32 = 0.3;
const CHUNK: i32 = 256;

/// What other entities may know about the player, refreshed whenever the player moves.
#[derive(Clone, Debug, Default)]
pub struct HeroInfo {
    pub pos: Vec2,
    pub center: Vec2,
    pub hitbox: Box2,
    pub hurtbox: Box2,
    pub facing: Direction,
    pub airborne: bool,
    pub dead: bool,
    pub hidden: bool,
    pub z: f32,
    pub aim: Vec2,
}

/// Something the world asks its scene to open.
pub enum WorldRequest {
    Menu,
    Map,
    GameOver(Box<GameState>),
    Ending,
}

/// Builds an overlay scene once it is its turn to show.
pub type Overlay = Box<dyn FnOnce(&mut World) -> Box<dyn Scene>>;

/// What a defeated enemy leaves behind.
pub struct KillInfo {
    pub def: &'static EnemyDef,
    pub level: i32,
    pub pos: Vec2,
    pub boss: bool,
    pub blighted: bool,
}

pub struct World {
    pub state: GameState,
    pub data: MapData,
    pub map: TileMap,
    pub chem: Chemistry,
    pub rank: CombatRank,
    pub camera: Camera,
    pub rng: Rng,
    pub hero: HeroInfo,
    pub player: Option<Box<Player>>,
    pub boss: Option<EntityId>,
    pub decoy: Option<EntityId>,
    pub companion: Option<EntityId>,
    pub toasts: Vec<(String, f32)>,
    pub requests: Vec<WorldRequest>,
    pub auto_save: bool,
    pub time: f64,
    pub frame_delta: f32,
    entities: Vec<Option<Box<dyn Entity>>>,
    pending: Vec<Box<dyn Entity>>,
    dormant: Vec<crate::world::map::EntityData>,
    overlays: VecDeque<Overlay>,
    terrain: Vec<(IRect, TexId)>,
    water: Option<WaterLayer>,
    ambience: Ambience,
    next_id: EntityId,
    fade: f32,
    banner_time: f32,
    hit_stop: f32,
    slow_time: f32,
    slow_scale: f32,
    enemy_slow: f32,
    pending_warp: Option<(String, (i32, i32), Direction)>,
    pending_era: bool,
    fading_out: bool,
    show_hitboxes: bool,
    game_over: bool,
    flash_color: Rgba,
    flash_time: f32,
    flash_length: f32,
    checkpoint: Option<GameState>,
    ambient: Rgba,
    saved_flash: f32,
}

impl World {
    pub fn new(state: GameState, auto_save: bool) -> World {
        World {
            state,
            data: MapData::default(),
            map: TileMap::new(&MapData { rows: vec![".".into()], ..MapData::default() }).unwrap(),
            chem: Chemistry::new(1, 1),
            rank: CombatRank::default(),
            camera: Camera::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            rng: Rng::from_time(),
            hero: HeroInfo::default(),
            player: None,
            boss: None,
            decoy: None,
            companion: None,
            toasts: Vec::new(),
            requests: Vec::new(),
            auto_save,
            time: 0.0,
            frame_delta: 0.0,
            entities: Vec::new(),
            pending: Vec::new(),
            dormant: Vec::new(),
            overlays: VecDeque::new(),
            terrain: Vec::new(),
            water: None,
            ambience: Ambience::new(None, ""),
            next_id: PLAYER_ID + 1,
            fade: 1.0,
            banner_time: 0.0,
            hit_stop: 0.0,
            slow_time: 0.0,
            slow_scale: 1.0,
            enemy_slow: 0.0,
            pending_warp: None,
            pending_era: false,
            fading_out: false,
            show_hitboxes: false,
            game_over: false,
            flash_color: Rgba::WHITE,
            flash_time: 0.0,
            flash_length: 1.0,
            checkpoint: None,
            ambient: Rgba::WHITE,
            saved_flash: 0.0,
        }
    }

    pub fn enter(&mut self) {
        let id = self.state.map_id.clone();
        let (pos, facing) = (self.state.position(), self.state.facing);
        self.load_map(&id, pos, facing);
    }

    pub fn map_name(&self) -> &str {
        &self.data.name
    }

    pub fn map_id(&self) -> &str {
        &self.data.id
    }

    pub fn theme_name(&self) -> &str {
        &self.data.theme
    }

    pub fn lava_pits(&self) -> bool {
        self.data.lava
    }

    pub fn map_heat(&self) -> f32 {
        self.data.heat
    }

    pub fn map_blighted(&self) -> bool {
        self.data.blighted
    }

    pub fn dark(&self) -> bool {
        self.data.dark
    }

    pub fn input_locked(&self) -> bool {
        self.fading_out || self.pending_warp.is_some() || self.fade > 0.5
    }

    pub fn enemy_time_scale(&self) -> f32 {
        if self.enemy_slow > 0.0 { 0.25 } else { 1.0 }
    }

    /// How far from the player foes can be seen on a cloaked map, or None when the map is not cloaked.
    pub fn cloak_radius(&self) -> Option<f32> {
        self.data.cloaked.then(|| {
            56.0 + if self.state.soul.as_deref() == Some("lantern") { 44.0 } else { 0.0 }
                + if self.state.accessories.iter().any(|a| a.as_deref() == Some("night_eye")) { 36.0 } else { 0.0 }
        })
    }

    // ---- Entities ---------------------------------------------------------------------------------------------

    /// Adds an entity; it joins the world (and gets `added`) at the end of the current update.
    pub fn add(&mut self, entity: impl Entity) -> EntityId {
        self.add_boxed(Box::new(entity))
    }

    pub fn add_boxed(&mut self, mut entity: Box<dyn Entity>) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        entity.base_mut().id = id;
        self.pending.push(entity);
        id
    }

    fn flush_pending(&mut self) {
        while !self.pending.is_empty() {
            for mut entity in std::mem::take(&mut self.pending) {
                entity.added(self);
                self.entities.push(Some(entity));
            }
        }
    }

    /// Every live entity other than the player and whichever entity is running right now.
    pub fn entities(&self) -> impl Iterator<Item = &dyn Entity> {
        self.entities.iter().flatten().map(|e| e.as_ref()).filter(|e| !e.base().removed)
    }

    pub fn entities_of<T: Entity>(&self) -> impl Iterator<Item = &T> {
        self.entities().filter_map(|e| e.as_any().downcast_ref::<T>())
    }

    pub fn get(&self, id: EntityId) -> Option<&dyn Entity> {
        if id == PLAYER_ID {
            return self.player.as_deref().map(|p| p as &dyn Entity);
        }
        self.entities.iter().flatten().find(|e| e.base().id == id).map(|e| e.as_ref())
    }

    pub fn get_as<T: Entity>(&self, id: EntityId) -> Option<&T> {
        self.get(id).and_then(|e| e.as_any().downcast_ref::<T>())
    }

    pub fn ids_of<T: Entity>(&self) -> Vec<EntityId> {
        self.entities_of::<T>().map(|e| e.base().id).collect()
    }

    /// Runs `f` with exclusive access to one entity (the player included) and to the rest of the world.
    pub fn with_entity<R>(&mut self, id: EntityId, f: impl FnOnce(&mut dyn Entity, &mut World) -> R) -> Option<R> {
        if id == PLAYER_ID {
            let mut player = self.player.take()?;
            let result = f(player.as_mut(), self);
            self.player = Some(player);
            self.refresh_hero();
            return Some(result);
        }
        let index = self.entities.iter().position(|e| e.as_ref().is_some_and(|e| e.base().id == id))?;
        let mut entity = self.entities[index].take()?;
        let result = f(entity.as_mut(), self);
        self.entities[index] = Some(entity);
        Some(result)
    }

    pub fn with_as<T: Entity, R>(&mut self, id: EntityId, f: impl FnOnce(&mut T, &mut World) -> R) -> Option<R> {
        self.with_entity(id, |e, w| e.as_any_mut().downcast_mut::<T>().map(|t| f(t, w))).flatten()
    }

    pub fn with_player<R>(&mut self, f: impl FnOnce(&mut Player, &mut World) -> R) -> Option<R> {
        let mut player = self.player.take()?;
        let result = f(&mut player, self);
        self.player = Some(player);
        self.refresh_hero();
        Some(result)
    }

    pub fn refresh_hero(&mut self) {
        if let Some(p) = &self.player {
            self.hero = p.info();
        }
    }

    /// Ids of everything that can be hit, with faction and hurtbox, the player first.
    pub fn damageables(&self) -> Vec<(EntityId, Faction, Box2)> {
        let mut out = Vec::new();
        if let Some(p) = &self.player {
            out.push((PLAYER_ID, Faction::Player, p.hurtbox()));
        }
        for e in self.entities() {
            if let Some(f) = e.faction() {
                out.push((e.base().id, f, e.hurtbox()));
            }
        }
        out
    }

    /// Delivers a hit to an entity (or the player) and returns whether it connected.
    pub fn hit(&mut self, id: EntityId, hit: &mut Hit) -> bool {
        self.with_entity(id, |e, w| if e.base().removed { false } else { e.take_hit(w, hit) }).unwrap_or(false)
    }

    /// Hits the player; returns whether it connected.
    pub fn hit_player(&mut self, hit: &mut Hit) -> bool {
        self.hit(PLAYER_ID, hit)
    }

    pub fn entities_on_tile(&self, x: i32, y: i32) -> Vec<EntityId> {
        let mut out: Vec<EntityId> =
            self.entities().filter(|e| e.base().tile_pos() == (x, y)).map(|e| e.base().id).collect();
        if let Some(p) = &self.player
            && p.base().tile_pos() == (x, y)
        {
            out.push(PLAYER_ID);
        }
        out
    }

    // ---- Maps ---------------------------------------------------------------------------------------------------

    pub fn load_map(&mut self, id: &str, position: Option<Vec2>, facing: Direction) {
        self.data = MapData::load(id);
        self.map = TileMap::new(&self.data).unwrap_or_else(|e| panic!("map {id}: {e}"));
        self.chem = Chemistry::new(self.map.width, self.map.height);
        self.chem.zone_level = self.data.level;
        let theme = TerrainTheme::get(&self.data.theme);
        self.bake_terrain(&theme);
        self.water = Some(WaterLayer::new(&self.map, &theme));
        self.ambience = Ambience::new(self.data.weather.as_deref(), &self.data.theme);
        let lit = self.data.light_if.as_deref().is_some_and(|c| self.state.check(Some(c)));
        self.ambient = lighting::ambient_for(&self.data.theme, self.data.dark && !lit, self.data.ambient.as_deref());
        self.entities.clear();
        self.pending.clear();
        self.dormant.clear();
        self.boss = None;
        self.decoy = None;
        self.companion = None;
        self.state.map_id = id.to_string();
        self.reset_unfinished_arenas();
        if items::TOWNS.iter().any(|t| t.0 == id) {
            self.state.flags.insert(format!("visited:{id}"));
        }
        self.state.flags.insert(format!("seen:{id}"));

        let mut spawn_point = None;
        for entity in self.data.entities.clone() {
            if entity.kind == "spawn" {
                spawn_point = spawn_point.or(Some(tile_feet(entity.x as i32, entity.y as i32)));
            } else if self.should_exist(&entity) {
                if let Some(e) = spawn::create(self, &entity) {
                    self.add_boxed(e);
                }
            } else {
                self.dormant.push(entity);
            }
        }

        let start = position
            .or(spawn_point)
            .unwrap_or(Vec2::new(self.map.pixel_width() as f32 / 2.0, self.map.pixel_height() as f32 / 2.0));
        let mut player = Player::new(start, facing);
        player.added(self);
        self.player = Some(Box::new(player));
        self.refresh_hero();
        self.spawn_companion();
        self.flush_pending();

        if self.auto_save && self.boss.is_none() {
            self.state.set_position(Some(start));
            self.state.facing = facing;
            self.state.save();
            self.saved_flash = 1.2;
        }
        let mut checkpoint = self.state.clone();
        checkpoint.set_position(Some(start));
        checkpoint.facing = facing;
        self.checkpoint = Some(checkpoint);

        audio::play_music(self.data.music.as_deref());
        self.camera.screen_shake = settings::current().screen_shake;
        self.camera.follow(start, self.map.pixel_width(), self.map.pixel_height(), 0.0);
        self.banner_time = if self.data.name.is_empty() { 0.0 } else { 2.2 };
    }

    fn bake_terrain(&mut self, theme: &TerrainTheme) {
        let baked = baker::bake(&self.map, theme, self.data.lava);
        self.terrain.clear();
        for cy in (0..baked.height).step_by(CHUNK as usize) {
            for cx in (0..baked.width).step_by(CHUNK as usize) {
                let (cw, ch) = (CHUNK.min(baked.width - cx), CHUNK.min(baked.height - cy));
                let mut chunk = crate::core::canvas::Canvas::new(cw, ch);
                for y in 0..ch {
                    let row = ((cy + y) * baked.width + cx) as usize;
                    chunk.pixels[(y * cw) as usize..((y + 1) * cw) as usize]
                        .copy_from_slice(&baked.pixels[row..row + cw as usize]);
                }
                self.terrain.push((IRect::new(cx, cy, cw, ch), gfx::from_canvas(chunk)));
            }
        }
    }

    // Leaving a boss arena mid-fight must not leave its gates sealed for the next visit.
    fn reset_unfinished_arenas(&mut self) {
        let arenas = [
            ("boss_arena", "boss_warlock", "warlock_met"),
            ("serpent_arena", "boss_serpent", "serpent_met"),
            ("golem_arena", "boss_golem", "golem_met"),
            ("throne_arena", "boss_sovereign", "sovereign_met"),
            ("warden_arena", "boss_warden", "warden_met"),
            ("wyrm_arena", "boss_wyrm", "wyrm_met"),
        ];
        for (arena, boss, intro) in arenas {
            if !self.state.flags.contains(boss) && self.state.flags.remove(arena) {
                self.state.flags.remove(intro);
            }
        }
    }

    pub fn should_exist(&self, data: &crate::world::map::EntityData) -> bool {
        self.state.check(data.str("if")) && data.str("unless").is_none_or(|u| !self.state.check(Some(u)))
    }

    // Entities whose condition was false at load appear as soon as it becomes true (e.g. a reward chest after a boss).
    fn wake_dormant(&mut self) {
        let mut i = self.dormant.len();
        while i > 0 {
            i -= 1;
            if !self.should_exist(&self.dormant[i]) {
                continue;
            }
            let data = self.dormant[i].clone();
            let Some(entity) = spawn::create(self, &data) else {
                self.dormant.remove(i);
                continue;
            };
            // A solid entity appearing on top of the player would trap them; wait until they step away.
            if entity.base().solid && entity.base().hitbox().intersects(&self.hero.hitbox) {
                continue;
            }
            self.dormant.remove(i);
            self.add_boxed(entity);
            if data.kind == "chest" || data.kind == "npc" {
                self.add(Puff::new(tile_center(data.x as i32, data.y as i32), Rgba::WHITE));
            }
        }
    }

    // ---- Update -------------------------------------------------------------------------------------------------

    pub fn update(&mut self, raw_dt: f32) {
        self.time += raw_dt as f64;
        self.state.play_time += raw_dt as f64;
        self.banner_time = (self.banner_time - raw_dt).max(0.0);
        self.saved_flash = (self.saved_flash - raw_dt).max(0.0);
        self.flash_time = (self.flash_time - raw_dt).max(0.0);
        for t in &mut self.toasts {
            t.1 -= raw_dt;
        }
        self.toasts.retain(|t| t.1 > 0.0);

        self.update_fade(raw_dt);
        self.handle_debug_keys();

        let dead = self.player.as_ref().is_some_and(|p| p.is_dead());
        if !self.input_locked() && !dead && input::pressed(GameAction::Lens) {
            self.swap_era(false);
        }
        if !self.input_locked() && !dead && input::pressed(GameAction::Map) {
            self.requests.push(WorldRequest::Map);
            return;
        }
        if !self.input_locked() && !dead && input::pressed(GameAction::Menu) {
            audio::sfx(Sound::MenuSelect);
            self.requests.push(WorldRequest::Menu);
            return;
        }

        self.wake_dormant();
        if self.hit_stop > 0.0 {
            self.hit_stop -= raw_dt;
            return;
        }
        let mut dt = raw_dt;
        self.frame_delta = raw_dt;
        if self.slow_time > 0.0 {
            self.slow_time -= raw_dt;
            dt *= self.slow_scale;
        }
        self.enemy_slow = (self.enemy_slow - raw_dt).max(0.0);

        self.rank.update(dt);
        self.update_chemistry(dt);
        if let Some(mut player) = self.player.take() {
            player.update(self, dt);
            let interact = player.take_interact();
            let touching = player.take_touch_check();
            self.player = Some(player);
            self.refresh_hero();
            if let Some(id) = interact {
                self.with_entity(id, |e, w| e.interact(w));
            }
            if touching {
                self.touch_overlapping();
            }
        }
        for i in 0..self.entities.len() {
            let Some(mut entity) = self.entities[i].take() else { continue };
            if !entity.base().removed {
                entity.update(self, dt);
            }
            self.entities[i] = Some(entity);
        }
        self.entities.retain(|e| e.as_ref().is_some_and(|e| !e.base().removed));
        self.flush_pending();
        if self.pending_era {
            self.complete_era_swap();
        }

        let mut focus = self.hero.pos - Vec2::new(0.0, 8.0);
        if let Some(boss) = self.boss.and_then(|id| self.get(id)) {
            focus = focus.lerp(boss.base().pos - Vec2::new(0.0, 20.0), 0.35);
        } else if input::mouse_aiming() && !input::scripted() {
            focus += self.hero.aim * 18.0;
        }
        self.camera.follow(focus, self.map.pixel_width(), self.map.pixel_height(), raw_dt);
        self.ambience.update(dt, self.camera.position);
    }

    /// The Memory Lens: step into this place's other era, if it has one and the spot there is open.
    pub fn swap_era(&mut self, from_shard: bool) {
        let Some(pair) = self.data.pair.clone() else {
            if from_shard {
                self.show_message("The shard is dim. This place has no other memory.");
            }
            return;
        };
        if (!from_shard && !self.state.has("memory_lens")) || self.boss.is_some() || self.input_locked() {
            return;
        }
        let Ok(other) = TileMap::new(&MapData::load(&pair)) else { return };
        let hitbox = self.hero.hitbox;
        if other.box_blocked(&hitbox, |x, y| other.blocks_walking(x, y)) && !other.box_blocked(&hitbox, |x, y| !other.in_bounds(x, y)) {
            audio::sfx(Sound::MenuCancel);
            self.toast("Something solid stands here in the other time.");
            return;
        }
        self.pending_era = true;
    }

    fn complete_era_swap(&mut self) {
        self.pending_era = false;
        let Some(pair) = self.data.pair.clone() else { return };
        audio::play(Sound::ChargeLevel, 1.0, 0.5);
        let facing = self.player.as_ref().map(|p| p.facing).unwrap_or(Direction::Down);
        self.load_map(&pair, Some(self.hero.pos), facing);
        self.flash(Rgba::rgb(230, 210, 255), 0.5);
        self.banner_time = 0.0;
        self.toast(if self.data.era.as_deref() == Some("then") { "A hundred years ago..." } else { "...and now." });
    }

    fn touch_overlapping(&mut self) {
        let hitbox = self.hero.hitbox;
        let ids: Vec<EntityId> =
            self.entities().filter(|e| e.base().hitbox().intersects(&hitbox)).map(|e| e.base().id).collect();
        for id in ids {
            self.with_entity(id, |e, w| {
                if !e.base().removed {
                    e.touch(w);
                }
            });
        }
    }

    /// Takes the next queued overlay (message, shop, prompt) if the world is ready to show one.
    pub fn next_overlay(&mut self) -> Option<Box<dyn Scene>> {
        if self.input_locked() {
            return None;
        }
        let make = self.overlays.pop_front()?;
        Some(make(self))
    }

    fn update_fade(&mut self, dt: f32) {
        if self.fading_out {
            self.fade = (self.fade + dt / FADE_SECONDS).min(1.0);
            if self.fade >= 1.0
                && let Some((map, tile, facing)) = self.pending_warp.take()
            {
                self.fading_out = false;
                let target = tile_feet(tile.0, tile.1);
                if map == self.state.map_id {
                    if let Some(p) = self.player.as_mut() {
                        p.base_mut().pos = target;
                        p.facing = facing;
                    }
                    self.refresh_hero();
                } else {
                    self.load_map(&map, Some(target), facing);
                }
            }
        } else {
            self.fade = (self.fade - dt / FADE_SECONDS).max(0.0);
        }
    }

    fn handle_debug_keys(&mut self) {
        if input::key_pressed("F1") {
            self.show_hitboxes = !self.show_hitboxes;
        }
        if input::key_pressed("F2") {
            let need = self.state.exp_to_next() - self.state.exp;
            self.state.add_exp(need);
            audio::sfx(Sound::LevelUp);
        }
        if input::key_pressed("F3") {
            self.state.sp = self.state.max_sp();
        }
        if input::key_pressed("F5") {
            let (pos, facing) = (self.hero.pos, self.hero.facing);
            let id = self.state.map_id.clone();
            self.load_map(&id, Some(pos), facing);
        }
    }

    /// Where an enemy should aim: a decoy if one is up, otherwise the player unless they are hidden.
    pub fn enemy_target(&self, from: Vec2) -> Option<Vec2> {
        if let Some(decoy) = self.decoy.and_then(|id| self.get(id)) {
            return Some(decoy.base().pos);
        }
        if self.hero.dead || self.hero.hidden {
            return None;
        }
        if from.distance(self.hero.pos) > 40.0 && self.steam_hides(self.hero.pos) {
            return None;
        }
        Some(self.hero.pos)
    }

    fn steam_hides(&self, at: Vec2) -> bool {
        self.entities_of::<crate::entities::hazards::SteamCloud>().any(|s| s.base().pos.distance(at) < s.radius() + 6.0)
    }

    /// True while any nearby enemy is hunting the player, which pauses out-of-combat regeneration.
    pub fn in_combat(&self) -> bool {
        self.boss.is_some()
            || self
                .entities_of::<crate::entities::enemy::Enemy>()
                .any(|e| e.alerted() && e.base().pos.distance(self.hero.pos) < 260.0)
    }

    // ---- Movement -----------------------------------------------------------------------------------------------

    /// Moves an entity by up to `delta`, one pixel at a time per axis, and returns how far it went. `avoid_player` is
    /// for NPCs and pushed blocks, which must not walk or slide into the player.
    pub fn move_entity(
        &self,
        base: &mut crate::entities::entity::EntityBase,
        delta: Vec2,
        flying: bool,
        corner_assist: bool,
        avoid_player: bool,
    ) -> Vec2 {
        let start = base.pos;
        self.move_axis(base, delta.x, true, flying, corner_assist && delta.y == 0.0, avoid_player);
        self.move_axis(base, delta.y, false, flying, corner_assist && delta.x == 0.0, avoid_player);
        base.pos - start
    }

    fn move_axis(
        &self,
        base: &mut crate::entities::entity::EntityBase,
        mut amount: f32,
        horizontal: bool,
        flying: bool,
        corner_assist: bool,
        avoid_player: bool,
    ) {
        let axis = |v: f32| if horizontal { Vec2::new(v, 0.0) } else { Vec2::new(0.0, v) };
        let across = |v: f32| if horizontal { Vec2::new(0.0, v) } else { Vec2::new(v, 0.0) };
        while amount.abs() > 0.0001 {
            let step = amount.clamp(-1.0, 1.0);
            let offset = axis(step);
            if !self.blocked(base, &base.hitbox().offset(offset), flying, avoid_player) {
                base.pos += offset;
                amount -= step;
                continue;
            }
            // Slide around corners: if a small sidestep would clear the obstacle, nudge toward it.
            if corner_assist {
                for nudge in 1..=6 {
                    for sign in [-1.0f32, 1.0] {
                        let side = across(nudge as f32 * sign);
                        if !self.blocked(base, &base.hitbox().offset(side + offset), flying, avoid_player)
                            && !self.blocked(base, &base.hitbox().offset(side), flying, avoid_player)
                        {
                            let shift = across(sign * step.abs());
                            if !self.blocked(base, &base.hitbox().offset(shift), flying, avoid_player) {
                                base.pos += shift;
                            }
                            return;
                        }
                    }
                }
            }
            return;
        }
    }

    fn blocked(&self, mover: &crate::entities::entity::EntityBase, b: &Box2, flying: bool, avoid_player: bool) -> bool {
        // Tiles the mover already overlaps never block it, so it can always step out of a gate that closed on it or
        // off the edge of a pit it landed beside.
        let current = mover.hitbox();
        let t = TILE as f32;
        let (cx0, cx1) = ((current.x / t).floor() as i32, ((current.right() - 0.001) / t).floor() as i32);
        let (cy0, cy1) = ((current.y / t).floor() as i32, ((current.bottom() - 0.001) / t).floor() as i32);
        let tile_blocked = self.map.box_blocked(b, |x, y| {
            let blocks = if flying { self.map.blocks_flying(x, y) } else { self.map.blocks_walking(x, y) };
            blocks && !(x >= cx0 && x <= cx1 && y >= cy0 && y <= cy1)
        });
        if tile_blocked {
            return true;
        }
        // Solids already overlapping the mover are ignored so it can always walk out of them.
        for other in self.entities() {
            let ob = other.base();
            if ob.solid && ob.id != mover.id && ob.hitbox().intersects(b) && !ob.hitbox().intersects(&current) {
                return true;
            }
        }
        avoid_player && self.hero.hitbox.intersects(b)
    }

    pub fn find_interactable(&self, probe: &Box2) -> Option<EntityId> {
        self.entities()
            .find(|e| e.can_interact(self) && e.base().hitbox().inflate(1.0).intersects(probe))
            .map(|e| e.base().id)
    }

    // ---- Warps, effects, messages -------------------------------------------------------------------------------

    /// Attempts a warp; blocked warps push the player back and explain why.
    pub fn try_warp(
        &mut self,
        condition: Option<&str>,
        blocked_text: Option<&str>,
        target: &str,
        tile: (i32, i32),
        facing: Direction,
        door: bool,
    ) {
        if self.fading_out || self.pending_warp.is_some() || self.fade > 0.0 {
            return;
        }
        if !self.state.check(condition) {
            if let Some(text) = blocked_text {
                let back = self.hero.facing.to_vec() * 6.0;
                if let Some(p) = self.player.as_mut() {
                    p.base_mut().pos -= back;
                }
                self.refresh_hero();
                self.show_message(text);
            }
            return;
        }
        self.begin_warp(target, tile, facing, door);
    }

    pub fn begin_warp(&mut self, map: &str, tile: (i32, i32), facing: Direction, door: bool) {
        if door {
            audio::sfx(Sound::Door);
        }
        self.pending_warp = Some((map.to_string(), tile, facing));
        self.fading_out = true;
    }

    pub fn flash(&mut self, color: Rgba, seconds: f32) {
        self.flash_color = color;
        self.flash_time = seconds;
        self.flash_length = seconds;
    }

    /// Freezes the action for a moment so hits feel weighty.
    pub fn hit_stop(&mut self, seconds: f32) {
        self.hit_stop = self.hit_stop.max(seconds);
    }

    pub fn slow_motion(&mut self, seconds: f32, scale: f32) {
        self.slow_time = self.slow_time.max(seconds);
        self.slow_scale = scale;
    }

    pub fn slow_enemies(&mut self, seconds: f32) {
        self.enemy_slow = seconds;
        self.flash(Rgba::rgb(220, 180, 255), 0.3);
    }

    /// Replaces the travelling soul with whichever is equipped now.
    pub fn spawn_companion(&mut self) {
        if let Some(id) = self.companion.take() {
            self.with_entity(id, |e, _| e.base_mut().removed = true);
        }
        if let Some(soul) = self.state.soul.as_deref().and_then(souls::get) {
            let id = self.add(crate::entities::companion::Companion::new(soul, self.hero.pos));
            self.companion = Some(id);
        }
    }

    pub fn free_soul(&mut self, soul: &'static SoulDef) {
        if self.state.souls_owned.iter().any(|s| s == soul.id) {
            return;
        }
        let first = self.state.souls_owned.is_empty();
        self.state.give(&format!("soul_{}", soul.id), 1);
        audio::sfx(Sound::LevelUp);
        let mut text = format!(
            "A soul is freed: the {}! It will travel with you.\n{}\n{} ({})",
            soul.name,
            soul.passive,
            soul.active,
            input::glyph(GameAction::Soul)
        );
        if first {
            text.push_str("\nChoose which soul travels with you from the Souls tab of the menu.");
        }
        self.show_message(&text);
        self.spawn_companion();
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.insert(0, (text.into(), 2.6));
    }

    pub fn show_message(&mut self, text: &str) {
        self.show_message_from(text, "", None);
    }

    pub fn show_message_from(&mut self, text: &str, speaker: &str, then: Option<Then>) {
        let (text, speaker) = (text.to_string(), speaker.to_string());
        self.queue(Box::new(move |_| Box::new(DialogueScene::new(vec![text], speaker, then))));
    }

    pub fn queue(&mut self, overlay: Overlay) {
        self.overlays.push_back(overlay);
    }

    pub fn start_dialogue(&mut self, id: &str, speaker: &str) {
        let Some(entries) = data().dialogue.get(id) else {
            self.show_message_from("...", speaker, None);
            return;
        };
        let Some(entry) = entries.iter().find(|e| self.state.check(e.condition.as_deref())) else { return };
        let entry = entry.clone();
        let name = entry.speaker.clone().unwrap_or_else(|| speaker.to_string());
        let lines = entry.lines.clone();
        let display = name.clone();
        let after = move |w: &mut World| {
            w.run_actions(&entry.actions);
            let Some(ask) = entry.ask.clone() else { return };
            let name = name.clone();
            w.queue(Box::new(move |_| {
                let asker = name.clone();
                let answer: Ask = Box::new(move |w, yes| {
                    let lines = if yes { entry.yes_lines.clone() } else { entry.no_lines.clone() };
                    if !lines.is_empty() {
                        let name = name.clone();
                        w.queue(Box::new(move |_| Box::new(DialogueScene::new(lines, name, None))));
                    }
                    let actions = if yes { entry.yes.clone() } else { entry.no.clone() };
                    w.run_actions(&actions);
                });
                Box::new(DialogueScene::ask(ask, asker, answer))
            }));
        };
        if lines.is_empty() {
            after(self);
        } else {
            self.queue(Box::new(move |_| Box::new(DialogueScene::new(lines, display, Some(Box::new(after))))));
        }
    }

    /// Executes dialogue actions like `give:herb`, `flag:met_elder`, `gold:-10` or `shop:village`.
    pub fn run_actions(&mut self, actions: &[String]) {
        for action in actions {
            let (verb, arg) = action.split_once(':').unwrap_or((action.as_str(), ""));
            match verb {
                "give" => {
                    let (id, count) = split_count(arg);
                    self.give_item(id, count);
                }
                "take" => {
                    let (id, count) = split_count(arg);
                    self.state.take(id, count);
                }
                "flag" => {
                    self.state.flags.insert(arg.to_string());
                }
                "unflag" => {
                    self.state.flags.remove(arg);
                }
                "gold" => {
                    self.state.gold = (self.state.gold + arg.trim_start_matches('+').parse::<i32>().unwrap_or(0)).max(0)
                }
                "heal" => {
                    self.state.hp = self.state.max_hp();
                    self.state.sp = self.state.max_sp();
                    if let Some(p) = self.player.as_mut() {
                        p.cure_ailments();
                    }
                    audio::sfx(Sound::Heal);
                }
                "spell" | "art" => {
                    if arts::get(arg).is_some() {
                        self.give_item(arg, 1);
                    }
                }
                "shop" => {
                    let id = arg.to_string();
                    self.queue(Box::new(move |_| Box::new(super::shop::ShopScene::new(&id))));
                }
                "smith" => {
                    let max = arg.parse().unwrap_or(1);
                    self.queue(Box::new(move |_| Box::new(super::smith::SmithScene::new(max))));
                }
                "circuit" => self.queue(Box::new(|_| Box::new(super::circuit::CircuitScene::new()))),
                "pacts" => self.queue(Box::new(|_| Box::new(super::pact::PactScene::new()))),
                "quest" => {
                    if self.state.flags.insert(format!("quest:{arg}"))
                        && let Some(quest) = data().quests.get(arg)
                    {
                        if self.state.tracked_quest.is_none() {
                            self.state.tracked_quest = Some(arg.to_string());
                        }
                        self.toast(format!("New quest: {}", quest.title));
                        audio::play(Sound::ChestOpen, 0.6, 0.0);
                    }
                }
                "say" => self.show_message(arg),
                "save" => self.save_game(),
                "music" => {
                    let track = if arg.is_empty() { self.data.music.clone() } else { Some(arg.to_string()) };
                    audio::play_music(track.as_deref());
                }
                "warp" => {
                    let w: Vec<&str> = arg.split(',').collect();
                    if w.len() >= 3 {
                        let tile = (w[1].parse().unwrap_or(0), w[2].parse().unwrap_or(0));
                        self.begin_warp(w[0], tile, Direction::parse(w.get(3).copied()), true);
                    }
                }
                "ending" => {
                    self.state.flags.insert("game_clear".into());
                    audio::play_music(Some("Menu-AdventuresAwait"));
                    self.requests.push(WorldRequest::Ending);
                }
                _ => {}
            }
        }
    }

    pub fn give_item(&mut self, id: &str, count: i32) {
        let got = self.state.give(id, count);
        audio::sfx(Sound::Pickup);
        let name = items::name_of(id);
        let mut text = if count > 1 { format!("Obtained {count} x {name}!") } else { format!("Obtained the {name}!") };
        if let Some(item) = items::get(id) {
            if matches!(item.kind, ItemKind::Consumable | ItemKind::Material) {
                self.toast(if got < count {
                    format!("{name} x{got} (pouch full)")
                } else {
                    format!("{name} x{count}")
                });
                return;
            }
            text.push('\n');
            text.push_str(item.description);
        } else if let Some(sword) = items::sword(id) {
            text.push_str(&format!(
                "\nYou can now fight in {} mode. Switch modes with {} / {}, the mouse wheel, or the number keys.",
                sword.element.name(),
                input::glyph(GameAction::PrevMode),
                input::glyph(GameAction::NextMode)
            ));
        } else if let Some(art) = arts::get(id) {
            text = format!(
                "Learned the Combat Art {}!\n{} (Hold {} and {} in {} mode.)",
                art.name,
                art.description,
                input::glyph(GameAction::Art),
                art.slot.key(),
                art.element.label()
            );
        }
        self.show_message(&text);
    }

    pub fn heal_player(&mut self, amount: i32) {
        self.state.hp = (self.state.hp + amount).min(self.state.max_hp());
    }

    /// Adds SP, scaled by accessories, the Circuit and Neutral mode.
    pub fn add_sp(&mut self, amount: f32) {
        let mods = self.state.mods().sp_gain;
        let neutral = if self.state.mode == Element::None { 1.2 } else { 1.0 };
        self.state.sp = (self.state.sp + amount * self.state.acc().sp_gain * mods * neutral).min(self.state.max_sp());
    }

    pub fn reward_kill(&mut self, kill: KillInfo) {
        let weight = if kill.boss {
            5.0
        } else if kill.def.guard > 0.0 {
            1.5
        } else {
            1.0
        };
        self.rank.kill(weight);
        let mut multiplier = self.rank.multiplier();
        *self.state.kills.entry(kill.def.id.clone()).or_default() += 1;
        multiplier *= pacts::reward_multiplier(&self.state);
        let exp = (combat::enemy_exp(kill.level)
            * kill.def.exp
            * combat::exp_gap_factor(kill.level, self.state.level)
            * self.state.acc().exp
            * multiplier)
            .round() as i32;
        let levels = self.state.add_exp(exp);
        if exp > 0 {
            self.add(FloatingText::new(kill.pos - Vec2::new(0.0, 26.0), format!("{exp} EXP"), palette::EXPERIENCE));
        }
        let life_on_kill = self.state.acc().life_on_kill;
        if life_on_kill > 0.0 && self.state.hp < self.state.max_hp() {
            let amount = ((self.state.max_hp() as f32 * life_on_kill) as i32).max(1);
            self.heal_player(amount);
        }
        self.add_sp(12.0);
        if levels > 0 {
            audio::sfx(Sound::LevelUp);
            self.add(FloatingText::new(self.hero.pos - Vec2::new(0.0, 34.0), "LEVEL UP!", palette::HIGHLIGHT));
            Particle::burst(self, self.hero.pos - Vec2::new(0.0, 8.0), palette::HIGHLIGHT, 24, 70.0, 0.8, true);
            self.toast(format!("Reached level {}!", self.state.level));
        }
        let gold =
            (2.0 + 1.6 * kill.level as f32) * kill.def.gold * self.state.acc().gold * (0.8 + self.rng.next_f32() * 0.4);
        if gold >= 1.0 {
            self.add(Pickup::gold(kill.pos, gold.round() as i32));
        }
        for drop in &kill.def.drops {
            if self.rng.next_f32() < drop.chance * multiplier {
                let offset = Vec2::new(self.rng.range(-6, 7) as f32, 0.0);
                self.add(Pickup::item(kill.pos + offset, &drop.item));
            }
        }
        if kill.blighted && self.rng.next_f32() < 0.04 * multiplier {
            self.add(Pickup::item(kill.pos + Vec2::new(-4.0, 0.0), "blight_core"));
        }
        if self.rng.next_f32() < 0.1 && !kill.boss {
            let amount = (self.state.max_hp() / 10).max(4);
            self.add(Pickup::heart(kill.pos + Vec2::new(4.0, 0.0), amount));
        }
    }

    pub fn on_player_defeated(&mut self) {
        if self.game_over {
            return;
        }
        self.game_over = true;
        audio::play_music(None);
        let state = self.checkpoint.clone().unwrap_or_else(|| self.state.clone());
        self.requests.push(WorldRequest::GameOver(Box::new(state)));
    }

    pub fn on_boss_defeated(&mut self, boss_flag: &str) {
        let heat = pacts::heat(&self.state);
        if heat > 0 {
            let prefix = format!("heat:{boss_flag}=");
            let best = self.state.flags.iter().find(|f| f.starts_with(&prefix)).cloned();
            let beaten = best.as_ref().is_none_or(|b| b[prefix.len()..].parse::<i32>().unwrap_or(0) < heat);
            if beaten {
                if let Some(b) = best {
                    self.state.flags.remove(&b);
                }
                self.state.flags.insert(format!("{prefix}{heat}"));
                self.toast(format!("New best Heat: {heat}"));
            }
        }
        audio::play_music(self.data.music.as_deref());
        self.camera.shake(0.4, 2.0);
    }

    pub fn save_game(&mut self) {
        self.state.set_position(Some(self.hero.pos));
        self.state.facing = self.hero.facing;
        self.state.save();
    }

    // ---- Drawing ------------------------------------------------------------------------------------------------

    pub fn draw(&self, batch: &mut Batch) {
        let view = self.camera.view();
        let offset = self.camera.offset();
        batch.begin(offset);
        if let Some(water) = &self.water {
            water.draw(batch, view, self.time);
        }
        for &(area, tex) in &self.terrain {
            if area.intersects(&view) {
                batch.draw(tex, Vec2::new(area.x as f32, area.y as f32), Rgba::WHITE);
            }
        }
        self.draw_chemistry(batch, view);

        let visible = view.inflate(96, 96);
        let mut drawables: Vec<&dyn Entity> = self.entities().collect();
        if let Some(p) = self.player.as_deref() {
            drawables.push(p);
        }
        drawables.retain(|e| visible.contains_point(e.base().pos.x as i32, e.base().pos.y as i32));
        for layer in 0..=2 {
            let mut group: Vec<&dyn Entity> = drawables.iter().copied().filter(|e| e.layer() == layer).collect();
            if layer == 1 {
                group.sort_by(|a, b| a.sort_y().total_cmp(&b.sort_y()));
            }
            for e in group {
                e.draw(self, batch);
            }
        }
        if let Some(p) = self.player.as_deref()
            && !p.is_dead()
            && !self.input_locked()
        {
            crate::ui::hud::draw_aim(batch, self, p);
        }
        if self.show_hitboxes {
            for e in self.entities() {
                batch.draw_rect(e.base().hitbox().to_irect(), Rgba::YELLOW.mul(0.7));
                if e.faction().is_some() {
                    batch.draw_rect(e.hurtbox().to_irect(), Rgba::RED.mul(0.7));
                }
            }
        }
        batch.begin_screen();
        self.ambience.draw_lit(batch);
        self.publish_lights(offset);
        batch.mark_light();
        self.ambience.draw_emissive(batch);
        if self.banner_time > 0.0 && self.boss.is_none() {
            let alpha = (self.banner_time / 0.5).min(1.0) * ((2.2 - self.banner_time) / 0.3).min(1.0);
            let width = font::measure(&self.data.name) + 20;
            let rect = IRect::new((SCREEN_WIDTH - width) / 2, 30, width, 17);
            batch.fill(rect, palette::WINDOW_FILL.mul(0.85 * alpha));
            batch.fill(IRect::new(rect.x, rect.bottom() - 1, rect.w, 1), palette::HIGHLIGHT.mul(alpha));
            font::draw_centered(
                batch,
                &self.data.name,
                Vec2::new(SCREEN_WIDTH as f32 / 2.0, 34.0),
                palette::TEXT.mul(alpha),
                Some(palette::TEXT_SHADOW.mul(alpha)),
            );
        }
        crate::ui::hud::draw(batch, self);
        if self.saved_flash > 0.0 {
            let pos = Vec2::new((SCREEN_WIDTH - 34) as f32, (SCREEN_HEIGHT - 14) as f32);
            font::draw(
                batch,
                "Saved",
                pos,
                palette::TEXT_DIM.mul((self.saved_flash * 2.0).min(1.0)),
                Some(palette::TEXT_SHADOW),
            );
        }
        if self.flash_time > 0.0 {
            batch.fill(
                IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT),
                self.flash_color.mul(self.flash_time / self.flash_length * 0.6),
            );
        }
        if self.fade > 0.0 {
            batch.fill(IRect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT), Rgba::BLACK.mul(self.fade));
        }
    }

    fn publish_lights(&self, offset: Vec2) {
        if self.ambient == Rgba::WHITE {
            lighting::publish(LightFrame::default());
            return;
        }
        let mut lights = Vec::new();
        let mut sources: Vec<&dyn Entity> = self.entities().collect();
        if let Some(p) = self.player.as_deref() {
            sources.push(p);
        }
        for e in sources {
            if let Some((radius, color)) = e.light(self) {
                let flicker = 1.0 + (self.time as f32 * 9.0 + e.base().pos.x).sin() * 0.04;
                lights.push((e.base().pos - Vec2::new(0.0, 8.0) + offset, radius * flicker, color, 0.9));
            }
        }
        for (at, radius, color) in self.chem.lights(&self.map) {
            lights.push((at + offset, radius, color, 0.8));
        }
        lighting::publish(LightFrame { ambient: Some(self.ambient), lights });
    }
}

fn split_count(arg: &str) -> (&str, i32) {
    match arg.split_once('*') {
        Some((id, n)) => (id, n.parse().unwrap_or(1)),
        None => (arg, 1),
    }
}
