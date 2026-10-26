//! Shared boss plumbing: registers the health bar, ignores knockback, plays a drawn-out death, then sets the boss's flag
//! and runs its `defeat` actions from the map file. Also the breakable boss parts (a staff gem, a fin, an arm).

use std::sync::atomic::{AtomicU32, Ordering};

use bevy::math::Vec2;

use super::effects::{DamageNumber, FloatingText, Particle};
use super::enemy::Enemy;
use super::entity::{Entity, EntityBase, EntityId};
use super::pickup::Pickup;
use super::projectile::Projectile;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::geom::{Box2, safe_normalize, snap};
use crate::core::gfx::{self, Batch, TexId};
use crate::core::palette;
use crate::entity_base;
use crate::gameplay::combat::{self, Faction, Hit, HitKind};
use crate::gameplay::element::Element;
use crate::gameplay::game_data::EnemyDef;
use crate::scenes::world::World;

/// Test-only multiplier on boss life (see `--boss-hp`), stored as f32 bits.
pub static DEBUG_HP_SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000);

pub fn debug_hp_scale() -> f32 {
    f32::from_bits(DEBUG_HP_SCALE.load(Ordering::Relaxed))
}

/// One boss's behaviour. The boss itself is an [`Enemy`] carrying a [`BossCore`]; the brain is taken out of the core
/// while it runs, so `enemy.boss` is still readable (time, echo, dying) from inside these calls.
pub trait BossBrain {
    /// Called once the boss joins the world (after the shared setup); add parts here.
    fn added(&mut self, _enemy: &mut Enemy, _world: &mut World) {}

    fn think(&mut self, enemy: &mut Enemy, world: &mut World, dt: f32);

    fn draw(&self, enemy: &Enemy, boss: &BossCore, world: &World, batch: &mut Batch);

    fn hurtbox(&self, enemy: &Enemy) -> Box2;

    /// False while the boss is submerged, teleporting, or otherwise not physically present.
    fn tangible(&self) -> bool {
        true
    }

    /// How visible the boss is apart from dying (parts follow it).
    fn fade(&self) -> f32 {
        1.0
    }

    fn death_color(&self) -> Rgba {
        Rgba::rgb(255, 200, 140)
    }

    fn death_duration(&self) -> f32 {
        2.2
    }

    /// Whether ground hazards reach the boss even though it is a flier (bosses that rise out of water or lava).
    fn touches_ground(&self) -> bool {
        false
    }

    fn light(&self, _enemy: &Enemy) -> Option<(f32, Rgba)> {
        None
    }

    fn sort_y(&self, enemy: &Enemy) -> Option<f32> {
        let _ = enemy;
        None
    }

    /// Overrides the boss bar's fill (a boss whose first phase is measured some other way).
    fn health_fraction(&self, _enemy: &Enemy) -> Option<f32> {
        None
    }

    /// Runs before the shared hit handling. Adjust `hit` and return `None` to continue, or `Some(result)` to finish
    /// the hit here.
    fn before_hit(&mut self, _enemy: &mut Enemy, _world: &mut World, _hit: &mut Hit) -> Option<bool> {
        None
    }

    /// Runs when the boss's life runs out; return true to stay alive (a phase change instead of death).
    fn on_die(&mut self, _enemy: &mut Enemy, _world: &mut World) -> bool {
        false
    }

    /// Where part `index` sits right now.
    fn part_anchor(&self, _index: usize, enemy: &Enemy) -> Vec2 {
        enemy.base.pos
    }

    fn part_broken(&mut self, _index: usize, _enemy: &mut Enemy, _world: &mut World) {}
}

pub struct BossCore {
    pub flag: String,
    pub defeat_actions: Vec<String>,
    /// A Blighted remix of the boss: tougher, faster, and tinted by the blight.
    pub echo: bool,
    pub home: Vec2,
    pub time: f32,
    pub dying_time: f32,
    brain: Option<Box<dyn BossBrain>>,
    death_duration: f32,
    death_color: Rgba,
    tangible: bool,
    fade: f32,
}

impl BossCore {
    pub fn new(flag: String, defeat_actions: Vec<String>, home: Vec2, brain: Box<dyn BossBrain>) -> BossCore {
        let mut core = BossCore {
            flag,
            defeat_actions,
            echo: false,
            home,
            time: 0.0,
            dying_time: -1.0,
            death_duration: 2.2,
            death_color: Rgba::WHITE,
            tangible: true,
            fade: 1.0,
            brain: None,
        };
        core.put_brain(brain);
        core
    }

    /// Takes the brain out so it can borrow the enemy mutably; always pair with [`BossCore::put_brain`].
    pub fn take_brain(&mut self) -> Option<Box<dyn BossBrain>> {
        self.brain.take()
    }

    pub fn put_brain(&mut self, brain: Box<dyn BossBrain>) {
        self.death_duration = brain.death_duration();
        self.death_color = brain.death_color();
        self.tangible = brain.tangible();
        self.fade = brain.fade();
        self.brain = Some(brain);
    }

    pub fn brain(&self) -> Option<&dyn BossBrain> {
        self.brain.as_deref()
    }

    pub fn dying(&self) -> bool {
        self.dying_time >= 0.0
    }

    pub fn death_duration(&self) -> f32 {
        self.death_duration
    }

    pub fn death_color(&self) -> Rgba {
        self.death_color
    }

    pub fn death_fade(&self) -> f32 {
        if self.dying() { (1.0 - self.dying_time / self.death_duration).max(0.0) } else { 1.0 }
    }

    pub fn tangible(&self) -> bool {
        self.tangible
    }

    /// How visible the boss is right now (fading while teleporting or dying); parts follow it.
    pub fn fade(&self) -> f32 {
        self.fade * self.death_fade()
    }

    pub fn tint(&self) -> Rgba {
        if self.echo { Rgba::rgb(205, 165, 255) } else { Rgba::WHITE }
    }

    pub fn display_name(&self, def: &EnemyDef) -> String {
        if self.echo { format!("Echo of {}", def.name) } else { def.name.clone() }
    }
}

/// Runs `f` with the boss's brain taken out of the enemy, then puts it back.
pub fn with_brain<R>(enemy: &mut Enemy, f: impl FnOnce(&mut dyn BossBrain, &mut Enemy) -> R) -> Option<R> {
    let mut brain = enemy.boss.as_mut()?.take_brain()?;
    let result = f(brain.as_mut(), enemy);
    if let Some(core) = enemy.boss.as_mut() {
        core.put_brain(brain);
    }
    Some(result)
}

/// Builds a boss of the given kind from its map entry.
pub fn create(def: &'static EnemyDef, pos: Vec2, flag: String, defeat: Vec<String>, echo: bool) -> Enemy {
    let mut enemy = Enemy::new(def, pos, None);
    let brain: Box<dyn BossBrain> = match def.id.as_str() {
        "golem" => Box::new(super::golem::GolemBrain::new(&mut enemy)),
        "serpent" => Box::new(super::serpent::SerpentBrain::new(&mut enemy)),
        "warden" => Box::new(super::warden::WardenBrain::new(&mut enemy)),
        "wyrm" => Box::new(super::wyrm::WyrmBrain::new(&mut enemy)),
        "sovereign" => Box::new(super::sovereign::SovereignBrain::new(&mut enemy)),
        "concord" => Box::new(super::concord::ConcordBrain::new(&mut enemy)),
        _ => Box::new(super::warlock::WarlockState::new(pos)),
    };
    let mut core = BossCore::new(flag, defeat, pos, brain);
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
    world.telemetry.start(&format!("boss:{}", boss.flag));
    audio::sfx(Sound::BossRoar);
    let echo = if boss.echo { 1.6 } else { 1.0 };
    enemy.max_hp = ((enemy.max_hp as f32 * echo * debug_hp_scale()) as i32).max(1);
    enemy.hp = enemy.max_hp;
    with_brain(enemy, |brain, enemy| brain.added(enemy, world));
}

/// Adds breakable part `index` to the boss, with a share of the boss's life (at least 20).
#[allow(clippy::too_many_arguments)]
pub fn add_part(
    enemy: &Enemy,
    world: &mut World,
    index: usize,
    name: &str,
    hp_share: f32,
    art: TexId,
    size: Vec2,
    drop: Option<&str>,
) -> EntityId {
    let hp = ((enemy.max_hp as f32 * hp_share) as i32).max(20);
    world.add(BossPart::new(enemy.base.id, name, hp, index, art, size, drop))
}

/// Sets a part's armour (hits clink off) or hides it entirely.
pub fn set_part(world: &mut World, part: EntityId, shielded: bool, hidden: bool) {
    world.with_as::<BossPart, _>(part, |p, _| {
        p.shielded = shielded;
        p.hidden = hidden;
    });
}

pub fn part_broken(world: &World, part: EntityId) -> bool {
    world.get_as::<BossPart>(part).is_none_or(|p| p.broken())
}

/// An enemy projectile from the boss, aimed along `direction`.
pub fn shoot(enemy: &Enemy, world: &mut World, kind: &str, from: Vec2, direction: Vec2, speed_scale: f32, power: f32) {
    world.add(Projectile::enemy_shot(kind, from, safe_normalize(direction), enemy.stats(), power, speed_scale));
}

/// `count` shots spread evenly around a circle, starting at angle `offset`.
#[allow(clippy::too_many_arguments)]
pub fn ring(
    enemy: &Enemy,
    world: &mut World,
    kind: &str,
    from: Vec2,
    count: i32,
    offset: f32,
    speed_scale: f32,
    power: f32,
) {
    for i in 0..count {
        let a = offset + i as f32 * std::f32::consts::TAU / count as f32;
        shoot(enemy, world, kind, from, Vec2::new(a.cos(), a.sin()), speed_scale, power);
    }
}

/// A direct hit from the boss (slams, charges, beams).
pub fn boss_hit(enemy: &Enemy, power: f32, element: Element, from: Vec2, unblockable: bool) -> Hit {
    Hit {
        power,
        element,
        from,
        owner: Some(enemy.base.id),
        kind: HitKind::Attack,
        unblockable,
        knockback: 180.0,
        ..Hit::new(Faction::Enemy, enemy.stats())
    }
}

/// Hits the player with a boss attack, letting a parry stagger the boss.
pub fn hit_player(enemy: &mut Enemy, world: &mut World, mut hit: Hit) {
    world.hit_player(&mut hit);
    if hit.parried {
        enemy.parried(world);
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
    index: usize,
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
    pub fn new(
        owner: EntityId,
        name: &str,
        hp: i32,
        index: usize,
        art: TexId,
        size: Vec2,
        drop: Option<&str>,
    ) -> BossPart {
        BossPart {
            base: EntityBase { width: size.x, height: size.y, ..EntityBase::default() },
            owner,
            name: name.to_string(),
            max_hp: hp,
            hp,
            index,
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
        let Some(core) = owner.boss.as_ref() else { return false };
        if let Some(brain) = core.brain() {
            self.center = brain.part_anchor(self.index, owner);
        }
        self.fade = core.fade();
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
        world.add(FloatingText::new(
            center - Vec2::new(0.0, 16.0),
            format!("{} BROKEN", self.name.to_uppercase()),
            palette::HIGHLIGHT,
        ));
        world.rank.style(3.0);
        if let Some(drop) = &self.drop {
            world.add(Pickup::item(center + Vec2::new(0.0, 10.0), drop));
        }
        let index = self.index;
        world.with_as::<Enemy, _>(self.owner, |e, w| {
            with_brain(e, |brain, e| brain.part_broken(index, e, w));
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
        world.add(DamageNumber::new(
            self.hurtbox().center() - Vec2::new(0.0, 8.0),
            result.amount,
            element,
            result.crit,
        ));
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
