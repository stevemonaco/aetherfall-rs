//! The Weave: a lattice of passives on a square grid, grown outward from a central knot. Threads run along every even
//! row and column, so the grid has holes and more than one way around them. Every node costs one point and can be
//! taken once an orthogonal neighbour is; keystones also need a Weave Knot. Regions are themed by playstyle rather than
//! element. A node's effect is a short comma-separated list such as `atk:0.02`, `pierce:0.15` or `on:kill:spirit_tap`.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use super::element::{BASIC, Element};
use crate::core::color::Rgba;

pub const HUB: &str = "hub";
/// The grid reaches this far from the hub along each axis.
pub const RADIUS: i32 = 8;
/// Nodes lie within this many steps of the hub, which rounds the grid's corners into a diamond.
const REACH: i32 = 11;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Hub,
    Minor,
    Notable,
    Keystone,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Region {
    Hub,
    Might,
    Ward,
    Swiftness,
    Spirit,
    Aether,
}

impl Region {
    pub fn label(self) -> &'static str {
        match self {
            Region::Hub => "The Weave",
            Region::Might => "Might",
            Region::Ward => "Ward",
            Region::Swiftness => "Swiftness",
            Region::Spirit => "Spirit",
            Region::Aether => "Aether",
        }
    }

    pub fn color(self) -> Rgba {
        match self {
            Region::Hub => Rgba::rgb(230, 230, 245),
            Region::Might => Rgba::rgb(240, 110, 90),
            Region::Ward => Rgba::rgb(110, 160, 240),
            Region::Swiftness => Rgba::rgb(110, 230, 170),
            Region::Spirit => Rgba::rgb(180, 140, 250),
            Region::Aether => Element::Aether.glow(),
        }
    }

    fn of(x: i32, y: i32) -> Region {
        if x == 0 && y == 0 {
            Region::Hub
        } else if x.abs().min(y.abs()) >= 1 && (x.abs() - y.abs()).abs() <= 1 {
            Region::Aether
        } else if y.abs() > x.abs() {
            if y < 0 { Region::Might } else { Region::Ward }
        } else if x > 0 {
            Region::Swiftness
        } else {
            Region::Spirit
        }
    }
}

#[derive(Clone, Debug)]
pub struct WeaveNode {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub region: Region,
    pub kind: NodeKind,
    pub name: String,
    pub description: String,
    pub effect: String,
}

impl WeaveNode {
    pub fn needs_knot(&self) -> bool {
        self.kind == NodeKind::Keystone
    }
}

/// Events a trigger node can react to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Trigger {
    Kill,
    Hurt,
    Dodge,
    CloseCall,
    PerfectGuard,
    /// A skill performed at level 3.
    GreatSkill,
}

impl Trigger {
    fn parse(s: &str) -> Option<Trigger> {
        Some(match s {
            "kill" => Trigger::Kill,
            "hurt" => Trigger::Hurt,
            "dodge" => Trigger::Dodge,
            "close_call" => Trigger::CloseCall,
            "perfect_guard" => Trigger::PerfectGuard,
            "great_skill" => Trigger::GreatSkill,
            _ => return None,
        })
    }
}

/// A trigger granted by a node: what it reacts to, what it does and its cooldown.
#[derive(Clone, Debug)]
pub struct TriggerDef {
    pub event: Trigger,
    pub effect: String,
    pub cooldown: f32,
}

/// Everything the Weave contributes, folded into one record.
#[derive(Clone, Debug)]
pub struct WeaveMods {
    pub atk: f32,
    pub def: f32,
    pub hp: f32,
    pub foc: f32,
    pub speed: f32,
    pub sp_gain: f32,
    pub dodge_recharge: f32,
    pub dodge_charges: i32,
    pub bounces: i32,
    pub perfect_window: f32,
    pub tether_range: f32,
    /// Share of max life regenerated each second.
    pub regen: f32,
    pub finisher: f32,
    pub aether_gain: f32,
    pub pierce: f32,
    pub aether_time: f32,
    /// Chance a bolt flies one tier stronger.
    pub surge: f32,
    /// Extra melee reach, as a fraction.
    pub reach: f32,
    pub buildup: HashMap<Element, f32>,
    pub passives: HashSet<String>,
    pub triggers: Vec<TriggerDef>,
}

impl Default for WeaveMods {
    fn default() -> Self {
        WeaveMods {
            atk: 1.0,
            def: 1.0,
            hp: 1.0,
            foc: 1.0,
            speed: 1.0,
            sp_gain: 1.0,
            dodge_recharge: 1.0,
            dodge_charges: 0,
            bounces: 0,
            perfect_window: 0.0,
            tether_range: 0.0,
            regen: 0.0,
            finisher: 1.0,
            aether_gain: 1.0,
            pierce: 0.0,
            aether_time: 0.0,
            surge: 0.0,
            reach: 0.0,
            buildup: HashMap::new(),
            passives: HashSet::new(),
            triggers: Vec::new(),
        }
    }
}

impl WeaveMods {
    pub fn buildup_for(&self, e: Element) -> f32 {
        self.buildup.get(&e).copied().unwrap_or(1.0)
    }

    pub fn has(&self, passive: &str) -> bool {
        self.passives.contains(passive)
    }
}

/// Whether a grid cell holds a node: it must lie on a thread (an even row or column) and inside the diamond.
pub fn on_thread(x: i32, y: i32) -> bool {
    (x % 2 == 0 || y % 2 == 0) && x.abs() <= RADIUS && y.abs() <= RADIUS && x.abs() + y.abs() <= REACH
}

use NodeKind::{Keystone, Notable};

/// The named nodes; every other cell on a thread is a minor node of its region.
const NAMED: &[(i32, i32, NodeKind, &str, &str, &str, &str)] = &[
    (0, 0, NodeKind::Hub, HUB, "The Weave", "Where every thread begins.", ""),
    // Might
    (0, -4, Notable, "executioner", "Executioner", "Combo finishers deal 25% more damage.", "finisher:0.25"),
    (-2, -4, Notable, "ricochet", "Ricochet", "Bolts bounce one extra time.", "bounce:1"),
    (2, -4, Notable, "spirit_tap", "Spirit Tap", "Defeating a foe restores 15 SP.", "on:kill:spirit_tap"),
    (
        0,
        -6,
        Notable,
        "heavy_hand",
        "Heavy Hand",
        "ATK +6%, and finishers deal 10% more damage.",
        "atk:0.06,finisher:0.1",
    ),
    (-2, -6, Notable, "gale_force", "Gale Force", "Gust slams into walls deal double damage.", "passive:gust_slam"),
    (2, -6, Notable, "wildfire", "Wildfire", "Foes that die burning spread the flames.", "passive:burn_spread"),
    (-4, -6, Notable, "bloodlust", "Bloodlust", "Defeating a foe restores 3% of your life.", "on:kill:bloodlust"),
    (
        4,
        -6,
        Notable,
        "battle_trance",
        "Battle Trance",
        "Defeating a foe feeds the Aether meter.",
        "on:kill:aether_surge",
    ),
    (
        -2,
        -8,
        Notable,
        "elite_hunter",
        "Elite Hunter",
        "Deal 20% more damage to elites and bosses.",
        "passive:elite_hunter",
    ),
    (2, -8, Notable, "keen_edge", "Keen Edge", "FOC +8%: more critical hits.", "foc:0.08"),
    (
        -2,
        -5,
        Notable,
        "surging_shots",
        "Surging Shots",
        "Bolts have a 15% chance to fly one tier stronger.",
        "surge:0.15",
    ),
    (
        -3,
        -8,
        Notable,
        "tempest_quiver",
        "Tempest Quiver",
        "Bolts surge 10% more often and bounce once more.",
        "surge:0.1,bounce:1",
    ),
    (2, -5, Notable, "long_reach", "Long Reach", "Melee swings reach 15% further.", "reach:0.15"),
    (
        3,
        -8,
        Keystone,
        "iron_vanguard",
        "Iron Vanguard",
        "Melee reaches 10% further, contact hits deal 75% less and light blows can't knock you back, but shots deal 90% less damage.",
        "reach:0.1,passive:iron_vanguard",
    ),
    (
        1,
        -8,
        Notable,
        "whirling_edge",
        "Whirling Edge",
        "Melee swings reach 10% further, and finishers deal 10% more damage.",
        "reach:0.1,finisher:0.1",
    ),
    (0, -8, Keystone, "overcharge", "Overcharge", "ATK +25%, but DEF -25%.", "atk:0.25,def:-0.25"),
    (4, -7, Keystone, "glass_edge", "Glass Edge", "FOC +40%, but life -20%.", "foc:0.4,hp:-0.2"),
    // Ward
    (
        0,
        4,
        Notable,
        "riposte",
        "Riposte",
        "A perfect guard answers with a heavy counter-slash.",
        "on:perfect_guard:riposte",
    ),
    (-2, 4, Notable, "siphon", "Life Siphon", "Perfect guards restore 5% of your life.", "on:perfect_guard:siphon"),
    (
        2,
        4,
        Notable,
        "retaliation",
        "Retaliation",
        "Taking a hit releases a shockwave around you (every 3 seconds at most).",
        "on:hurt:retaliation:3",
    ),
    (0, 6, Notable, "stoneskin", "Stoneskin", "DEF +6% and life +4%.", "def:0.06,hp:0.04"),
    (-2, 6, Notable, "renewal", "Renewal", "Regenerate 0.6% of your life every second.", "regen:0.006"),
    (2, 6, Notable, "deep_chill", "Deep Chill", "Chill lasts longer and freezes come sooner.", "passive:deep_chill"),
    (-4, 6, Notable, "last_stand", "Last Stand", "Below 30% life, you take 25% less damage.", "passive:last_stand"),
    (4, 6, Notable, "guardian", "Guardian", "The perfect guard window is a little wider.", "perfect:0.04"),
    (
        -2,
        8,
        Notable,
        "mending_light",
        "Mending Light",
        "Life +4%, and regenerate 0.4% of it every second.",
        "hp:0.04,regen:0.004",
    ),
    (2, 8, Notable, "iron_will", "Iron Will", "Ailments build up on you 40% slower.", "passive:iron_will"),
    (
        0,
        8,
        Keystone,
        "stillwater",
        "Stillwater",
        "The perfect guard window doubles, but you can hold only one dodge charge.",
        "passive:stillwater",
    ),
    (
        -4,
        7,
        Keystone,
        "bulwark",
        "Bulwark",
        "DEF +30% and hits never stagger you, but you move 15% slower.",
        "def:0.3,speed:-0.15,passive:bulwark",
    ),
    // Swiftness
    (4, 0, Notable, "second_wind", "Second Wind", "One extra dodge charge.", "dodges:1"),
    (
        4,
        -2,
        Notable,
        "phantom_step",
        "Phantom Step",
        "Dodging leaves a decoy that foes attack (every 6 seconds at most).",
        "on:dodge:phantom:6",
    ),
    (4, 2, Notable, "arc_step", "Arc Step", "Dodges shock everything you pass through.", "on:dodge:arc_step"),
    (6, 0, Notable, "fleet", "Fleet", "Move 6% faster.", "speed:0.06"),
    (6, -2, Notable, "blaze_step", "Blaze Step", "Dodges leave a line of fire.", "on:dodge:blaze_step"),
    (6, 2, Notable, "long_tether", "Long Tether", "The Aether Tether reaches 40% further.", "tether:0.4"),
    (
        6,
        -4,
        Notable,
        "acrobat",
        "Acrobat",
        "A close call refunds a dodge charge (every 3 seconds at most).",
        "on:close_call:acrobat:3",
    ),
    (
        6,
        4,
        Notable,
        "slipstream",
        "Slipstream",
        "Dodging feeds the Aether meter (every 2 seconds at most).",
        "on:dodge:aether_surge:2",
    ),
    (
        8,
        -2,
        Notable,
        "wind_runner",
        "Wind Runner",
        "Move 5% faster and dodges recharge 10% faster.",
        "speed:0.05,dodge_recharge:1.1",
    ),
    (8, 2, Notable, "third_wind", "Third Wind", "One extra dodge charge.", "dodges:1"),
    (
        8,
        0,
        Keystone,
        "tailwind",
        "Tailwind",
        "Move 15% faster and dodges recharge 40% faster, but DEF -20%.",
        "passive:tailwind",
    ),
    // Spirit
    (-4, 0, Notable, "spirit_well", "Spirit Well", "SP builds 15% faster.", "sp:1.15"),
    (
        -4,
        -2,
        Notable,
        "overflow",
        "Overflow",
        "Performing a skill at level 3 refunds one SP pip.",
        "on:great_skill:overflow",
    ),
    (
        -4,
        2,
        Notable,
        "chain_shock",
        "Chain Shock",
        "Shock pulses arc to any nearby foe, wet or not.",
        "passive:chain_shock",
    ),
    (
        -6,
        0,
        Notable,
        "deep_well",
        "Deep Well",
        "SP builds 10% faster and ailments 10% faster.",
        "sp:1.1,buildup:all:1.1",
    ),
    (
        -6,
        -2,
        Notable,
        "mist_veil",
        "Mist Veil",
        "A close call hides you in mist (every 6 seconds at most).",
        "on:close_call:mist_veil:6",
    ),
    (
        -6,
        2,
        Notable,
        "tempest_ward",
        "Tempest Ward",
        "A perfect guard raises a ward that reflects projectiles for 2 seconds.",
        "on:perfect_guard:tempest_ward",
    ),
    (-6, -4, Notable, "venom_mastery", "Venom Mastery", "Ailments build 25% faster.", "buildup:all:1.25"),
    (
        -6,
        4,
        Notable,
        "spirit_ward",
        "Spirit Ward",
        "Performing a skill at level 3 wards you for a second.",
        "on:great_skill:spirit_ward",
    ),
    (-8, -2, Notable, "focus_flow", "Focus Flow", "SP builds 10% faster and FOC +5%.", "sp:1.1,foc:0.05"),
    (
        -8,
        2,
        Notable,
        "echoing_arts",
        "Echoing Arts",
        "Performing a skill at level 3 feeds the Aether meter.",
        "on:great_skill:aether_surge",
    ),
    (
        -8,
        0,
        Keystone,
        "blood_art",
        "Blood Art",
        "Skills cost 8% of your life per level instead of SP. They cannot take your last point of life.",
        "passive:blood_art",
    ),
    // Aether
    (
        -4,
        -4,
        Notable,
        "flicker",
        "Flicker",
        "A close call slows every foe to a crawl for 2 seconds (every 12 seconds at most).",
        "on:close_call:flicker:12",
    ),
    (4, -4, Notable, "lingering_light", "Lingering Light", "Aether lasts 2 seconds longer.", "aether_time:2"),
    (-4, 4, Notable, "true_sight", "True Sight", "Aether pierces weaknesses 15% more often.", "pierce:0.15"),
    (
        4,
        4,
        Notable,
        "anchor",
        "Anchor",
        "Aether progress locks a second sooner, and charged bolts lock it like finishers.",
        "passive:anchor",
    ),
    (
        6,
        -5,
        Notable,
        "radiant_core",
        "Radiant Core",
        "Aether lasts 1.5 seconds longer and pierces 10% more often.",
        "aether_time:1.5,pierce:0.1",
    ),
    (
        -6,
        5,
        Notable,
        "afterglow",
        "Afterglow",
        "When Aether fades, a quarter of the meter stays filled and locked.",
        "passive:afterglow",
    ),
    (
        6,
        5,
        Keystone,
        "aether_bond",
        "Aether Bond",
        "Aether always pierces, but lasts half as long.",
        "passive:aether_bond",
    ),
    (
        -5,
        6,
        Keystone,
        "crescendo",
        "Crescendo",
        "Only finishers and charged shots fill the Aether meter, but they fill it twice as fast.",
        "passive:crescendo",
    ),
    (
        -6,
        -5,
        Keystone,
        "unbroken",
        "Unbroken",
        "Aether progress locks in the moment you earn it, but the meter fills 25% slower.",
        "passive:unbroken",
    ),
];

/// A minor node's name, description and effect. Each region rotates three stats along its threads, and the values grow
/// with distance from the hub, so the strongest minors sit deep in a region.
fn minor(region: Region, x: i32, y: i32) -> (String, String, String) {
    let tier = match x.abs() + y.abs() {
        0..=4 => 1.0,
        5..=8 => 2.0,
        _ => 3.0,
    };
    let pct = |v: f32| format!("{}%", (v * 1000.0).round() / 10.0);
    let stat = |name: &str, text: &str, key: &str, v: f32| {
        (name.to_string(), format!("{text} +{}.", pct(v)), format!("{key}:{v}"))
    };
    let mul = |name: &str, text: &str, key: &str, v: f32| {
        (name.to_string(), format!("{text} {} faster.", pct(v)), format!("{key}:{}", 1.0 + v))
    };
    let t = tier;
    match (region, (x + y).rem_euclid(3)) {
        (Region::Might, 0) => stat("Strength", "ATK", "atk", 0.01 * t),
        (Region::Might, 1) if x < 0 => stat("Focus", "FOC", "foc", 0.015 * t),
        (Region::Might, 1) => stat("Reach", "Melee reach", "reach", 0.02 * t),
        (Region::Might, _) if x < 0 => stat("Surge", "Bolt surge chance", "surge", 0.01 * t),
        (Region::Might, _) => stat("Edge", "Finisher damage", "finisher", 0.04 * t),
        (Region::Ward, 0) => stat("Vitality", "Life", "hp", 0.015 * t),
        (Region::Ward, 1) => stat("Toughness", "DEF", "def", 0.01 * t),
        (Region::Ward, _) => (
            "Mending".to_string(),
            format!("Regenerate {}% of your life every second.", (5.0 * t).round() / 100.0),
            format!("regen:{}", 0.0005 * t),
        ),
        (Region::Swiftness, 0) => stat("Fleetness", "Move speed", "speed", 0.01 * t),
        (Region::Swiftness, 1) => mul("Quick Feet", "Dodges recharge", "dodge_recharge", 0.03 * t),
        (Region::Swiftness, _) => stat("Momentum", "ATK", "atk", 0.005 * t),
        (Region::Spirit, 0) => mul("Spirit", "SP builds", "sp", 0.02 * t),
        (Region::Spirit, 1) => (
            "Potency".to_string(),
            format!("Ailments build {} faster.", pct(0.03 * t)),
            format!("buildup:all:{}", 1.0 + 0.03 * t),
        ),
        (Region::Spirit, _) => stat("Resolve", "Life", "hp", 0.0075 * t),
        (Region::Aether, 0) => mul("Attunement", "The Aether meter fills", "aether", 0.03 * t),
        (Region::Aether, 1) => (
            "Clarity".to_string(),
            format!("Aether pierces weaknesses {} more often.", pct(0.02 * t)),
            format!("pierce:{}", 0.02 * t),
        ),
        (Region::Aether, _) => stat("Radiance", "ATK", "atk", 0.005 * t),
        (Region::Hub, _) => (String::new(), String::new(), String::new()),
    }
}

pub static NODES: LazyLock<Vec<WeaveNode>> = LazyLock::new(|| {
    let mut nodes = Vec::new();
    for y in -RADIUS..=RADIUS {
        for x in -RADIUS..=RADIUS {
            if !on_thread(x, y) {
                continue;
            }
            let region = Region::of(x, y);
            let node = match NAMED.iter().find(|n| n.0 == x && n.1 == y) {
                Some(&(_, _, kind, id, name, description, effect)) => WeaveNode {
                    id: id.into(),
                    x,
                    y,
                    region,
                    kind,
                    name: name.into(),
                    description: description.into(),
                    effect: effect.into(),
                },
                None => {
                    let (name, description, effect) = minor(region, x, y);
                    WeaveNode {
                        id: format!("w{x}_{y}"),
                        x,
                        y,
                        region,
                        kind: NodeKind::Minor,
                        name,
                        description,
                        effect,
                    }
                }
            };
            nodes.push(node);
        }
    }
    nodes
});

pub fn by_id(id: &str) -> Option<&'static WeaveNode> {
    NODES.iter().find(|n| n.id == id)
}

pub fn at(x: i32, y: i32) -> Option<&'static WeaveNode> {
    NODES.iter().find(|n| n.x == x && n.y == y)
}

/// The nodes a thread joins directly to this one.
pub fn neighbours(node: &WeaveNode) -> impl Iterator<Item = &'static WeaveNode> + '_ {
    [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().filter_map(move |(dx, dy)| at(node.x + dx, node.y + dy))
}

/// Folds the effects of every taken node into one set of modifiers.
pub fn compute<'a>(taken: impl Iterator<Item = &'a String>) -> WeaveMods {
    let mut mods = WeaveMods::default();
    for id in taken {
        let Some(node) = by_id(id) else { continue };
        for effect in node.effect.split(',').filter(|e| !e.is_empty()) {
            let p: Vec<&str> = effect.split(':').collect();
            let value = |i: usize| p.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            match p[0] {
                "hp" => mods.hp += value(1),
                "atk" => mods.atk += value(1),
                "def" => mods.def += value(1),
                "foc" => mods.foc += value(1),
                "speed" => mods.speed += value(1),
                "sp" => mods.sp_gain *= value(1),
                "dodge_recharge" => mods.dodge_recharge *= value(1),
                "dodges" => mods.dodge_charges += value(1) as i32,
                "bounce" => mods.bounces += value(1) as i32,
                "perfect" => mods.perfect_window += value(1),
                "tether" => mods.tether_range += value(1),
                "regen" => mods.regen += value(1),
                "finisher" => mods.finisher += value(1),
                "aether" => mods.aether_gain *= value(1),
                "pierce" => mods.pierce += value(1),
                "aether_time" => mods.aether_time += value(1),
                "surge" => mods.surge += value(1),
                "reach" => mods.reach += value(1),
                "passive" => {
                    mods.passives.insert(p[1].to_string());
                    if p[1] == "tailwind" {
                        mods.speed *= 1.15;
                        mods.dodge_recharge *= 1.4;
                        mods.def -= 0.2;
                    }
                }
                "buildup" => {
                    let targets = if p[1] == "all" { BASIC.to_vec() } else { vec![Element::parse(Some(p[1]))] };
                    for e in targets {
                        let v = mods.buildup_for(e) * value(2);
                        mods.buildup.insert(e, v);
                    }
                }
                "on" => {
                    if let Some(event) = p.get(1).and_then(|e| Trigger::parse(e)) {
                        mods.triggers.push(TriggerDef {
                            event,
                            effect: p.get(2).copied().unwrap_or("").to_string(),
                            cooldown: value(3),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    mods
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_nodes_sit_on_threads() {
        for n in NAMED {
            assert!(on_thread(n.0, n.1), "{} is off the threads at {},{}", n.3, n.0, n.1);
        }
        let ids: HashSet<&str> = NODES.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids.len(), NODES.len(), "duplicate node ids");
    }

    #[test]
    fn every_node_connects_to_the_hub() {
        let mut reached: HashSet<&str> = HashSet::from([HUB]);
        let mut frontier = vec![by_id(HUB).unwrap()];
        while let Some(node) = frontier.pop() {
            for next in neighbours(node) {
                if reached.insert(next.id.as_str()) {
                    frontier.push(next);
                }
            }
        }
        assert_eq!(reached.len(), NODES.len());
    }

    #[test]
    fn effects_fold_into_mods() {
        let picked: Vec<String> = ["executioner", "riposte", "aether_bond", "w0_-3"].map(String::from).into();
        let mods = compute(picked.iter());
        assert!((mods.finisher - 1.25).abs() < 1e-5);
        assert!((mods.atk - 1.01).abs() < 1e-5);
        assert_eq!(mods.triggers.len(), 1);
        assert_eq!(mods.triggers[0].event, Trigger::PerfectGuard);
        assert!(mods.has("aether_bond"));
    }
}
