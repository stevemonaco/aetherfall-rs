/// An 8-bit straight-alpha colour. MonoGame's premultiplied `color * alpha` maps to [`Rgba::mul`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
    pub const WHITE: Rgba = Rgba::rgb(255, 255, 255);
    pub const BLACK: Rgba = Rgba::rgb(0, 0, 0);
    pub const GRAY: Rgba = Rgba::rgb(128, 128, 128);
    pub const RED: Rgba = Rgba::rgb(255, 0, 0);
    pub const YELLOW: Rgba = Rgba::rgb(255, 255, 0);

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

    /// Fades the colour, the straight-alpha equivalent of multiplying a premultiplied colour.
    pub fn mul(self, alpha: f32) -> Rgba {
        Rgba { a: (self.a as f32 * alpha.clamp(0.0, 1.0)) as u8, ..self }
    }

    pub fn with_alpha(self, alpha: f32) -> Rgba {
        Rgba { a: (255.0 * alpha.clamp(0.0, 1.0)) as u8, ..self }
    }

    pub fn packed(self) -> u32 {
        u32::from_le_bytes([self.r, self.g, self.b, self.a])
    }
}

pub fn ramp(hex: &[u32]) -> Vec<Rgba> {
    hex.iter().map(|&h| Rgba::hex(h)).collect()
}

/// Shorthand used throughout the art code.
pub const fn c(hex: u32) -> Rgba {
    Rgba::hex(hex)
}
