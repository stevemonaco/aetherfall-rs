//! The Circuit skill tree. Each node's effect is a small key/value string, e.g. `atk:0.05`, `art:cyclone`,
//! `overload:fire:0.85` or `passive:perfect_heal`. Shard nodes also need a Circuit Shard.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use super::arts;
use super::element::{BASIC, Element};

#[derive(Clone, Debug)]
pub struct CircuitNode {
    pub id: String,
    pub tree: Element,
    pub name: String,
    pub description: String,
    pub cost: i32,
    pub effect: String,
    pub col: i32,
    pub row: i32,
    pub requires: Vec<String>,
    pub shard: bool,
}

/// Everything the Circuit contributes, folded into one record.
#[derive(Clone, Debug)]
pub struct CircuitMods {
    pub atk: f32,
    pub def: f32,
    pub hp: f32,
    pub foc: f32,
    pub sp_gain: f32,
    pub dodge_recharge: f32,
    pub dodge_charges: i32,
    pub bounces: i32,
    pub perfect_window: f32,
    pub tether_range: f32,
    pub overload: HashMap<Element, f32>,
    pub buildup: HashMap<Element, f32>,
    pub passives: HashSet<String>,
}

impl Default for CircuitMods {
    fn default() -> Self {
        CircuitMods {
            atk: 1.0,
            def: 1.0,
            hp: 1.0,
            foc: 1.0,
            sp_gain: 1.0,
            dodge_recharge: 1.0,
            dodge_charges: 0,
            bounces: 0,
            perfect_window: 0.0,
            tether_range: 0.0,
            overload: HashMap::new(),
            buildup: HashMap::new(),
            passives: HashSet::new(),
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

    pub fn has(&self, passive: &str) -> bool {
        self.passives.contains(passive)
    }
}

#[allow(clippy::too_many_arguments)]
fn node(
    id: &str,
    tree: Element,
    name: &str,
    desc: &str,
    cost: i32,
    effect: &str,
    col: i32,
    row: i32,
    requires: &[&str],
    shard: bool,
) -> CircuitNode {
    CircuitNode {
        id: id.into(),
        tree,
        name: name.into(),
        description: desc.into(),
        cost,
        effect: effect.into(),
        col,
        row,
        requires: requires.iter().map(|s| s.to_string()).collect(),
        shard,
    }
}

#[allow(clippy::too_many_arguments)]
fn element_tree(
    e: Element,
    dodge_art: &str,
    throw_art: &str,
    guard_art: &str,
    passive: &str,
    passive_name: &str,
    passive_desc: &str,
    melee_art: &str,
) -> Vec<CircuitNode> {
    let (k, label) = (e.key(), e.label());
    let art_name = |id: &str| arts::get(id).map(|a| a.name).unwrap_or(id).to_string();
    vec![
        node(
            &format!("{k}_attune1"),
            e,
            &format!("{label} Attunement"),
            &format!("{label} mode overloads 15% slower."),
            1,
            &format!("overload:{k}:0.85"),
            1,
            0,
            &[],
            false,
        ),
        node(
            &format!("{k}_potency"),
            e,
            &format!("{label} Potency"),
            &format!("{label} ailments build 25% faster."),
            2,
            &format!("buildup:{k}:1.25"),
            2,
            0,
            &[],
            false,
        ),
        node(
            &format!("{k}_melee"),
            e,
            &art_name(melee_art),
            &format!("The {label} melee Art, learned with the sword."),
            0,
            &format!("art:{melee_art}"),
            0,
            0,
            &[],
            false,
        ),
        node(
            &format!("{k}_throw"),
            e,
            &art_name(throw_art),
            &format!("Learn the {label} throw Art {}.", art_name(throw_art)),
            2,
            &format!("art:{throw_art}"),
            0,
            1,
            &[&format!("{k}_melee")],
            false,
        ),
        node(
            &format!("{k}_dodge"),
            e,
            &art_name(dodge_art),
            &format!("Learn the {label} dodge Art {}.", art_name(dodge_art)),
            3,
            &format!("art:{dodge_art}"),
            1,
            1,
            &[&format!("{k}_attune1")],
            false,
        ),
        node(
            &format!("{k}_guard"),
            e,
            &art_name(guard_art),
            &format!("Learn the {label} guard Art {}.", art_name(guard_art)),
            3,
            &format!("art:{guard_art}"),
            2,
            1,
            &[&format!("{k}_potency")],
            false,
        ),
        node(&format!("{k}_passive"), e, passive_name, passive_desc, 3, passive, 1, 2, &[&format!("{k}_dodge")], false),
        node(
            &format!("{k}_attune2"),
            e,
            &format!("{label} Attunement II"),
            &format!("{label} mode overloads 20% slower."),
            3,
            &format!("overload:{k}:0.8"),
            2,
            2,
            &[&format!("{k}_guard")],
            false,
        ),
        node(
            &format!("{k}_mastery"),
            e,
            &format!("{label} Mastery"),
            &format!("{label} mode's bonuses grow: ailments +25% and ATK +5% while in it."),
            5,
            &format!("buildup:{k}:1.25,mode_atk:{k}:0.05"),
            1,
            3,
            &[&format!("{k}_passive"), &format!("{k}_attune2")],
            true,
        ),
    ]
}

pub static NODES: LazyLock<Vec<CircuitNode>> = LazyLock::new(|| {
    use Element as E;
    let mut nodes = vec![
        node("core_hp1", E::None, "Vitality", "Life +5%.", 1, "hp:0.05", 0, 0, &[], false),
        node("core_atk1", E::None, "Strength", "ATK +4%.", 1, "atk:0.04", 1, 0, &[], false),
        node("core_def1", E::None, "Toughness", "DEF +5%.", 1, "def:0.05", 2, 0, &[], false),
        node("core_foc1", E::None, "Focus", "FOC +8%.", 1, "foc:0.08", 3, 0, &[], false),
        node(
            "core_whirl",
            E::None,
            "Whirlwind",
            "Learn the Neutral melee Art Whirlwind.",
            2,
            "art:whirl",
            1,
            1,
            &["core_atk1"],
            false,
        ),
        node(
            "core_triple",
            E::None,
            "Triple Shot",
            "Learn the Neutral throw Art Triple Shot.",
            2,
            "art:triple_shot",
            3,
            1,
            &["core_foc1"],
            false,
        ),
        node(
            "core_recover",
            E::None,
            "Quick Feet",
            "Dodges recharge 25% faster.",
            2,
            "dodge_recharge:1.25",
            0,
            1,
            &["core_hp1"],
            false,
        ),
        node(
            "core_counter",
            E::None,
            "Counter Stance",
            "Learn the Neutral guard Art Counter Stance.",
            3,
            "art:counter_stance",
            2,
            1,
            &["core_def1"],
            false,
        ),
        node(
            "core_phantom",
            E::None,
            "Phantom Step",
            "Learn the Neutral dodge Art Phantom Step.",
            3,
            "art:phantom_step",
            0,
            2,
            &["core_recover"],
            false,
        ),
        node(
            "core_spirit",
            E::None,
            "Spirit Well",
            "SP builds 15% faster.",
            2,
            "sp:1.15",
            1,
            2,
            &["core_whirl"],
            false,
        ),
        node(
            "core_ricochet",
            E::None,
            "Ricochet",
            "Bolts bounce one extra time.",
            3,
            "bounce:1",
            3,
            2,
            &["core_triple"],
            false,
        ),
        node(
            "core_form",
            E::None,
            "Perfect Form",
            "The perfect guard window widens.",
            3,
            "perfect:0.04",
            2,
            2,
            &["core_counter"],
            false,
        ),
        node("core_hp2", E::None, "Vitality II", "Life +8%.", 3, "hp:0.08", 0, 3, &["core_phantom"], false),
        node("core_atk2", E::None, "Strength II", "ATK +6%.", 3, "atk:0.06", 1, 3, &["core_spirit"], false),
        node("core_def2", E::None, "Toughness II", "DEF +8%.", 3, "def:0.08", 2, 3, &["core_form"], false),
        node(
            "core_reach",
            E::None,
            "Long Tether",
            "The Aether Tether reaches 40% further.",
            2,
            "tether:0.4",
            3,
            3,
            &["core_ricochet"],
            false,
        ),
        node("core_wind", E::None, "Second Wind", "One extra dodge charge.", 4, "dodges:1", 0, 4, &["core_hp2"], true),
        node(
            "core_siphon",
            E::None,
            "Life Siphon",
            "Perfect guards restore 5% of your life.",
            4,
            "passive:perfect_heal",
            2,
            4,
            &["core_def2"],
            true,
        ),
        node("core_atk3", E::None, "Strength III", "ATK +8%.", 5, "atk:0.08", 1, 4, &["core_atk2"], true),
    ];
    nodes.extend(element_tree(
        E::Wind,
        "updraft",
        "cyclone",
        "tempest_wall",
        "passive:gust_slam",
        "Gale Force",
        "Gust slams into walls deal double damage.",
        "gale_slash",
    ));
    nodes.extend(element_tree(
        E::Fire,
        "blaze_step",
        "meteor",
        "flame_aegis",
        "passive:burn_spread",
        "Wildfire",
        "Foes that die burning spread the flames.",
        "cinder_rush",
    ));
    nodes.extend(element_tree(
        E::Water,
        "mist_step",
        "glacier_lance",
        "ice_mirror",
        "passive:deep_chill",
        "Deep Chill",
        "Chill lasts longer and freezes come sooner.",
        "frost_bloom",
    ));
    nodes.extend(element_tree(
        E::Thunder,
        "arc_step",
        "storm_call",
        "static_field",
        "passive:chain_shock",
        "Chain Shock",
        "Shock pulses arc to any nearby foe, wet or not.",
        "arc_cleave",
    ));
    nodes.extend([
        node(
            "aether_edge",
            E::Aether,
            "Prism Edge",
            "Learn the Aether melee Art Prism Edge.",
            4,
            "art:prism_edge",
            0,
            0,
            &[],
            false,
        ),
        node(
            "aether_ray",
            E::Aether,
            "Prism Ray",
            "Learn the Aether throw Art Prism Ray.",
            4,
            "art:prism_ray",
            1,
            0,
            &[],
            false,
        ),
        node(
            "aether_flicker",
            E::Aether,
            "Flicker",
            "Learn the Aether dodge Art Flicker.",
            4,
            "art:flicker",
            2,
            0,
            &[],
            false,
        ),
        node(
            "aether_sanctum",
            E::Aether,
            "Sanctum",
            "Learn the Aether guard Art Sanctum.",
            4,
            "art:sanctum",
            3,
            0,
            &[],
            false,
        ),
        node(
            "aether_resonance",
            E::Aether,
            "Resonance",
            "Every element's ailments build 15% faster.",
            4,
            "buildup:all:1.15",
            1,
            1,
            &["aether_edge"],
            false,
        ),
        node(
            "aether_calm",
            E::Aether,
            "Stillness",
            "Every element overloads 15% slower.",
            4,
            "overload:all:0.85",
            2,
            1,
            &["aether_flicker"],
            false,
        ),
        node(
            "aether_mastery",
            E::Aether,
            "Concord Mastery",
            "ATK, DEF and FOC +6%.",
            6,
            "atk:0.06,def:0.06,foc:0.06",
            1,
            2,
            &["aether_resonance", "aether_calm"],
            true,
        ),
    ]);
    nodes
});

pub fn by_id(id: &str) -> Option<&'static CircuitNode> {
    NODES.iter().find(|n| n.id == id)
}

/// Folds the effects of every unlocked node into one set of modifiers.
pub fn compute<'a>(unlocked: impl Iterator<Item = &'a String>) -> CircuitMods {
    let mut mods = CircuitMods::default();
    for id in unlocked {
        let Some(node) = by_id(id) else { continue };
        for effect in node.effect.split(',') {
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
                "passive" => {
                    mods.passives.insert(p[1].to_string());
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
                    mods.passives.insert(format!("mode_atk:{}", p[1]));
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
