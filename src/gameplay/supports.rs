//! Support gems: socketed into a mode's attack, ranged or special group, each reshapes those hits and carries a
//! cost. A gem is a single item, so it can sit in only one socket. Prism gems fit only the Aether page's socket.

use super::combat::HitKind;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Group {
    Attack,
    Ranged,
    Special,
}

impl Group {
    pub const ALL: [Group; 3] = [Group::Attack, Group::Ranged, Group::Special];

    pub fn key(self) -> &'static str {
        match self {
            Group::Attack => "attack",
            Group::Ranged => "ranged",
            Group::Special => "special",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Group::Attack => "Attack",
            Group::Ranged => "Ranged",
            Group::Special => "Special",
        }
    }

    /// The group a hit belongs to, if any.
    pub fn of(kind: HitKind) -> Option<Group> {
        match kind {
            HitKind::Melee | HitKind::Finisher => Some(Group::Attack),
            HitKind::Bolt | HitKind::ChargedBolt => Some(Group::Ranged),
            HitKind::Skill => Some(Group::Special),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SupportDef {
    pub id: &'static str,
    pub name: &'static str,
    /// The only group this gem fits, or None for any group.
    pub fits: Option<Group>,
    pub prism: bool,
    pub description: &'static str,
}

impl SupportDef {
    pub fn fits_group(&self, group: Group) -> bool {
        self.fits.is_none_or(|g| g == group)
    }
}

/// The highest Gemcutter tier a gem can reach.
pub const MAX_TIER: i32 = 4;

/// A gem's name with its Gemcutter tier, e.g. "Flawless Multishot Gem".
pub fn tier_name(gem: &SupportDef, tier: i32) -> String {
    match tier.clamp(0, MAX_TIER) {
        1 => format!("Cut {}", gem.name),
        2 => format!("Polished {}", gem.name),
        3 => format!("Flawless {}", gem.name),
        4 => format!("Perfect {}", gem.name),
        _ => gem.name.to_string(),
    }
}

/// Prisms it takes to hone a gem from `tier` to the next: one more for every tier already cut.
pub fn prisms_for(tier: i32) -> i32 {
    tier + 1
}

/// Gems whose upside is a new behaviour; tiers make them hit harder instead of scaling a number.
const HONED: [&str; 11] = [
    "momentum_gem",
    "multishot_gem",
    "pierce_gem",
    "linger_gem",
    "echo_gem",
    "efficiency_gem",
    "lunge_gem",
    "sweep_gem",
    "whirlwind_gem",
    "deflect_gem",
    "seeking_gem",
];

/// A gem's description with its Gemcutter tier folded into the numbers: the upside (before ", but") grows by a
/// tenth per tier and the drawback after it shrinks by a sixth.
pub fn describe(gem: &SupportDef, tier: i32) -> String {
    let t = tier.clamp(0, MAX_TIER) as f32;
    if t == 0.0 {
        return gem.description.to_string();
    }
    let honed = HONED.contains(&gem.id);
    let (up, down) = gem.description.split_once(", but ").unwrap_or((gem.description, ""));
    let up = if honed { up.to_string() } else { scale_percents(up, 1.0 + 0.1 * t) };
    let mut text = if down.is_empty() { up } else { format!("{up}, but {}", scale_percents(down, 1.0 - t / 6.0)) };
    if honed {
        text.push_str(&format!(" Honed: {}% more damage.", (4.0 * t) as i32));
    }
    text
}

fn scale_percents(text: &str, factor: f32) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        if !digits.is_empty() {
            let n: f32 = digits.parse().unwrap_or(0.0);
            if c == '%' {
                let v = n * factor;
                let shown = if v < 10.0 {
                    format!("{:.1}", v).trim_end_matches(".0").to_string()
                } else {
                    format!("{}", v.round() as i32)
                };
                out.push_str(&shown);
            } else {
                out.push_str(&digits);
            }
            digits.clear();
        }
        out.push(c);
    }
    out.push_str(&digits);
    out
}

const fn gem(id: &'static str, name: &'static str, fits: Option<Group>, description: &'static str) -> SupportDef {
    SupportDef { id, name, fits, prism: false, description }
}

const fn prism(id: &'static str, name: &'static str, description: &'static str) -> SupportDef {
    SupportDef { id, name, fits: None, prism: true, description }
}

use Group::*;

pub const ALL: [SupportDef; 22] = [
    gem("cleave_gem", "Cleave Gem", Some(Attack), "Melee swings reach 30% wider, but deal 15% less damage."),
    gem(
        "momentum_gem",
        "Momentum Gem",
        Some(Attack),
        "Combo finishers deal 60% more damage; the first two swings 20% less.",
    ),
    gem(
        "multishot_gem",
        "Multishot Gem",
        Some(Ranged),
        "Bolts fly in a spread of three that strikes each foe once, but deal 40% less damage.",
    ),
    gem("pierce_gem", "Piercing Gem", Some(Ranged), "Every bolt pierces, but deals 20% less damage."),
    gem(
        "lunge_gem",
        "Lunging Gem",
        Some(Attack),
        "Swings dash you toward the nearest foe ahead, but deal 10% less damage.",
    ),
    gem("sweep_gem", "Sweeping Gem", Some(Attack), "Swings sweep all the way around you, but deal 15% less damage."),
    gem(
        "whirlwind_gem",
        "Whirlwind Gem",
        Some(Attack),
        "Hold attack to whirl, striking everything around you as you move. Whirling burns SP.",
    ),
    gem(
        "deflect_gem",
        "Deflecting Gem",
        Some(Attack),
        "Swings cut down enemy shots and finishers send them back, but swings are 10% slower.",
    ),
    gem(
        "sunder_gem",
        "Sundering Gem",
        Some(Attack),
        "Melee hits wear shields twice as fast and break guards 50% faster, but deal 10% less damage.",
    ),
    gem("surge_gem", "Surging Gem", Some(Ranged), "Bolts have a 25% chance to fly one tier stronger."),
    gem("seeking_gem", "Seeking Gem", Some(Ranged), "Bolts curve toward foes, but deal 15% less damage."),
    gem(
        "executioner_gem",
        "Executioner Gem",
        None,
        "Hits deal 35% more to broken or frozen foes, but 10% less to anything else.",
    ),
    gem(
        "linger_gem",
        "Lingering Gem",
        Some(Special),
        "Skill hits leave their element on the ground, but skills charge 20% slower.",
    ),
    gem(
        "echo_gem",
        "Echo Gem",
        Some(Special),
        "Skills repeat once at half power, but the Aether meter fills 30% slower.",
    ),
    gem(
        "efficiency_gem",
        "Efficiency Gem",
        Some(Special),
        "Levels 2 and 3 cost one pip less, but skills deal 15% less damage.",
    ),
    gem("haste_gem", "Haste Gem", Some(Special), "Skills charge 40% faster, but deal 10% less damage."),
    gem("brutal_gem", "Brutal Gem", None, "Hits break guards 50% faster, but build ailments 30% slower."),
    gem("infusion_gem", "Infusion Gem", None, "Hits build ailments 40% faster, but deal 15% less damage."),
    gem("leech_gem", "Leech Gem", None, "Hits return 2% of their damage as life, but build 25% less SP."),
    gem("overdrive_gem", "Overdrive Gem", None, "Hits deal 25% more damage, but the Aether meter fills 40% slower."),
    prism("prism_lens", "Prism Lens", "Aether pierces weaknesses 20% more often, but lasts 1.5 seconds less."),
    prism("everlight", "Everlight", "Aether lasts 3 seconds longer, but its meter fills 20% slower."),
];

pub fn get(id: &str) -> Option<&'static SupportDef> {
    ALL.iter().find(|s| s.id == id)
}

/// The combined effect of the gems in one socket group.
#[derive(Clone, Copy, Debug)]
pub struct SupportMods {
    pub damage: f32,
    pub poise: f32,
    pub status: f32,
    /// Multiplier on how fast these hits fill the Aether meter.
    pub aether: f32,
    pub sp_gain: f32,
    pub leech: f32,
    pub charge_speed: f32,
    /// How much farther melee swings reach (1.0 without the Cleave gem).
    pub cleave: f32,
    pub momentum: bool,
    pub multishot: bool,
    pub pierce: bool,
    pub linger: bool,
    pub echo: bool,
    pub efficiency: bool,
    pub lunge: bool,
    pub sweep: bool,
    pub whirlwind: bool,
    pub deflect: bool,
    pub sunder: bool,
    /// Chance a bolt flies one tier stronger.
    pub surge: f32,
    pub seeking: bool,
    /// Damage multiplier against broken or frozen foes.
    pub execute: f32,
    /// Multiplier on how fast swings play out.
    pub swing_speed: f32,
}

impl Default for SupportMods {
    fn default() -> Self {
        SupportMods {
            damage: 1.0,
            poise: 1.0,
            status: 1.0,
            aether: 1.0,
            sp_gain: 1.0,
            leech: 0.0,
            charge_speed: 1.0,
            cleave: 1.0,
            momentum: false,
            multishot: false,
            pierce: false,
            linger: false,
            echo: false,
            efficiency: false,
            lunge: false,
            sweep: false,
            whirlwind: false,
            deflect: false,
            sunder: false,
            surge: 0.0,
            seeking: false,
            execute: 1.0,
            swing_speed: 1.0,
        }
    }
}

impl SupportMods {
    /// Folds socketed gems, each with its Gemcutter tier (0 to `MAX_TIER`), into one set of modifiers. Every tier
    /// grows a gem's upside by a tenth and trims a sixth off its drawback; gems whose upside is a new behaviour hit a
    /// little harder instead.
    pub fn from_gems<'a>(gems: impl Iterator<Item = (&'a SupportDef, i32)>) -> SupportMods {
        let mut m = SupportMods::default();
        for (gem, tier) in gems {
            let t = tier.clamp(0, MAX_TIER) as f32;
            let up = |v: f32| 1.0 + (v - 1.0) * (1.0 + 0.1 * t);
            let down = |v: f32| 1.0 - (1.0 - v) * (1.0 - t / 6.0);
            let honed = 1.0 + 0.04 * t;
            match gem.id {
                "cleave_gem" => {
                    m.cleave = up(1.3);
                    m.damage *= down(0.85);
                }
                "momentum_gem" => {
                    m.momentum = true;
                    m.damage *= honed;
                }
                "multishot_gem" => {
                    m.multishot = true;
                    m.damage *= down(0.6) * honed;
                }
                "pierce_gem" => {
                    m.pierce = true;
                    m.damage *= down(0.8) * honed;
                }
                "linger_gem" => {
                    m.linger = true;
                    m.charge_speed *= down(0.8);
                    m.damage *= honed;
                }
                "echo_gem" => {
                    m.echo = true;
                    m.aether *= down(0.7);
                    m.damage *= honed;
                }
                "efficiency_gem" => {
                    m.efficiency = true;
                    m.damage *= down(0.85) * honed;
                }
                "haste_gem" => {
                    m.charge_speed *= up(1.4);
                    m.damage *= down(0.9);
                }
                "brutal_gem" => {
                    m.poise *= up(1.5);
                    m.status *= down(0.7);
                }
                "infusion_gem" => {
                    m.status *= up(1.4);
                    m.damage *= down(0.85);
                }
                "leech_gem" => {
                    m.leech += 0.02 * (1.0 + 0.1 * t);
                    m.sp_gain *= down(0.75);
                }
                "overdrive_gem" => {
                    m.damage *= up(1.25);
                    m.aether *= down(0.6);
                }
                "lunge_gem" => {
                    m.lunge = true;
                    m.damage *= down(0.9) * honed;
                }
                "sweep_gem" => {
                    m.sweep = true;
                    m.damage *= down(0.85) * honed;
                }
                "whirlwind_gem" => {
                    m.whirlwind = true;
                    m.damage *= honed;
                }
                "deflect_gem" => {
                    m.deflect = true;
                    m.swing_speed *= down(0.9);
                    m.damage *= honed;
                }
                "sunder_gem" => {
                    m.sunder = true;
                    m.poise *= up(1.5);
                    m.damage *= down(0.9);
                }
                "surge_gem" => m.surge += 0.25 * (1.0 + 0.1 * t),
                "seeking_gem" => {
                    m.seeking = true;
                    m.damage *= down(0.85) * honed;
                }
                "executioner_gem" => {
                    m.execute *= up(1.5);
                    m.damage *= down(0.9);
                }
                _ => {}
            }
        }
        m
    }
    /// The damage multiplier for a hit of this kind, including Momentum's finisher bonus.
    pub fn damage_for(&self, kind: HitKind) -> f32 {
        let momentum = match (self.momentum, kind) {
            (true, HitKind::Finisher) => 1.6,
            (true, HitKind::Melee) => 0.8,
            _ => 1.0,
        };
        self.damage * momentum
    }

    /// SP pips a skill level costs.
    pub fn skill_cost(&self, level: i32) -> i32 {
        if self.efficiency && level > 1 { level - 1 } else { level }
    }
}
