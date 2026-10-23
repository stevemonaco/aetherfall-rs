//! Items, equipment and sigils.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::element::Element;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Consumable,
    Armor,
    Shield,
    Accessory,
    Key,
    Material,
    /// Permanent upgrades such as boots, rings and shards; owning one is enough (shards stack).
    Powerup,
}

/// Bonuses granted by an equipped accessory or shield. Multipliers default to 1 (no change).
#[derive(Clone, Copy, Debug)]
pub struct AccessoryStats {
    pub atk: f32,
    pub def: f32,
    pub hp: f32,
    pub foc: f32,
    pub speed: f32,
    pub gold: f32,
    pub exp: f32,
    pub sp_gain: f32,
    pub overload: f32,
    pub jump_time: f32,
    pub fire_taken: f32,
    pub life_on_kill: f32,
    pub safe_falls: bool,
    pub dodge_charges: i32,
    pub perfect_window: f32,
    pub auto_revive: bool,
    pub blight_taken: f32,
    pub mastery: f32,
}

impl AccessoryStats {
    pub const NONE: AccessoryStats = AccessoryStats {
        atk: 1.0,
        def: 1.0,
        hp: 1.0,
        foc: 1.0,
        speed: 1.0,
        gold: 1.0,
        exp: 1.0,
        sp_gain: 1.0,
        overload: 1.0,
        jump_time: 1.0,
        fire_taken: 1.0,
        life_on_kill: 0.0,
        safe_falls: false,
        dodge_charges: 0,
        perfect_window: 0.0,
        auto_revive: false,
        blight_taken: 1.0,
        mastery: 1.0,
    };

    pub fn combine(a: &AccessoryStats, b: &AccessoryStats) -> AccessoryStats {
        AccessoryStats {
            atk: a.atk * b.atk,
            def: a.def * b.def,
            hp: a.hp * b.hp,
            foc: a.foc * b.foc,
            speed: a.speed * b.speed,
            gold: a.gold * b.gold,
            exp: a.exp * b.exp,
            sp_gain: a.sp_gain * b.sp_gain,
            overload: a.overload * b.overload,
            jump_time: a.jump_time.max(b.jump_time),
            fire_taken: a.fire_taken * b.fire_taken,
            life_on_kill: a.life_on_kill + b.life_on_kill,
            safe_falls: a.safe_falls || b.safe_falls,
            dodge_charges: a.dodge_charges + b.dodge_charges,
            perfect_window: a.perfect_window + b.perfect_window,
            auto_revive: a.auto_revive || b.auto_revive,
            blight_taken: a.blight_taken * b.blight_taken,
            mastery: a.mastery * b.mastery,
        }
    }
}

impl Default for AccessoryStats {
    fn default() -> Self {
        AccessoryStats::NONE
    }
}

/// An item. `power` is healing % for consumables and DEF for armor and shields.
#[derive(Clone, Debug)]
pub struct ItemDef {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: ItemKind,
    pub price: i32,
    pub power: i32,
    pub description: &'static str,
    pub accessory: AccessoryStats,
    /// Extra max life granted by armor.
    pub hp: i32,
    /// The element a resonance ring attunes.
    pub ring: Element,
}

const fn item(
    id: &'static str,
    name: &'static str,
    kind: ItemKind,
    price: i32,
    power: i32,
    description: &'static str,
) -> ItemDef {
    ItemDef { id, name, kind, price, power, description, accessory: AccessoryStats::NONE, hp: 0, ring: Element::None }
}

impl ItemDef {
    const fn with_hp(mut self, hp: i32) -> Self {
        self.hp = hp;
        self
    }

    const fn with_acc(mut self, acc: AccessoryStats) -> Self {
        self.accessory = acc;
        self
    }

    const fn with_ring(mut self, ring: Element) -> Self {
        self.ring = ring;
        self
    }
}

const N: AccessoryStats = AccessoryStats::NONE;
pub const CONSUMABLE_CAP: i32 = 5;

use ItemKind::*;

static ITEM_LIST: LazyLock<Vec<ItemDef>> = LazyLock::new(|| {
    vec![
        item("herb", "Medical Herb", Consumable, 15, 30, "Restores 30% of your life."),
        item("potion", "Healing Draught", Consumable, 60, 60, "Restores 60% of your life."),
        item("spirit_tonic", "Spirit Tonic", Consumable, 80, 2, "Restores two SP pips."),
        item("elixir", "Elixir", Consumable, 300, 100, "Fully restores life and SP and cures ailments."),
        item("cooling_salve", "Cooling Salve", Consumable, 25, 0, "Cures burn, chill and shock."),
        item("tanned_hide", "Tanned Hide", Armor, 60, 4, "Light armor. DEF +4.").with_hp(10),
        item("chain_mail", "Chain Mail", Armor, 260, 9, "Sturdy armor. DEF +9, life +20.").with_hp(20),
        item("scale_armor", "Scale Armor", Armor, 900, 16, "Armor of serpent scales. DEF +16, life +35.").with_hp(35),
        item("frost_plate", "Frost Plate", Armor, 2200, 25, "Mountain-forged plate. DEF +25, life +55.").with_hp(55),
        item("glass_mail", "Glassweave Mail", Armor, 4200, 34, "Spun from storm glass. DEF +34, life +75.").with_hp(75),
        item("aether_mail", "Aether Mail", Armor, 0, 44, "Woven from sky-city silk. DEF +44, life +100.").with_hp(100),
        item("blight_plate", "Blightforged Plate", Armor, 0, 62, "Tempered in blightfire. DEF +62, life +150.")
            .with_hp(150),
        item("wood_shield", "Wood Shield", Shield, 40, 2, "A simple shield. DEF +2."),
        item("iron_shield", "Iron Shield", Shield, 200, 5, "A heavy shield. DEF +5."),
        item("coral_shield", "Coral Shield", Shield, 700, 9, "Light and strong. DEF +9. Wider perfect guard.")
            .with_acc(AccessoryStats { perfect_window: 0.03, ..N }),
        item("storm_shield", "Storm Shield", Shield, 1800, 14, "Crackles faintly. DEF +14."),
        item("ember_shield", "Ember Bulwark", Shield, 3600, 19, "Forged in the Ashen Reach. DEF +19.")
            .with_acc(AccessoryStats { fire_taken: 0.8, ..N }),
        item("sacred_shield", "Sacred Shield", Shield, 0, 25, "The Concord's own ward. DEF +25. Wider perfect guard.")
            .with_acc(AccessoryStats { perfect_window: 0.05, ..N }),
        item("power_charm", "Power Charm", Accessory, 400, 0, "ATK +10%.").with_acc(AccessoryStats { atk: 1.1, ..N }),
        item("guard_charm", "Guard Charm", Accessory, 400, 0, "DEF +10%, life +10%.").with_acc(AccessoryStats {
            def: 1.1,
            hp: 1.1,
            ..N
        }),
        item("swift_anklet", "Swift Anklet", Accessory, 0, 0, "Move 20% faster.")
            .with_acc(AccessoryStats { speed: 1.2, ..N }),
        item("focus_band", "Focus Band", Accessory, 0, 0, "FOC +25%: more critical hits and stronger ailments.")
            .with_acc(AccessoryStats { foc: 1.25, ..N }),
        item("fortune_coin", "Fortune Coin", Accessory, 350, 0, "Enemies drop 60% more gold.")
            .with_acc(AccessoryStats { gold: 1.6, ..N }),
        item("scholar_lens", "Scholar's Lens", Accessory, 0, 0, "Gain 30% more experience.")
            .with_acc(AccessoryStats { exp: 1.3, ..N }),
        item("mentor_ribbon", "Mentor's Ribbon", Accessory, 450, 0, "Element mastery grows 50% faster.")
            .with_acc(AccessoryStats { mastery: 1.5, ..N }),
        item("sage_pendant", "Sage Pendant", Accessory, 600, 0, "Gain SP 40% faster.")
            .with_acc(AccessoryStats { sp_gain: 1.4, ..N }),
        item("vampire_fang", "Vampire Fang", Accessory, 0, 0, "Recover 3% life per defeated foe.")
            .with_acc(AccessoryStats { life_on_kill: 0.03, ..N }),
        item("feather_charm", "Feather Charm", Accessory, 0, 0, "Jump 40% longer. Falls cause no harm.")
            .with_acc(AccessoryStats { jump_time: 1.4, safe_falls: true, ..N }),
        item("salamander_scale", "Salamander Scale", Accessory, 0, 0, "Fire damage halved. ATK +5%.")
            .with_acc(AccessoryStats { fire_taken: 0.5, atk: 1.05, ..N }),
        item("cool_stone", "Cool Stone", Accessory, 500, 0, "Elemental overload builds 35% slower.")
            .with_acc(AccessoryStats { overload: 0.65, ..N }),
        item("wind_feather", "Wind Feather", Accessory, 0, 0, "One extra dodge charge.")
            .with_acc(AccessoryStats { dodge_charges: 1, ..N }),
        item("parry_ring", "Duelist's Ring", Accessory, 0, 0, "Perfect guard window is much wider.")
            .with_acc(AccessoryStats { perfect_window: 0.08, ..N }),
        item("phoenix_plume", "Phoenix Plume", Accessory, 0, 0, "Once per area, survive a fatal blow.")
            .with_acc(AccessoryStats { auto_revive: true, ..N }),
        item(
            "ember_crown",
            "Ember Crown",
            Accessory,
            0,
            0,
            "The Warlock's echo, crowned. ATK +15%, fire damage halved.",
        )
        .with_acc(AccessoryStats { atk: 1.15, fire_taken: 0.5, ..N }),
        item("tide_locket", "Tide Locket", Accessory, 0, 0, "Holds a drowned tide. SP gain +30%, life +10%.")
            .with_acc(AccessoryStats { sp_gain: 1.3, hp: 1.1, ..N }),
        item(
            "night_eye",
            "Night Eye",
            Accessory,
            0,
            0,
            "Sees what hides. FOC +30%, EXP +20%, cloaked foes show farther.",
        )
        .with_acc(AccessoryStats { foc: 1.3, exp: 1.2, ..N }),
        item("storm_greaves", "Storm Greaves", Accessory, 0, 0, "Move 15% faster. One extra dodge charge.")
            .with_acc(AccessoryStats { speed: 1.15, dodge_charges: 1, ..N }),
        item("corsair_coin", "Corsair's Coin", Accessory, 0, 0, "Double gold. Recover 4% life per defeated foe.")
            .with_acc(AccessoryStats { gold: 2.0, life_on_kill: 0.04, ..N }),
        item(
            "concord_core",
            "Concord Core",
            Accessory,
            0,
            0,
            "The First Concord's heart. ATK, DEF, life +20%. Blight corrupts you half as fast.",
        )
        .with_acc(AccessoryStats { atk: 1.2, def: 1.2, hp: 1.2, blight_taken: 0.5, ..N }),
        item("rabbit_boots", "Rabbit Boots", Powerup, 0, 0, "Jump over pits and narrow water (C / L, or Y)."),
        item(
            "aether_tether",
            "Aether Tether",
            Powerup,
            0,
            0,
            "A latching bolt (F / I, or X): pull yourself to anchors, pull foes and objects to you, rip off shields.",
        ),
        item(
            "memory_lens",
            "Memory Lens",
            Powerup,
            0,
            0,
            "Look through time anywhere in the Bastion (T / L3), not just at memory shards.",
        ),
        item("gale_dash", "Gale Boots", Powerup, 0, 0, "Dodges carry you across gaps up to three tiles wide."),
        item("life_shard", "Life Shard", Powerup, 0, 0, "Every four shards raise your life by 10%."),
        item("spirit_pip", "Spirit Gem", Powerup, 0, 0, "Your SP gauge grows by one pip."),
        item("circuit_shard", "Circuit Shard", Powerup, 0, 0, "Opens a sealed branch of your Circuit."),
        item("circuit_point", "Tome of Insight", Powerup, 0, 0, "Grants one extra Circuit Point."),
        item(
            "wind_orb",
            "Gale Ring",
            Powerup,
            0,
            0,
            "Wind resonance: Wind ailments build 30% faster and overload 25% slower.",
        )
        .with_ring(Element::Wind),
        item(
            "flame_ring",
            "Flame Ring",
            Powerup,
            0,
            0,
            "Fire resonance: Fire ailments build 30% faster and overload 25% slower.",
        )
        .with_ring(Element::Fire),
        item(
            "tide_ring",
            "Tide Ring",
            Powerup,
            0,
            0,
            "Water resonance: Water ailments build 30% faster and overload 25% slower.",
        )
        .with_ring(Element::Water),
        item(
            "storm_ring",
            "Storm Ring",
            Powerup,
            0,
            0,
            "Thunder resonance: Thunder ailments build 30% faster and overload 25% slower.",
        )
        .with_ring(Element::Thunder),
        item(
            "prism_band",
            "Prism Band",
            Powerup,
            0,
            0,
            "Every element resonates: ailments build faster and overload slower.",
        )
        .with_ring(Element::Aether),
        item("grotto_key", "Grotto Key", Key, 0, 0, "Opens the sealed door in the Mossy Grotto."),
        item("temple_key", "Coral Key", Key, 0, 0, "Opens a sealed door in the Sunken Temple."),
        item("spire_key", "Storm Key", Key, 0, 0, "Opens a sealed door in the Storm Spire."),
        item("sky_key", "Sky Key", Key, 0, 0, "Opens a sealed door in the Sky Citadel."),
        item("ember_crystal", "Ember Crystal", Key, 0, 0, "One of the four great crystals, warm as a hearth."),
        item("tide_crystal", "Tide Crystal", Key, 0, 0, "One of the four great crystals. It sounds like the sea."),
        item("storm_crystal", "Storm Crystal", Key, 0, 0, "One of the four great crystals. It hums with thunder."),
        item("slime_gel", "Slime Gel", Material, 4, 0, "Wobbly. Smiths use it to temper blades."),
        item("beast_fang", "Beast Fang", Material, 8, 0, "A sharp fang. Good for edges."),
        item("ember_shard", "Ember Shard", Material, 20, 0, "Still warm. Burns with a steady light."),
        item("tide_pearl", "Tide Pearl", Material, 30, 0, "Cool and smooth, from the deep."),
        item("frost_core", "Frost Core", Material, 45, 0, "Never melts."),
        item("storm_glass", "Storm Glass", Material, 60, 0, "Fused by lightning."),
        item("concord_alloy", "Concord Alloy", Material, 90, 0, "Sky-city metal. Impossibly light."),
        item("blight_core", "Blight Core", Material, 0, 0, "Corrupted aether. The forge can purify it."),
    ]
});

static ITEMS: LazyLock<HashMap<&'static str, ItemDef>> =
    LazyLock::new(|| ITEM_LIST.iter().map(|i| (i.id, i.clone())).collect());

pub fn get(id: &str) -> Option<&'static ItemDef> {
    ITEMS.get(id)
}

/// Every item, in definition order.
pub fn all() -> &'static [ItemDef] {
    &ITEM_LIST
}

/// A sigil set into the Aether Blade's hilt: unlocks an element mode.
#[derive(Clone, Copy, Debug)]
pub struct SigilDef {
    pub id: &'static str,
    pub name: &'static str,
    pub element: Element,
}

pub const SIGILS: [SigilDef; 4] = [
    SigilDef { id: "wind_sigil", name: "Sigil of Wind", element: Element::Wind },
    SigilDef { id: "fire_sigil", name: "Sigil of Fire", element: Element::Fire },
    SigilDef { id: "water_sigil", name: "Sigil of Water", element: Element::Water },
    SigilDef { id: "thunder_sigil", name: "Sigil of Thunder", element: Element::Thunder },
];

pub fn sigil(id: &str) -> Option<&'static SigilDef> {
    SIGILS.iter().find(|s| s.id == id)
}

pub fn name_of(id: &str) -> String {
    if let Some(i) = get(id) {
        return i.name.to_string();
    }
    if let Some(s) = sigil(id) {
        return s.name.to_string();
    }
    if let Some(a) = super::arts::get(id) {
        return a.name.to_string();
    }
    id.to_string()
}

/// Save crystals that fast travel can reach once activated, with the tile the player arrives on.
pub const TOWNS: [(&str, &str, (i32, i32)); 3] = [
    ("leafhollow", "Leafhollow", (19, 13)),
    ("brinewatch", "Brinewatch", (15, 16)),
    ("highcrag", "Highcrag", (17, 16)),
];
