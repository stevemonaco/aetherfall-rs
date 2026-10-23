//! Four-step colour ramps (dark to light) for each terrain in a region.

use std::collections::HashMap;

use crate::core::color::{Rgba, ramp};
use crate::world::map::Terrain;

pub struct TerrainTheme {
    pub name: &'static str,
    pub outline: Rgba,
    pub foam: Rgba,
    pub flower_colors: Vec<Rgba>,
    pub trunk: Rgba,
    ramps: HashMap<Terrain, Vec<Rgba>>,
}

impl TerrainTheme {
    pub fn ramp(&self, t: Terrain) -> &[Rgba] {
        self.ramps.get(&t).unwrap_or_else(|| &self.ramps[&Terrain::Grass])
    }

    fn base(name: &'static str, ramps: &[(Terrain, [u32; 4])]) -> TerrainTheme {
        TerrainTheme {
            name,
            outline: Rgba::hex(0x14101C),
            foam: Rgba::hex(0xE8F6FF),
            flower_colors: ramp(&[0xFFFFFF, 0xF06070, 0xF8D850]),
            trunk: Rgba::hex(0x5A3A22),
            ramps: ramps.iter().map(|(t, hex)| (*t, ramp(hex))).collect(),
        }
    }

    pub fn get(name: &str) -> TerrainTheme {
        use Terrain::*;
        match name {
            "ash" => TerrainTheme {
                outline: Rgba::hex(0x120E10),
                foam: Rgba::hex(0xB8F0E8),
                flower_colors: ramp(&[0xE07040, 0xC8C0B0, 0x8A8090]),
                trunk: Rgba::hex(0x1E1614),
                ..Self::base(
                    "ash",
                    &[
                        (Grass, [0x3A3530, 0x5A524A, 0x7A7064, 0x9A9080]),
                        (Flowers, [0x3A3530, 0x5A524A, 0x7A7064, 0x9A9080]),
                        (Sand, [0x4A4448, 0x6E686C, 0x908A8C, 0xB8B2B0]),
                        (Path, [0x2A2020, 0x4A3A34, 0x6A5448, 0x8A7060]),
                        (Rock, [0x0E0C10, 0x221E26, 0x3A343E, 0x5A5260]),
                        (Forest, [0x0A0808, 0x1A1414, 0x2E2424, 0x4A3A36]),
                        (Water, [0x0E3A40, 0x1A6A70, 0x2AA0A0, 0x9AF0E0]),
                        (Floor, [0x2A2830, 0x48464E, 0x66646E, 0x8A8892]),
                        (Cobble, [0x3A3440, 0x5A5262, 0x7A7286, 0xA09AAE]),
                        (Bridge, [0x2A1C14, 0x4A3424, 0x6A4C34, 0x8A6A4A]),
                        (Pit, [0x1A0604, 0x3A0E08, 0x5A1A0C, 0x8A3010]),
                        (Bog, [0x1E1A14, 0x302A20, 0x423A2C, 0x6A5A40]),
                    ],
                )
            },
            "ruin" => TerrainTheme {
                outline: Rgba::hex(0x100C18),
                foam: Rgba::hex(0xC8D0F0),
                ..Self::base(
                    "ruin",
                    &[
                        (Floor, [0x3A3448, 0x524A64, 0x6C6482, 0x8C84A0]),
                        (Cobble, [0x4A3E36, 0x6E5E4E, 0x8E7C66, 0xB09C82]),
                        (Rock, [0x14101C, 0x2A2436, 0x403852, 0x5E5476]),
                        (Water, [0x1A2A4A, 0x2A4A7A, 0x3E6AA8, 0x8AAAE0]),
                        (Pit, [0x1A0A08, 0x3A1410, 0x5A2418, 0x7A3A20]),
                        (Grass, [0x2A2E2A, 0x3E443E, 0x545C52, 0x707A6C]),
                        (Sand, [0x4A4448, 0x6E686C, 0x908A8C, 0xB8B2B0]),
                        (Bridge, [0x2A2414, 0x4A3E2A, 0x6A5A3E, 0x8A7A58]),
                        (Forest, [0x0A0808, 0x1A1414, 0x2E2424, 0x4A3A36]),
                    ],
                )
            },
            "cave" => TerrainTheme {
                outline: Rgba::hex(0x08070E),
                foam: Rgba::hex(0x9FD3F5),
                trunk: Rgba::hex(0x2C2840),
                ..Self::base(
                    "cave",
                    &[
                        (Grass, [0x14262A, 0x1E3C38, 0x2A5244, 0x3E6E52]),
                        (Floor, [0x1A1A26, 0x2E2E42, 0x424258, 0x5C5C78]),
                        (Path, [0x1A1A26, 0x2E2E42, 0x424258, 0x5C5C78]),
                        (Sand, [0x3A3030, 0x5A4A44, 0x7A6658, 0x9A8470]),
                        (Water, [0x0E1840, 0x1C3678, 0x2A54A8, 0x6AA0DA]),
                        (Rock, [0x14121E, 0x2C2840, 0x4A4466, 0x6E6890]),
                        (Forest, [0x0E1C1C, 0x1A3434, 0x28504A, 0x3E7064]),
                        (Bridge, [0x2A1C14, 0x5A3C24, 0x7E5634, 0xA0744A]),
                        (Cobble, [0x1E1C28, 0x3A3850, 0x56546E, 0x76748E]),
                        (Pit, [0x000000, 0x0A0812, 0x16121E, 0x221C2E]),
                    ],
                )
            },
            "interior" => TerrainTheme {
                outline: Rgba::hex(0x120C10),
                ..Self::base(
                    "interior",
                    &[
                        (WoodFloor, [0x3A2414, 0x7A4A26, 0x9C6434, 0xBC844C]),
                        (InteriorWall, [0x2A1E1A, 0x8A6A52, 0xC8AE88, 0xE6D6B4]),
                        (Cobble, [0x3A3640, 0x6E6878, 0x9A94A2, 0xC8C2CC]),
                        (Floor, [0x4A3428, 0x7A5A44, 0x9A7A5E, 0xBA9C7C]),
                        (Pit, [0x050408, 0x14101C, 0x241C30, 0x302840]),
                        (Grass, [0x1F4A2C, 0x3A7A3A, 0x5AA845, 0x8ED15A]),
                        (Water, [0x1B2F6B, 0x2D5BB0, 0x3F86D8, 0x9FD3F5]),
                        (Rock, [0x2A1E1A, 0x6A4E3A, 0x9A7656, 0xC8A27A]),
                    ],
                )
            },
            "snow" => Self::base(
                "snow",
                &[
                    (Grass, [0x5A6A8A, 0xA8B8D8, 0xDCE6F5, 0xFFFFFF]),
                    (Snow, [0x5A6A8A, 0xA8B8D8, 0xDCE6F5, 0xFFFFFF]),
                    (Ice, [0x3A6AA0, 0x7AB0E0, 0xA8D8F4, 0xE8FAFF]),
                    (Floor, [0x3A3A54, 0x5A5A7A, 0x7A7A9A, 0x9C9CBC]),
                    (Path, [0x4A4060, 0x7A7090, 0x9A92B0, 0xBCB6D0]),
                    (Water, [0x1B2F6B, 0x2D5BB0, 0x5A9AE0, 0xC8ECFF]),
                    (Rock, [0x1E2238, 0x3E4868, 0x687898, 0x9AAAC8]),
                    (Forest, [0x0F2028, 0x1E3E48, 0x2E5E62, 0x8AB0B8]),
                    (Bridge, [0x3A2414, 0x7A4E2A, 0xA8743F, 0xCC9A5C]),
                    (Cobble, [0x3A3640, 0x6E6878, 0x9A94A2, 0xC8C2CC]),
                    (Pit, [0x050408, 0x14101C, 0x241C30, 0x302840]),
                ],
            ),
            "marsh" => TerrainTheme {
                outline: Rgba::hex(0x10140C),
                foam: Rgba::hex(0x9AB89A),
                flower_colors: ramp(&[0xC8A0E8, 0xE8E8C0, 0x8AE0C0]),
                trunk: Rgba::hex(0x3A2A1A),
                ..Self::base(
                    "marsh",
                    &[
                        (Grass, [0x1A2E1E, 0x2E4A2C, 0x46663A, 0x6E8A4A]),
                        (Flowers, [0x1A2E1E, 0x2E4A2C, 0x46663A, 0x6E8A4A]),
                        (Path, [0x3A2E1E, 0x5A4A30, 0x786444, 0x9A845A]),
                        (Sand, [0x4A4630, 0x6E6A48, 0x8C8860, 0xAAA67A]),
                        (Water, [0x14261E, 0x234A3A, 0x2E6450, 0x6AA08A]),
                        (Rock, [0x1E1E1A, 0x3E3C32, 0x5E5A4A, 0x86826A]),
                        (Forest, [0x0A1810, 0x16301E, 0x22482A, 0x3E6A3A]),
                        (Bridge, [0x2A1C10, 0x5A3E24, 0x7A5634, 0x9A7448]),
                        (Bog, [0x1E1A10, 0x342C1A, 0x463C24, 0x7A7A4A]),
                        (Cobble, [0x2E2E2A, 0x4E4E46, 0x6A6A60, 0x8A8A7E]),
                        (Pit, [0x050408, 0x14101C, 0x241C30, 0x302840]),
                    ],
                )
            },
            "temple" => TerrainTheme {
                outline: Rgba::hex(0x061418),
                foam: Rgba::hex(0xBCF0F0),
                trunk: Rgba::hex(0x1A3A40),
                ..Self::base(
                    "temple",
                    &[
                        (Floor, [0x14363A, 0x22545A, 0x2E6E72, 0x4A9494]),
                        (Cobble, [0x183E44, 0x2C6268, 0x3E8086, 0x66AAAA]),
                        (Rock, [0x0C2228, 0x1C4048, 0x2E5E66, 0x4C8890]),
                        (Water, [0x0A2850, 0x14488A, 0x1E66B8, 0x7AC8F0]),
                        (Grass, [0x0E3028, 0x1A4C3A, 0x2A6A4A, 0x4A8E5E]),
                        (Sand, [0x4A5A50, 0x6E8270, 0x8EA08A, 0xB0C0A6]),
                        (Bridge, [0x2A2414, 0x5A4A2A, 0x7E6A3E, 0xA28C58]),
                        (Pit, [0x020810, 0x061420, 0x0C2030, 0x142C40]),
                        (Ice, [0x4A7AB0, 0x8AC0E8, 0xBCE4F8, 0xF0FCFF]),
                    ],
                )
            },
            "tower" => TerrainTheme {
                outline: Rgba::hex(0x0C0814),
                foam: Rgba::hex(0xD8D0FF),
                ..Self::base(
                    "tower",
                    &[
                        (Floor, [0x24203A, 0x3A3456, 0x504A72, 0x6E6890]),
                        (Cobble, [0x2C2440, 0x463C62, 0x5E5482, 0x8478A8]),
                        (Rock, [0x14101E, 0x2A2440, 0x443C62, 0x665C8A]),
                        (Water, [0x14183A, 0x242C6A, 0x34449A, 0x7A8AD8]),
                        (Pit, [0x08061A, 0x161230, 0x221C44, 0x30285A]),
                        (Bridge, [0x2A2414, 0x5A4A2A, 0x7E6A3E, 0xA28C58]),
                        (Grass, [0x1E2A2A, 0x2E403C, 0x40584E, 0x5A7462]),
                        (Snow, [0x5A6A8A, 0xA8B8D8, 0xDCE6F5, 0xFFFFFF]),
                    ],
                )
            },
            "citadel" => TerrainTheme {
                outline: Rgba::hex(0x1A1830),
                foam: Rgba::hex(0xFFFFFF),
                ..Self::base(
                    "citadel",
                    &[
                        (Floor, [0x8A84A0, 0xB4AEC8, 0xD4D0E4, 0xF2F0FA]),
                        (Cobble, [0x9A8A5A, 0xC8B470, 0xE6D28A, 0xFFF0B4]),
                        (Rock, [0x4A4466, 0x6E6890, 0x9A94B8, 0xC8C4E0]),
                        (Water, [0x2A3A8A, 0x4A6AC8, 0x6A9AF0, 0xC8E4FF]),
                        // Pits in the citadel open onto the sky far below.
                        (Pit, [0xA8C8F0, 0x8AB0E8, 0x6A94D8, 0x5A84C8]),
                        (Grass, [0x2E5A4A, 0x4A7A5A, 0x6A9A6A, 0x9ACA8A]),
                        (Bridge, [0x6A5A3A, 0x9A845A, 0xC0A874, 0xE0CC98]),
                        (Sand, [0x9A8A6A, 0xC0B08A, 0xD8CAA4, 0xEEE2C4]),
                    ],
                )
            },
            _ => Self::base(
                "field",
                &[
                    (Grass, [0x1F4A2C, 0x3A7A3A, 0x5AA845, 0x8ED15A]),
                    (Flowers, [0x1F4A2C, 0x3A7A3A, 0x5AA845, 0x8ED15A]),
                    (Path, [0x5A3A22, 0x8A5E36, 0xB98A55, 0xD9AE74]),
                    (Sand, [0x8A6A3A, 0xC9A564, 0xE6CB8A, 0xF6E5B4]),
                    (Water, [0x1B2F6B, 0x2D5BB0, 0x3F86D8, 0x9FD3F5]),
                    (Rock, [0x3A2A24, 0x6A4E3A, 0x9A7656, 0xC8A27A]),
                    (Forest, [0x0F2A1E, 0x1E5130, 0x2F7A3C, 0x57A84A]),
                    (Bridge, [0x3A2414, 0x7A4E2A, 0xA8743F, 0xCC9A5C]),
                    (Cobble, [0x6A5A50, 0x9C8C7C, 0xB4A48E, 0xCCBEA6]),
                    (Floor, [0x5A3A22, 0x8A5E36, 0xB98A55, 0xD9AE74]),
                    (Pit, [0x050408, 0x14101C, 0x241C30, 0x302840]),
                    (Snow, [0x5A6A8A, 0xA8B8D8, 0xDCE6F5, 0xFFFFFF]),
                    (Bog, [0x2A2414, 0x4A4022, 0x5E5430, 0x8A8A5A]),
                    (Ice, [0x4A7AB0, 0x8AC0E8, 0xBCE4F8, 0xF0FCFF]),
                ],
            ),
        }
    }
}
