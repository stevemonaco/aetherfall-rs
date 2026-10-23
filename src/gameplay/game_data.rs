//! The JSON content: enemies, dialogue, shops, towns, quests and the world map, embedded at build time.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::element::Element;

mod content {
    include!(concat!(env!("OUT_DIR"), "/content.rs"));
}

pub use content::IMAGE_FILES;

/// The text of an embedded data file, e.g. `data/maps/fields.json`.
pub fn file(path: &str) -> Option<&'static str> {
    content::DATA_FILES.iter().find(|(p, _)| *p == path).map(|(_, text)| *text)
}

/// One telegraphed attack an enemy can choose. See the enemy module for what each `kind` does.
#[derive(Deserialize, Clone, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct AttackDef {
    #[serde(rename = "type")]
    pub kind: String,
    pub windup: f32,
    pub recover: f32,
    pub cooldown: f32,
    pub range: f32,
    pub min_range: f32,
    pub power: f32,
    pub element: Option<String>,
    pub projectile: String,
    pub count: i32,
    pub spread: f32,
    pub speed: f32,
    pub radius: f32,
    pub distance: f32,
    pub summon: Option<String>,
    pub unblockable: bool,
    pub weight: f32,
}

impl Default for AttackDef {
    fn default() -> Self {
        AttackDef {
            kind: "lunge".into(),
            windup: 0.6,
            recover: 0.6,
            cooldown: 2.5,
            range: 50.0,
            min_range: 0.0,
            power: 1.0,
            element: None,
            projectile: "orb".into(),
            count: 1,
            spread: 0.3,
            speed: 1.0,
            radius: 22.0,
            distance: 64.0,
            summon: None,
            unblockable: false,
            weight: 1.0,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct DropDef {
    pub item: String,
    pub chance: f32,
}

impl Default for DropDef {
    fn default() -> Self {
        DropDef { item: String::new(), chance: 0.1 }
    }
}

/// An enemy kind. `hp`, `attack`, `defense`, `exp` and `gold` are multipliers on the level-based combat formulas.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct EnemyDef {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub sprite: String,
    pub scale: f32,
    pub size: Vec<i32>,
    pub flying: bool,
    pub level: i32,
    pub hp: f32,
    pub attack: f32,
    pub defense: f32,
    pub exp: f32,
    pub gold: f32,
    #[serde(rename = "move")]
    pub movement: String,
    pub speed: f32,
    pub sight: f32,
    pub leash: f32,
    pub resist: HashMap<String, f32>,
    pub guard: f32,
    pub heavy: bool,
    pub contact: f32,
    pub traits: Vec<String>,
    pub attacks: Vec<AttackDef>,
    pub drops: Vec<DropDef>,
}

impl Default for EnemyDef {
    fn default() -> Self {
        EnemyDef {
            id: String::new(),
            name: String::new(),
            sprite: "monster_03".into(),
            scale: 1.0,
            size: vec![12, 10],
            flying: false,
            level: 1,
            hp: 1.0,
            attack: 1.0,
            defense: 1.0,
            exp: 1.0,
            gold: 1.0,
            movement: "walk".into(),
            speed: 24.0,
            sight: 110.0,
            leash: 220.0,
            resist: HashMap::new(),
            guard: 0.0,
            heavy: false,
            contact: 0.4,
            traits: Vec::new(),
            attacks: Vec::new(),
            drops: Vec::new(),
        }
    }
}

impl EnemyDef {
    pub fn resist_to(&self, element: Element) -> f32 {
        self.resist.get(element.key()).copied().unwrap_or(1.0)
    }

    /// A trait's argument ("" for a bare trait), or None when the enemy lacks it.
    pub fn trait_value(&self, name: &str) -> Option<&str> {
        for t in &self.traits {
            if t == name {
                return Some("");
            }
            if let Some(rest) = t.strip_prefix(name).and_then(|r| r.strip_prefix(':')) {
                return Some(rest);
            }
        }
        None
    }

    pub fn has_trait(&self, name: &str) -> bool {
        self.trait_value(name).is_some()
    }

    pub fn width(&self) -> f32 {
        self.size.first().copied().unwrap_or(12) as f32
    }

    pub fn height(&self) -> f32 {
        self.size.get(1).or(self.size.first()).copied().unwrap_or(10) as f32
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct DialogueEntry {
    #[serde(rename = "if")]
    pub condition: Option<String>,
    pub speaker: Option<String>,
    pub lines: Vec<String>,
    #[serde(rename = "do")]
    pub actions: Vec<String>,
    pub ask: Option<String>,
    pub yes: Vec<String>,
    pub no: Vec<String>,
    pub yes_lines: Vec<String>,
    pub no_lines: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ShopDef {
    pub greeting: String,
    pub items: Vec<String>,
}

impl Default for ShopDef {
    fn default() -> Self {
        ShopDef { greeting: "Welcome!".into(), items: Vec::new() }
    }
}

/// A town whose people and buildings return as the listed rifts are sealed.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct TownDef {
    pub name: String,
    pub rifts: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct QuestStage {
    pub text: String,
    pub done: Option<String>,
}

/// A quest is active once the flag `quest:id` is set and complete once every stage's condition holds.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct QuestDef {
    #[serde(skip)]
    pub id: String,
    pub title: String,
    pub giver: String,
    pub region: String,
    pub reward: String,
    pub side: bool,
    pub stages: Vec<QuestStage>,
}

impl Default for QuestDef {
    fn default() -> Self {
        QuestDef {
            id: String::new(),
            title: String::new(),
            giver: String::new(),
            region: String::new(),
            reward: String::new(),
            side: true,
            stages: Vec::new(),
        }
    }
}

/// A region on the world map: where it sits, which maps belong to it and when it appears.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct WorldNode {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub maps: Vec<String>,
    pub links: Vec<String>,
    pub town: Option<String>,
    pub show: Option<String>,
    pub kind: String,
}

impl Default for WorldNode {
    fn default() -> Self {
        WorldNode {
            id: String::new(),
            name: String::new(),
            x: 0,
            y: 0,
            maps: Vec::new(),
            links: Vec::new(),
            town: None,
            show: None,
            kind: "field".into(),
        }
    }
}

pub struct GameData {
    pub enemies: HashMap<String, EnemyDef>,
    pub dialogue: HashMap<String, Vec<DialogueEntry>>,
    pub shops: HashMap<String, ShopDef>,
    pub towns: HashMap<String, TownDef>,
    pub quests: HashMap<String, QuestDef>,
    pub world: Vec<WorldNode>,
}

/// Removes `//` comments and trailing commas, which the hand-written data files use, so serde_json can read them.
pub fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                out.extend(chars.next());
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            while chars.next_if(|&n| n != '\n').is_some() {}
        } else {
            out.push(c);
        }
    }
    remove_trailing_commas(&out)
}

fn remove_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' {
            let next = chars[i + 1..].iter().find(|n| !n.is_whitespace());
            if !matches!(next, Some('}' | ']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

pub fn parse<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let text = file(path).unwrap_or_else(|| panic!("missing data file {path}"));
    serde_json::from_str(&strip_jsonc(text)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

pub static DATA: LazyLock<GameData> = LazyLock::new(|| {
    let mut enemies: HashMap<String, EnemyDef> = parse("data/enemies.json");
    for (id, def) in enemies.iter_mut() {
        def.id = id.clone();
    }
    let mut quests: HashMap<String, QuestDef> = parse("data/quests.json");
    for (id, quest) in quests.iter_mut() {
        quest.id = id.clone();
    }
    GameData {
        enemies,
        dialogue: parse("data/dialogue.json"),
        shops: parse("data/shops.json"),
        towns: parse("data/towns.json"),
        quests,
        world: parse("data/world.json"),
    }
});

pub fn data() -> &'static GameData {
    &DATA
}
