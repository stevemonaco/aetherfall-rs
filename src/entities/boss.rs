//! Shared boss plumbing: registers the health bar, ignores knockback, plays a drawn-out death, then sets the boss's flag
//! and runs its `defeat` actions from the map file. Also the breakable boss parts (a staff gem, a fin, an arm).

use std::sync::atomic::{AtomicU32, Ordering};

use bevy::math::Vec2;

use super::effects::{DamageNumber, FloatingText, Particle};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use super::pickup::Pickup;
use super::warlock::WarlockState;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, snap};
use crate::core::gfx::{self, Batch, TexId};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{self, Faction, Hit};
use crate::gameplay::element::Element;
use crate::gameplay::game_data::EnemyDef;
use crate::scenes::world::World;

/// Test-only multiplier on boss life (see `--boss-hp`), stored as f32 bits.
pub static DEBUG_HP_SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000);

pub fn debug_hp_scale() -> f32 {
    f32::from_bits(DEBUG_HP_SCALE.load(Ordering::Relaxed))
}

/// Per-boss behaviour.
pub enum BossKind {
    None,
    Warlock(Box<WarlockState>),
}

impl BossKind {
    pub fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32) {
        match self {
            BossKind::Warlock(state) => state.think(enemy, world, dt),
            BossKind::None => {}
        }
    }

    pub fn draw(&self, enemy: &Enemy, boss: &BossCore, world: &World, batch: &mut Batch) {
        if let BossKind::Warlock(state) = self {
            state.draw(enemy, boss, world, batch);
        }
    }
}

pub struct BossCore {
    pub flag: String,
    pub defeat_actions: Vec<String>,
    /// A Blighted remix of the boss: tougher, faster, and tinted by the blight.
    pub echo: bool,
    pub home: Vec2,
    pub time: f32,
    pub dying_time: f32,
    pub kind: BossKind,
}

impl BossCore {
    pub fn new(flag: String, defeat_actions: Vec<String>, home: Vec2, kind: BossKind) -> BossCore {
        BossCore { flag, defeat_actions, echo: false, home, time: 0.0, dying_time: -1.0, kind }
    }

    pub fn dying(&self) -> bool {
        self.dying_time >= 0.0
    }

    pub fn death_duration(&self) -> f32 {
        2.2
    }

    pub fn death_color(&self) -> Rgba {
        Rgba::rgb(255, 200, 140)
    }

    pub fn death_fade(&self) -> f32 {
        if self.dying() { (1.0 - self.dying_time / self.death_duration()).max(0.0) } else { 1.0 }
    }

    /// False while the boss is submerged, teleporting, or otherwise not physically present.
    pub fn tangible(&self) -> bool {
        match &self.kind {
            BossKind::Warlock(state) => state.tangible(),
            BossKind::None => true,
        }
    }

    /// How visible the boss is right now (fading while teleporting or dying); parts follow it.
    pub fn fade(&self) -> f32 {
        match &self.kind {
            BossKind::Warlock(state) => state.fade() * self.death_fade(),
            BossKind::None => self.death_fade(),
        }
    }

    pub fn tint(&self) -> Rgba {
        if self.echo { Rgba::rgb(205, 165, 255) } else { Rgba::WHITE }
    }

    pub fn hurtbox(&self, enemy: &Enemy) -> Option<Box2> {
        match &self.kind {
            BossKind::Warlock(_) => Some(Box2::new(enemy.base.pos.x - 10.0, enemy.base.pos.y - 34.0 - enemy.z, 20.0, 32.0)),
            BossKind::None => None,
        }
    }

    pub fn accepts_hit(&self, _hit: &Hit) -> bool {
        true
    }

    pub fn damage_scale(&self, _element: Element) -> f32 {
        1.0
    }

    pub fn display_name(&self, def: &EnemyDef) -> String {
        if self.echo { format!("Echo of {}", def.name) } else { def.name.clone() }
    }
}

/// Builds a boss of the given kind from its map entry.
pub fn create(def: &'static EnemyDef, pos: Vec2, flag: String, defeat: Vec<String>, echo: bool) -> Enemy {
    let mut enemy = Enemy::new(def, pos, None);
    let kind = match def.id.as_str() {
        _ => BossKind::Warlock(Box::new(WarlockState::new(pos))),
    };
    let mut core = BossCore::new(flag, defeat, pos, kind);
    core.echo = echo;
    enemy.boss = Some(Box::new(core));
    enemy
}

/// Runs when a boss joins the world.
pub fn added(enemy: &mut Enemy, world: &mut World) {
    let Some(boss) = enemy.boss.as_ref() else { return };
    if world.state.flags.contains(&boss.flag) {
        enemy.base.removed = true;
        return;
    }
    world.boss = Some(enemy.base.id);
    audio::sfx(Sound::BossRoar);
    let echo = if boss.echo { 1.6 } else { 1.0 };
    enemy.max_hp = ((enemy.max_hp as f32 * echo * debug_hp_scale()) as i32).max(1);
    enemy.hp = enemy.max_hp;
    if matches!(boss.kind, BossKind::Warlock(_)) {
        let hp = ((enemy.max_hp as f32 * 0.14) as i32).max(20);
        let part = BossPart::new(enemy.base.id, "Staff gem", hp, PartAnchor::WarlockGem, crate::art::combat::boss_gem(Rgba::rgb(255, 140, 60)), Vec2::new(10.0, 10.0), Some("ember_shard"));
        world.add(part);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartAnchor {
    WarlockGem,
}

impl PartAnchor {
    fn at(self, owner: &Enemy) -> Vec2 {
        match self {
            PartAnchor::WarlockGem => owner.base.pos - Vec2::new(-11.0, 33.0 + owner.z),
        }
    }
}

/// A breakable piece of a boss. It has its own life; damage to it also bleeds into the boss. Breaking it calls back
/// into the boss, which loses whatever the part powered.
pub struct BossPart {
    base: EntityBase,
    owner: EntityId,
    name: String,
    max_hp: i32,
    hp: i32,
    anchor: PartAnchor,
    art: TexId,
    drop: Option<String>,
    flash: f32,
    center: Vec2,
    fade: f32,
    owner_y: f32,
    pub shielded: bool,
    pub hidden: bool,
}

impl BossPart {
    pub fn new(owner: EntityId, name: &str, hp: i32, anchor: PartAnchor, art: TexId, size: Vec2, drop: Option<&str>) -> BossPart {
        BossPart {
            base: EntityBase { width: size.x, height: size.y, ..EntityBase::default() },
            owner,
            name: name.to_string(),
            max_hp: hp,
            hp,
            anchor,
            art,
            drop: drop.map(str::to_string),
            flash: 0.0,
            center: Vec2::new(-999.0, -999.0),
            fade: 1.0,
            owner_y: 0.0,
            shielded: false,
            hidden: false,
        }
    }

    pub fn broken(&self) -> bool {
        self.hp <= 0
    }

    fn refresh(&mut self, world: &World) -> bool {
        let Some(owner) = world.get_as::<Enemy>(self.owner) else { return false };
        self.center = self.anchor.at(owner);
        self.fade = owner.boss.as_ref().map(|b| b.fade()).unwrap_or(1.0);
        self.owner_y = owner.base.pos.y;
        self.base.pos = self.center + Vec2::new(0.0, self.base.height / 2.0);
        true
    }

    fn break_part(&mut self, world: &mut World) {
        audio::play(Sound::Break, 1.0, 0.2);
        world.hit_stop(0.12);
        world.camera.shake(0.4, 2.0);
        let center = self.hurtbox().center();
        Particle::burst(world, center, Rgba::WHITE, 30, 110.0, 0.7, true);
        world.add(FloatingText::new(center - Vec2::new(0.0, 16.0), format!("{} BROKEN", self.name.to_uppercase()), palette::HIGHLIGHT));
        world.rank.style(3.0);
        if let Some(drop) = &self.drop {
            world.add(Pickup::item(center + Vec2::new(0.0, 10.0), drop));
        }
        let anchor = self.anchor;
        world.with_as::<Enemy, _>(self.owner, |e, w| {
            if let Some(boss) = e.boss.as_mut() {
                let mut kind = std::mem::replace(&mut boss.kind, BossKind::None);
                if let (BossKind::Warlock(state), PartAnchor::WarlockGem) = (&mut kind, anchor) {
                    state.gem_broken(e, w);
                }
                if let Some(b) = e.boss.as_mut() {
                    b.kind = kind;
                }
            }
        });
    }
}

impl Entity for BossPart {
    entity_base!();

    fn layer(&self) -> i32 {
        2
    }

    fn sort_y(&self) -> f32 {
        self.owner_y + 1.0
    }

    fn faction(&self) -> Option<Faction> {
        Some(if self.hidden || self.broken() { Faction::Neutral } else { Faction::Enemy })
    }

    fn hurtbox(&self) -> Box2 {
        if self.hidden {
            return Box2::new(-999.0, -999.0, 0.0, 0.0);
        }
        Box2::from_center(self.center, self.base.width, self.base.height)
    }

    fn added(&mut self, world: &mut World) {
        self.refresh(world);
    }

    fn update(&mut self, world: &mut World, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
        if !self.refresh(world) {
            self.base.removed = true;
        }
    }

    fn take_hit(&mut self, world: &mut World, hit: &mut Hit) -> bool {
        let Some(owner) = world.get_as::<Enemy>(self.owner) else { return false };
        let dying = owner.boss.as_ref().is_some_and(|b| b.dying());
        if self.broken() || self.hidden || hit.source != Faction::Player || dying {
            return false;
        }
        if self.shielded {
            audio::play(Sound::Clink, 0.7, 0.3);
            return true;
        }
        let element = if hit.element == Element::Aether { Element::Water } else { hit.element };
        let resist = owner.def.resist_to(element);
        let mut stats = owner.stats();
        stats.def *= 0.7;
        let mut probe = hit.clone();
        probe.element = element;
        let result = combat::resolve(&probe, stats, resist.max(0.25), 1.0, &world.rng);
        hit.result = result;
        self.hp -= result.amount;
        self.flash = 0.12;
        audio::play(Sound::Hit, 0.9, 0.2);
        world.add(DamageNumber::new(self.hurtbox().center() - Vec2::new(0.0, 8.0), result.amount, element, result.crit));
        let bleed = result.amount / 2;
        world.with_as::<Enemy, _>(self.owner, |e, w| e.bleed_from_part(w, bleed));
        if self.hp <= 0 {
            self.break_part(world);
        }
        true
    }

    fn draw(&self, _world: &World, batch: &mut Batch) {
        if self.broken() || self.hidden {
            return;
        }
        let at = snap(self.center);
        let (w, h) = gfx::size(self.art);
        let flashing = self.flash > 0.0;
        let tint = if flashing { Rgba::WHITE } else { Rgba::WHITE.mul(self.fade) };
        let tex = if flashing { gfx::silhouette(self.art) } else { self.art };
        batch.draw_ex(tex, at, None, tint, 0.0, Vec2::new(w as f32 / 2.0, h as f32 / 2.0), Vec2::ONE, false, false);
        if self.hp < self.max_hp {
            let bar = Vec2::new(at.x - 8.0, at.y - h as f32 / 2.0 - 4.0);
            batch.fill_rect(bar.x - 1.0, bar.y - 1.0, 18.0, 3.0, palette::OUTLINE);
            batch.fill_rect(bar.x, bar.y, 16.0 * self.hp as f32 / self.max_hp as f32, 1.0, palette::HIGHLIGHT);
        }
    }
}
