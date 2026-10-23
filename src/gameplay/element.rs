use crate::core::color::Rgba;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum Element {
    #[default]
    None,
    Wind,
    Fire,
    Water,
    Thunder,
    Aether,
}

pub const MODES: [Element; 6] = [Element::None, Element::Wind, Element::Fire, Element::Water, Element::Thunder, Element::Aether];
pub const BASIC: [Element; 4] = [Element::Wind, Element::Fire, Element::Water, Element::Thunder];

impl Element {
    pub fn opposite(self) -> Element {
        match self {
            Element::Fire => Element::Water,
            Element::Water => Element::Fire,
            Element::Wind => Element::Thunder,
            Element::Thunder => Element::Wind,
            _ => Element::None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Element::None => "none",
            Element::Wind => "wind",
            Element::Fire => "fire",
            Element::Water => "water",
            Element::Thunder => "thunder",
            Element::Aether => "aether",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Element::None => "None",
            Element::Wind => "Wind",
            Element::Fire => "Fire",
            Element::Water => "Water",
            Element::Thunder => "Thunder",
            Element::Aether => "Aether",
        }
    }

    pub fn label(self) -> &'static str {
        if self == Element::None { "Neutral" } else { self.name() }
    }

    pub fn parse(s: Option<&str>) -> Element {
        match s.map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("wind") => Element::Wind,
            Some("fire") => Element::Fire,
            Some("water") => Element::Water,
            Some("thunder") => Element::Thunder,
            Some("aether") => Element::Aether,
            _ => Element::None,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn glow(self) -> Rgba {
        match self {
            Element::Wind => Rgba::rgb(120, 236, 200),
            Element::Fire => Rgba::rgb(255, 128, 48),
            Element::Water => Rgba::rgb(96, 172, 255),
            Element::Thunder => Rgba::rgb(252, 228, 84),
            Element::Aether => Rgba::rgb(244, 150, 255),
            Element::None => Rgba::rgb(232, 232, 244),
        }
    }

    pub fn dark(self) -> Rgba {
        match self {
            Element::Wind => Rgba::rgb(30, 110, 100),
            Element::Fire => Rgba::rgb(150, 40, 20),
            Element::Water => Rgba::rgb(30, 60, 140),
            Element::Thunder => Rgba::rgb(140, 110, 20),
            Element::Aether => Rgba::rgb(110, 40, 140),
            Element::None => Rgba::rgb(80, 80, 104),
        }
    }

    pub fn light(self) -> Rgba {
        self.glow().lerp(Rgba::WHITE, 0.55)
    }
}
