/// An 8-bit straight-alpha colour, the pixel type for all procedural art.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
    pub const WHITE: Rgba = Rgba::new(255, 255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    pub const fn hex(rgb: u32) -> Self {
        Self::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }

    /// Truncating lerp, as MonoGame's Color.Lerp does, so baked art matches the C# game.
    pub fn lerp(self, to: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        Rgba::new(mix(self.r, to.r), mix(self.g, to.g), mix(self.b, to.b), mix(self.a, to.a))
    }

    pub fn with_alpha(self, alpha: f32) -> Rgba {
        Rgba { a: (255.0 * alpha) as u8, ..self }
    }
}

pub fn ramp(hex: &[u32]) -> Vec<Rgba> {
    hex.iter().map(|&h| Rgba::hex(h)).collect()
}

pub const OUTLINE: Rgba = Rgba::rgb(20, 16, 28);
