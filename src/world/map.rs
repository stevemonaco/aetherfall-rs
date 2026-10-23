//! Map files (the JSON written by the PowerShell map scripts) and the terrain grid built from them.

use std::collections::HashMap;

use bevy::math::Vec2;
use serde::Deserialize;

use crate::core::geom::Box2;

pub const TILE: i32 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Terrain {
    Grass,
    Flowers,
    Path,
    Sand,
    Water,
    Rock,
    Forest,
    Bridge,
    Cobble,
    Floor,
    Pit,
    WoodFloor,
    InteriorWall,
    Snow,
    Ice,
    Bog,
    TallGrass,
    Shallows,
    Metal,
    Blight,
}

impl Terrain {
    pub fn from_char(c: char) -> Option<Terrain> {
        use Terrain::*;
        Some(match c {
            '.' => Grass,
            '"' => Flowers,
            ':' => Path,
            ',' => Sand,
            '~' => Water,
            '#' => Rock,
            'T' => Forest,
            '=' => Bridge,
            '+' => Cobble,
            '_' => Floor,
            ' ' => Pit,
            'o' => WoodFloor,
            'H' => InteriorWall,
            '*' => Snow,
            '-' => Ice,
            ';' => Bog,
            'w' => TallGrass,
            's' => Shallows,
            'm' => Metal,
            'x' => Blight,
            _ => return None,
        })
    }

    pub fn blocks_walking(self) -> bool {
        matches!(self, Terrain::Water | Terrain::Rock | Terrain::Forest | Terrain::Pit | Terrain::InteriorWall)
    }

    pub fn blocks_projectiles(self) -> bool {
        matches!(self, Terrain::Rock | Terrain::Forest | Terrain::InteriorWall)
    }

    /// Raised terrain casts shadows and owns the outline where it meets lower ground.
    pub fn is_raised(self) -> bool {
        matches!(self, Terrain::Rock | Terrain::Forest | Terrain::InteriorWall)
    }

    /// Organic terrains get rounded corners and noisy borders; built surfaces keep crisp tile edges.
    pub fn is_organic(self) -> bool {
        !matches!(self, Terrain::Bridge | Terrain::Cobble | Terrain::WoodFloor | Terrain::InteriorWall | Terrain::Metal)
    }

    /// Which side of a border bulges and draws the outline.
    pub fn priority(self) -> i32 {
        use Terrain::*;
        match self {
            Pit => 0,
            Water => 1,
            Sand | Ice | Bog | Shallows => 2,
            Path | Floor | Blight => 3,
            Bridge | Cobble | WoodFloor | Metal => 4,
            Grass | Flowers | Snow | TallGrass => 5,
            Forest => 7,
            Rock | InteriorWall => 8,
        }
    }

    /// Flowers and tall grass border like grass; blight borders like a path.
    pub fn edge_class(self) -> Terrain {
        match self {
            Terrain::Flowers | Terrain::TallGrass => Terrain::Grass,
            Terrain::Blight => Terrain::Path,
            t => t,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct MapData {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub theme: String,
    pub music: Option<String>,
    pub dark: bool,
    pub level: i32,
    pub ambient: Option<String>,
    pub weather: Option<String>,
    /// A dark map becomes lit once this condition holds (e.g. a lighthouse restored).
    pub light_if: Option<String>,
    /// The same place in another era, for the Memory Lens (maps share coordinates).
    pub pair: Option<String>,
    pub era: Option<String>,
    /// Heat gained per second outside Water mode (the Caldera); 0 for temperate places.
    pub heat: f32,
    /// Pits here are lava: falling in burns badly.
    pub lava: bool,
    /// A Blighted expedition: its foes revive once unless finished by their marked element or an Art.
    pub blighted: bool,
    /// Foes here are only visible near the player.
    pub cloaked: bool,
    /// The Blighted area this map belongs to, for completion grades.
    pub blight_area: Option<String>,
    pub rows: Vec<String>,
    pub entities: Vec<EntityData>,
}

impl Default for MapData {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            theme: "field".into(),
            music: None,
            dark: false,
            level: 1,
            ambient: None,
            weather: None,
            light_if: None,
            pair: None,
            era: None,
            heat: 0.0,
            lava: false,
            blighted: false,
            cloaked: false,
            blight_area: None,
            rows: Vec::new(),
            entities: Vec::new(),
        }
    }
}

impl MapData {
    pub fn load(id: &str) -> MapData {
        let mut data: MapData = crate::gameplay::game_data::parse(&format!("data/maps/{id}.json"));
        data.id = id.to_string();
        data
    }

    pub fn exists(id: &str) -> bool {
        crate::gameplay::game_data::file(&format!("data/maps/{id}.json")).is_some()
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct EntityData {
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f32,
    pub y: f32,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl EntityData {
    pub fn str(&self, key: &str) -> Option<&str> {
        self.extra.get(key).and_then(|v| v.as_str())
    }

    pub fn str_or<'a>(&'a self, key: &str, fallback: &'a str) -> &'a str {
        self.str(key).unwrap_or(fallback)
    }

    pub fn int(&self, key: &str, fallback: i32) -> i32 {
        self.extra.get(key).and_then(|v| v.as_f64()).map(|v| v as i32).unwrap_or(fallback)
    }

    pub fn float(&self, key: &str, fallback: f32) -> f32 {
        self.extra.get(key).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(fallback)
    }

    pub fn bool(&self, key: &str, fallback: bool) -> bool {
        self.extra.get(key).and_then(|v| v.as_bool()).unwrap_or(fallback)
    }

    pub fn strings(&self, key: &str) -> Vec<String> {
        match self.extra.get(key) {
            Some(serde_json::Value::Array(items)) => items.iter().map(|i| i.as_str().unwrap_or("").to_string()).collect(),
            _ => Vec::new(),
        }
    }

    pub fn tile(&self) -> (i32, i32) {
        (self.x as i32, self.y as i32)
    }
}

/// The terrain grid. Out-of-bounds reads repeat the nearest edge so borders render and collide naturally.
pub struct TileMap {
    pub width: i32,
    pub height: i32,
    cells: Vec<Terrain>,
    blocked: Vec<bool>,
}

impl TileMap {
    pub fn new(data: &MapData) -> Result<TileMap, String> {
        let height = data.rows.len() as i32;
        let width = data.rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
        let mut cells = Vec::with_capacity((width * height) as usize);
        for (y, row) in data.rows.iter().enumerate() {
            let chars: Vec<char> = row.chars().collect();
            for x in 0..width as usize {
                let c = chars.get(x).copied().unwrap_or(' ');
                cells.push(Terrain::from_char(c).ok_or_else(|| format!("unknown terrain '{c}' at {x},{y}"))?);
            }
        }
        Ok(TileMap { width, height, cells, blocked: vec![false; (width * height) as usize] })
    }

    pub fn pixel_width(&self) -> i32 {
        self.width * TILE
    }

    pub fn pixel_height(&self) -> i32 {
        self.height * TILE
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn at(&self, x: i32, y: i32) -> Terrain {
        let cx = x.clamp(0, self.width - 1);
        let cy = y.clamp(0, self.height - 1);
        self.cells[(cy * self.width + cx) as usize]
    }

    pub fn set_terrain(&mut self, x: i32, y: i32, t: Terrain) {
        if self.in_bounds(x, y) {
            self.cells[(y * self.width + x) as usize] = t;
        }
    }

    /// Marks a cell as blocked by a static object such as a building, independent of its terrain.
    pub fn block(&mut self, x: i32, y: i32, blocked: bool) {
        if self.in_bounds(x, y) {
            self.blocked[(y * self.width + x) as usize] = blocked;
        }
    }

    fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x, y) && self.blocked[(y * self.width + x) as usize]
    }

    pub fn blocks_walking(&self, x: i32, y: i32) -> bool {
        !self.in_bounds(x, y) || self.at(x, y).blocks_walking() || self.is_blocked(x, y)
    }

    pub fn blocks_projectiles(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x, y) && self.at(x, y).blocks_projectiles()
    }

    /// Flyers cross water and pits but not walls, trees, or buildings.
    pub fn blocks_flying(&self, x: i32, y: i32) -> bool {
        !self.in_bounds(x, y) || self.at(x, y).blocks_projectiles() || self.is_blocked(x, y)
    }

    pub fn box_blocked(&self, b: &Box2, blocks: impl Fn(i32, i32) -> bool) -> bool {
        let t = TILE as f32;
        let (x0, x1) = ((b.x / t).floor() as i32, ((b.right() - 0.001) / t).floor() as i32);
        let (y0, y1) = ((b.y / t).floor() as i32, ((b.bottom() - 0.001) / t).floor() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if blocks(x, y) {
                    return true;
                }
            }
        }
        false
    }
}

pub fn tile_center(x: i32, y: i32) -> Vec2 {
    Vec2::new((x * TILE) as f32 + TILE as f32 / 2.0, (y * TILE) as f32 + TILE as f32 / 2.0)
}

/// Where an actor placed on a tile stands: the tile centre, nudged down so its feet sit near the bottom.
pub fn tile_feet(x: i32, y: i32) -> Vec2 {
    tile_center(x, y) + Vec2::new(0.0, 6.0)
}

impl Terrain {
    /// Hazards a jumping player may cross but must not land in.
    pub fn is_hazard(self) -> bool {
        matches!(self, Terrain::Water | Terrain::Pit)
    }
}