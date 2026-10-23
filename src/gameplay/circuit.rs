//! The Circuit: one connected web of passives grown outward from a central hub, with an arm per mode. Every node costs
//! one point and can be taken once a linked node is; keystones also need a Circuit Shard. A node's effect is a short
//! comma-separated list such as `atk:0.04`, `skill_level:fire:2`, `skill:meteor` or `on:perfect_guard:riposte`.

use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;
use std::sync::LazyLock;

use bevy::math::Vec2;

use super::element::{BASIC, Element};

pub const HUB: &str = "hub";
/// Distance between neighbouring nodes in the web's layout, in pixels.
pub const SPACING: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Hub,
    Small,
    Notable,
    Keystone,
}

#[derive(Clone, Debug)]
pub struct CircuitNode {
    pub id: String,
    /// The mode whose arm holds this node; element arms open once that element's sigil is owned.
    pub region: Element,
    pub kind: NodeKind,
    pub name: String,
    pub description: String,
    pub effect: String,
    pub pos: Vec2,
    pub links: Vec<String>,
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

/// A trigger granted by a node: what it reacts to, what it does, its cooldown, and the mode it needs (if any).
#[derive(Clone, Debug)]
pub struct TriggerDef {
    pub event: Trigger,
    pub effect: String,
    pub cooldown: f32,
    pub mode: Option<Element>,
}

/// Everything the Circuit contributes, folded into one record.
#[derive(Clone, Debug)]
pub struct CircuitMods {
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
    pub burst_bonus: f32,
    pub resonance_gain: f32,
    /// Share of max life regenerated each second.
    pub regen: f32,
    pub overload: HashMap<Element, f32>,
    pub buildup: HashMap<Element, f32>,
    pub mode_atk: HashMap<Element, f32>,
    pub passives: HashSet<String>,
    pub skill_levels: HashMap<Element, i32>,
    pub triggers: Vec<TriggerDef>,
}

impl Default for CircuitMods {
    fn default() -> Self {
        CircuitMods {
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
            burst_bonus: 0.0,
            resonance_gain: 1.0,
            regen: 0.0,
            overload: HashMap::new(),
            buildup: HashMap::new(),
            mode_atk: HashMap::new(),
            passives: HashSet::new(),
            skill_levels: HashMap::new(),
            triggers: Vec::new(),
        }
    }
}

impl CircuitMods {
    pub fn overload_for(&self, e: Element) -> f32 {
        self.overload.get(&e).copied().unwrap_or(1.0)
    }

    pub fn buildup_for(&self, e: Element) -> f32 {
        self.buildup.get(&e).copied().unwrap_or(1.0)
    }

    /// The ATK multiplier for fighting in a mode.
    pub fn mode_atk_for(&self, e: Element) -> f32 {
        1.0 + self.mode_atk.get(&e).copied().unwrap_or(0.0)
    }

    pub fn has(&self, passive: &str) -> bool {
        self.passives.contains(passive)
    }

    pub fn skill_level(&self, mode: Element) -> i32 {
        self.skill_levels.get(&mode).copied().unwrap_or(1)
    }
}

impl CircuitNode {
    pub fn needs_shard(&self) -> bool {
        self.kind == NodeKind::Keystone
    }
}

struct Arm {
    region: Element,
    dir: Vec2,
    nodes: Vec<CircuitNode>,
}

impl Arm {
    fn new(region: Element, degrees: f32) -> Arm {
        let a = degrees.to_radians();
        Arm { region, dir: Vec2::new(a.cos(), -a.sin()), nodes: Vec::new() }
    }

    /// Adds a node `along` steps out from the hub and `side` steps to the arm's left (negative) or right.
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        id: &str,
        kind: NodeKind,
        name: &str,
        description: &str,
        effect: &str,
        along: f32,
        side: f32,
        links: &[&str],
    ) {
        let side_dir = Vec2::new(-self.dir.y, self.dir.x);
        self.nodes.push(CircuitNode {
            id: id.into(),
            region: self.region,
            kind,
            name: name.into(),
            description: description.into(),
            effect: effect.into(),
            pos: (self.dir * along + side_dir * side) * SPACING,
            links: links.iter().map(|s| s.to_string()).collect(),
        });
    }
}

use NodeKind::{Keystone, Notable, Small};

struct ElementArm {
    element: Element,
    degrees: f32,
    skills: [(&'static str, &'static str); 2],
    trigger: (&'static str, &'static str, &'static str),
    passive: (&'static str, &'static str, &'static str),
    keystone: (&'static str, &'static str, &'static str),
}

fn element_arm(spec: &ElementArm) -> Vec<CircuitNode> {
    let (k, label) = (spec.element.key(), spec.element.label());
    let id = |s: &str| format!("{k}_{s}");
    let mut arm = Arm::new(spec.element, spec.degrees);
    let edge = format!("mode_atk:{k}:0.04");
    let edge_text = format!("ATK +4% in {label} mode.");
    let potency = format!("buildup:{k}:1.15");
    let potency_text = format!("{label} ailments build 15% faster.");
    let calm = format!("overload:{k}:0.9");
    let calm_text = format!("{label} mode overloads 10% slower.");
    arm.add(&id("edge1"), Small, &format!("{label} Edge"), &edge_text, &edge, 2.0, 0.0, &[HUB]);
    arm.add(&id("potency1"), Small, &format!("{label} Potency"), &potency_text, &potency, 3.0, 0.0, &[&id("edge1")]);
    arm.add(
        &id("technique"),
        Notable,
        &format!("{label} Technique"),
        &format!("{label} skills can be charged to level 2."),
        &format!("skill_level:{k}:2"),
        4.0,
        0.0,
        &[&id("potency1")],
    );
    arm.add(&id("calm1"), Small, &format!("{label} Discipline"), &calm_text, &calm, 4.8, -1.0, &[&id("technique")]);
    let (skill2, skill2_name) = spec.skills[0];
    arm.add(
        &id("skill2"),
        Notable,
        skill2_name,
        &format!("Learn the {label} skill {skill2_name}."),
        &format!("skill:{skill2}"),
        5.6,
        -1.8,
        &[&id("calm1")],
    );
    arm.add(&id("potency2"), Small, &format!("{label} Potency"), &potency_text, &potency, 6.4, -2.4, &[&id("skill2")]);
    let (t_name, t_text, t_effect) = spec.trigger;
    arm.add(&id("trigger"), Notable, t_name, t_text, t_effect, 7.3, -2.8, &[&id("potency2")]);
    arm.add(&id("edge2"), Small, &format!("{label} Edge"), &edge_text, &edge, 5.0, 0.0, &[&id("technique")]);
    arm.add(&id("calm2"), Small, &format!("{label} Discipline"), &calm_text, &calm, 6.0, 0.0, &[&id("edge2")]);
    arm.add(
        &id("apex"),
        Notable,
        &format!("{label} Apex"),
        &format!("{label} skills can be charged to level 3."),
        &format!("skill_level:{k}:3"),
        7.0,
        0.0,
        &[&id("calm2")],
    );
    arm.add(&id("edge3"), Small, &format!("{label} Edge"), &edge_text, &edge, 4.8, 1.0, &[&id("technique")]);
    let (skill3, skill3_name) = spec.skills[1];
    arm.add(
        &id("skill3"),
        Notable,
        skill3_name,
        &format!("Learn the {label} skill {skill3_name}."),
        &format!("skill:{skill3}"),
        5.6,
        1.8,
        &[&id("edge3")],
    );
    arm.add(&id("potency3"), Small, &format!("{label} Potency"), &potency_text, &potency, 6.4, 2.4, &[&id("skill3")]);
    let (p_name, p_text, p_effect) = spec.passive;
    arm.add(&id("passive"), Notable, p_name, p_text, p_effect, 7.3, 2.8, &[&id("potency3")]);
    let (key_name, key_text, key_effect) = spec.keystone;
    arm.add(&id("keystone"), Keystone, key_name, key_text, key_effect, 8.2, 0.0, &[&id("apex")]);
    arm.nodes
}

fn neutral_arm() -> Vec<CircuitNode> {
    let mut arm = Arm::new(Element::None, 90.0);
    arm.add("n_vitality1", Small, "Vitality", "Life +5%.", "hp:0.05", 2.0, 0.0, &[HUB]);
    arm.add("n_strength1", Small, "Strength", "ATK +4%.", "atk:0.04", 3.0, 0.0, &["n_vitality1"]);
    arm.add(
        "n_technique",
        Notable,
        "Neutral Technique",
        "Neutral skills can be charged to level 2.",
        "skill_level:none:2",
        4.0,
        0.0,
        &["n_strength1"],
    );
    arm.add("n_toughness1", Small, "Toughness", "DEF +5%.", "def:0.05", 4.8, -1.0, &["n_technique"]);
    arm.add(
        "n_riposte",
        Notable,
        "Riposte",
        "A perfect guard answers with a heavy counter-slash.",
        "on:perfect_guard:riposte",
        5.6,
        -1.8,
        &["n_toughness1"],
    );
    arm.add(
        "n_form",
        Small,
        "Perfect Form",
        "The perfect guard window widens.",
        "perfect:0.04",
        6.4,
        -2.4,
        &["n_riposte"],
    );
    arm.add(
        "n_siphon",
        Notable,
        "Life Siphon",
        "Perfect guards restore 5% of your life.",
        "on:perfect_guard:siphon",
        7.3,
        -2.8,
        &["n_form"],
    );
    arm.add("n_vitality2", Small, "Vitality", "Life +8%.", "hp:0.08", 8.1, -3.2, &["n_siphon"]);
    arm.add(
        "n_retaliation",
        Notable,
        "Retaliation",
        "Taking a hit releases a shockwave around you (every 3 seconds at most).",
        "on:hurt:retaliation:3",
        8.9,
        -3.6,
        &["n_vitality2"],
    );
    arm.add(
        "n_renewal",
        Notable,
        "Renewal",
        "Regenerate 0.6% of your life every second.",
        "regen:0.006",
        9.7,
        -4.0,
        &["n_retaliation"],
    );
    arm.add("n_focus1", Small, "Focus", "FOC +8%.", "foc:0.08", 4.8, 1.0, &["n_technique"]);
    arm.add(
        "n_triple",
        Notable,
        "Triple Shot",
        "Learn the Neutral skill Triple Shot.",
        "skill:triple_shot",
        5.6,
        1.8,
        &["n_focus1"],
    );
    arm.add("n_ricochet", Small, "Ricochet", "Bolts bounce one extra time.", "bounce:1", 6.4, 2.4, &["n_triple"]);
    arm.add(
        "n_phantom",
        Notable,
        "Phantom Step",
        "Learn the Neutral skill Phantom Step.",
        "skill:phantom_step",
        7.3,
        2.8,
        &["n_ricochet"],
    );
    arm.add(
        "n_feet",
        Small,
        "Quick Feet",
        "Dodges recharge 25% faster.",
        "dodge_recharge:1.25",
        8.1,
        3.2,
        &["n_phantom"],
    );
    arm.add("n_wind", Notable, "Second Wind", "One extra dodge charge.", "dodges:1", 8.9, 3.6, &["n_feet"]);
    arm.add("n_strength2", Small, "Strength", "ATK +4%.", "atk:0.04", 5.0, 0.0, &["n_technique"]);
    arm.add("n_well", Small, "Spirit Well", "SP builds 15% faster.", "sp:1.15", 6.0, 0.0, &["n_strength2"]);
    arm.add(
        "n_apex",
        Notable,
        "Neutral Apex",
        "Neutral skills can be charged to level 3.",
        "skill_level:none:3",
        7.0,
        0.0,
        &["n_well"],
    );
    arm.add(
        "n_tether",
        Small,
        "Long Tether",
        "The Aether Tether reaches 40% further.",
        "tether:0.4",
        8.0,
        0.0,
        &["n_apex"],
    );
    arm.add(
        "n_tap",
        Notable,
        "Spirit Tap",
        "Defeating a foe restores 15 SP.",
        "on:kill:spirit_tap",
        9.0,
        0.0,
        &["n_tether"],
    );
    arm.add(
        "n_keystone",
        Keystone,
        "Blood Art",
        "Skills cost 8% of your life per level instead of SP. They cannot take your last point of life.",
        "passive:blood_art",
        10.0,
        0.0,
        &["n_tap"],
    );
    arm.nodes
}

fn aether_arm() -> Vec<CircuitNode> {
    let mut arm = Arm::new(Element::Aether, 270.0);
    arm.add("a_attune1", Small, "Attunement", "Resonance fills 15% faster.", "resonance:1.15", 2.0, 0.0, &[HUB]);
    arm.add(
        "a_concord",
        Small,
        "Concord",
        "ATK, DEF and FOC +3%.",
        "atk:0.03,def:0.03,foc:0.03",
        3.0,
        0.0,
        &["a_attune1"],
    );
    arm.add(
        "a_technique",
        Notable,
        "Aether Technique",
        "Aether skills can be charged to level 2.",
        "skill_level:aether:2",
        4.0,
        0.0,
        &["a_concord"],
    );
    arm.add(
        "a_ray",
        Notable,
        "Prism Ray",
        "Learn the Aether skill Prism Ray.",
        "skill:prism_ray",
        4.8,
        -1.2,
        &["a_technique"],
    );
    arm.add(
        "a_linger1",
        Small,
        "Lingering Light",
        "Aether bursts last 1 second longer.",
        "burst:1",
        5.6,
        -2.0,
        &["a_ray"],
    );
    arm.add(
        "a_flicker",
        Notable,
        "Flicker",
        "A close call slows every foe to a crawl for 2 seconds (every 12 seconds at most).",
        "on:close_call:flicker:12",
        6.4,
        -2.6,
        &["a_linger1"],
    );
    arm.add(
        "a_sanctum",
        Notable,
        "Sanctum",
        "Learn the Aether skill Sanctum.",
        "skill:sanctum",
        4.8,
        1.2,
        &["a_technique"],
    );
    arm.add(
        "a_attune2",
        Small,
        "Attunement",
        "Resonance fills 15% faster.",
        "resonance:1.15",
        5.6,
        2.0,
        &["a_sanctum"],
    );
    arm.add(
        "a_overflow",
        Notable,
        "Overflow",
        "Performing a skill at level 3 refunds one SP pip.",
        "on:great_skill:overflow",
        6.4,
        2.6,
        &["a_attune2"],
    );
    arm.add(
        "a_linger2",
        Small,
        "Lingering Light",
        "Aether bursts last 1 second longer.",
        "burst:1",
        5.0,
        0.0,
        &["a_technique"],
    );
    arm.add(
        "a_apex",
        Notable,
        "Aether Apex",
        "Aether skills can be charged to level 3.",
        "skill_level:aether:3",
        6.0,
        0.0,
        &["a_linger2"],
    );
    arm.add(
        "a_keystone",
        Keystone,
        "Aether Bond",
        "Aether bursts last 4 seconds longer, but Resonance fills 40% slower.",
        "burst:4,resonance:0.6",
        7.0,
        0.0,
        &["a_apex"],
    );
    arm.nodes
}

pub static NODES: LazyLock<Vec<CircuitNode>> = LazyLock::new(|| {
    let mut nodes = vec![CircuitNode {
        id: HUB.into(),
        region: Element::None,
        kind: NodeKind::Hub,
        name: "The Circuit".into(),
        description: "Where every path begins.".into(),
        effect: String::new(),
        pos: Vec2::ZERO,
        links: Vec::new(),
    }];
    nodes.extend(neutral_arm());
    let arms = [
        ElementArm {
            element: Element::Wind,
            degrees: 30.0,
            skills: [("cyclone", "Cyclone"), ("updraft", "Updraft")],
            trigger: (
                "Tempest Ward",
                "In Wind mode, a perfect guard raises a ward that reflects projectiles for 2 seconds.",
                "on:perfect_guard:tempest_ward:0:wind",
            ),
            passive: ("Gale Force", "Gust slams into walls deal double damage.", "passive:gust_slam"),
            keystone: ("Tailwind", "Move 15% faster and dodges recharge 40% faster, but DEF -20%.", "passive:tailwind"),
        },
        ElementArm {
            element: Element::Fire,
            degrees: -30.0,
            skills: [("meteor", "Meteor"), ("flame_aegis", "Flame Aegis")],
            trigger: ("Blaze Step", "In Fire mode, dodges leave a line of fire.", "on:dodge:blaze_step:0:fire"),
            passive: ("Wildfire", "Foes that die burning spread the flames.", "passive:burn_spread"),
            keystone: (
                "Unbound Flame",
                "Overload never forces you out of your mode. Instead, reaching full overload burns 10% of your life.",
                "passive:unbound",
            ),
        },
        ElementArm {
            element: Element::Water,
            degrees: 210.0,
            skills: [("glacier_lance", "Glacier Lance"), ("ice_mirror", "Ice Mirror")],
            trigger: (
                "Mist Veil",
                "In Water mode, a close call hides you in mist (every 6 seconds at most).",
                "on:close_call:mist_veil:6:water",
            ),
            passive: ("Deep Chill", "Chill lasts longer and freezes come sooner.", "passive:deep_chill"),
            keystone: (
                "Stillwater",
                "The perfect guard window doubles, but you can hold only one dodge charge.",
                "passive:stillwater",
            ),
        },
        ElementArm {
            element: Element::Thunder,
            degrees: 150.0,
            skills: [("storm_call", "Storm Call"), ("static_field", "Static Field")],
            trigger: (
                "Arc Step",
                "In Thunder mode, dodges shock everything you pass through.",
                "on:dodge:arc_step:0:thunder",
            ),
            passive: ("Chain Shock", "Shock pulses arc to any nearby foe, wet or not.", "passive:chain_shock"),
            keystone: ("Overcharge", "ATK +25%, but DEF -25%.", "atk:0.25,def:-0.25"),
        },
    ];
    for spec in &arms {
        nodes.extend(element_arm(spec));
    }
    nodes.extend(aether_arm());
    link_both_ways(&mut nodes);
    nodes
});

/// Makes every link symmetric so the web can be walked from either end.
fn link_both_ways(nodes: &mut [CircuitNode]) {
    let pairs: Vec<(String, String)> =
        nodes.iter().flat_map(|n| n.links.iter().map(move |l| (n.id.clone(), l.clone()))).collect();
    for (from, to) in pairs {
        if let Some(target) = nodes.iter_mut().find(|n| n.id == to)
            && !target.links.contains(&from)
        {
            target.links.push(from);
        }
    }
}

pub fn by_id(id: &str) -> Option<&'static CircuitNode> {
    NODES.iter().find(|n| n.id == id)
}

/// The direction an arm leaves the hub, for laying out labels.
pub fn arm_angle(region: Element) -> f32 {
    match region {
        Element::None => FRAC_PI_2,
        Element::Wind => 30f32.to_radians(),
        Element::Fire => -30f32.to_radians(),
        Element::Water => 210f32.to_radians(),
        Element::Thunder => 150f32.to_radians(),
        Element::Aether => -FRAC_PI_2,
    }
}

/// Folds the effects of every unlocked node into one set of modifiers.
pub fn compute<'a>(unlocked: impl Iterator<Item = &'a String>) -> CircuitMods {
    let mut mods = CircuitMods::default();
    for id in unlocked {
        let Some(node) = by_id(id) else { continue };
        for effect in node.effect.split(',').filter(|e| !e.is_empty()) {
            let p: Vec<&str> = effect.split(':').collect();
            let value = |i: usize| p.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            match p[0] {
                "hp" => mods.hp += value(1),
                "atk" => mods.atk += value(1),
                "def" => mods.def += value(1),
                "foc" => mods.foc += value(1),
                "sp" => mods.sp_gain *= value(1),
                "dodge_recharge" => mods.dodge_recharge *= value(1),
                "dodges" => mods.dodge_charges += value(1) as i32,
                "bounce" => mods.bounces += value(1) as i32,
                "perfect" => mods.perfect_window += value(1),
                "tether" => mods.tether_range += value(1),
                "burst" => mods.burst_bonus += value(1),
                "regen" => mods.regen += value(1),
                "resonance" => mods.resonance_gain *= value(1),
                "passive" => {
                    mods.passives.insert(p[1].to_string());
                    if p[1] == "tailwind" {
                        mods.speed *= 1.15;
                        mods.dodge_recharge *= 1.4;
                        mods.def -= 0.2;
                    }
                }
                "overload" => {
                    for e in targets(p[1]) {
                        let v = mods.overload_for(e) * value(2);
                        mods.overload.insert(e, v);
                    }
                }
                "buildup" => {
                    for e in targets(p[1]) {
                        let v = mods.buildup_for(e) * value(2);
                        mods.buildup.insert(e, v);
                    }
                }
                "mode_atk" => {
                    *mods.mode_atk.entry(Element::parse(Some(p[1]))).or_default() += value(2);
                }
                "skill_level" => {
                    let e = Element::parse(Some(p[1]));
                    let level = mods.skill_level(e).max(value(2) as i32);
                    mods.skill_levels.insert(e, level);
                }
                "on" => {
                    if let Some(event) = p.get(1).and_then(|e| Trigger::parse(e)) {
                        mods.triggers.push(TriggerDef {
                            event,
                            effect: p.get(2).copied().unwrap_or("").to_string(),
                            cooldown: value(3),
                            mode: p.get(4).map(|m| Element::parse(Some(m))),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    mods
}

fn targets(key: &str) -> Vec<Element> {
    if key == "all" { BASIC.to_vec() } else { vec![Element::parse(Some(key))] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_link_points_at_a_real_node() {
        for node in NODES.iter() {
            for link in &node.links {
                assert!(by_id(link).is_some(), "{} links to missing {link}", node.id);
            }
        }
        let ids: HashSet<&str> = NODES.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids.len(), NODES.len(), "duplicate node ids");
    }

    #[test]
    fn effects_fold_into_mods() {
        let picked: Vec<String> = ["fire_technique", "fire_edge1", "n_riposte", "a_keystone"].map(String::from).into();
        let mods = compute(picked.iter());
        assert_eq!(mods.skill_level(Element::Fire), 2);
        assert!((mods.mode_atk_for(Element::Fire) - 1.04).abs() < 1e-5);
        assert_eq!(mods.triggers.len(), 1);
        assert_eq!(mods.triggers[0].event, Trigger::PerfectGuard);
        assert!((mods.burst_bonus - 4.0).abs() < 1e-5);
    }
}
