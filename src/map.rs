//! Map files (the JSON written by the PowerShell map scripts) and the terrain grid built from them.

use std::collections::HashMap;

use serde::Deserialize;

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
    pub name: String,
    pub theme: String,
    pub music: Option<String>,
    pub dark: bool,
    pub level: i32,
    pub lava: bool,
    pub rows: Vec<String>,
    pub entities: Vec<EntityData>,
}

impl Default for MapData {
    fn default() -> Self {
        Self {
            name: String::new(),
            theme: "field".into(),
            music: None,
            dark: false,
            level: 1,
            lava: false,
            rows: Vec::new(),
            entities: Vec::new(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
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
}

/// The terrain grid. Out-of-bounds reads repeat the nearest edge so borders render and collide naturally.
pub struct TileMap {
    pub width: i32,
    pub height: i32,
    cells: Vec<Terrain>,
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
        Ok(TileMap { width, height, cells })
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

    pub fn blocks_walking(&self, x: i32, y: i32) -> bool {
        !self.in_bounds(x, y) || self.at(x, y).blocks_walking()
    }

    /// Flyers cross water and pits but not walls or trees.
    pub fn blocks_flying(&self, x: i32, y: i32) -> bool {
        !self.in_bounds(x, y) || self.at(x, y).blocks_projectiles()
    }
}
