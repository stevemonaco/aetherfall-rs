//! A small CPU-side pixel buffer for drawing procedural sprites before uploading them as textures.

use super::color::Rgba;
use super::palette;

#[derive(Clone)]
pub struct Canvas {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<Rgba>,
}

impl Canvas {
    pub fn new(width: i32, height: i32) -> Canvas {
        Canvas { width, height, pixels: vec![Rgba::TRANSPARENT; (width * height).max(0) as usize] }
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize]
        } else {
            Rgba::TRANSPARENT
        }
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = c;
        }
    }

    pub fn is_set(&self, x: i32, y: i32) -> bool {
        self.get(x, y).a > 0
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) -> &mut Self {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
        self
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) -> &mut Self {
        let mut y = (cy - ry - 1.0) as i32;
        while y as f32 <= cy + ry + 1.0 {
            let mut x = (cx - rx - 1.0) as i32;
            while x as f32 <= cx + rx + 1.0 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    self.set(x, y, c);
                }
                x += 1;
            }
            y += 1;
        }
        self
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) -> &mut Self {
        self.ellipse(cx, cy, r, r, c)
    }

    pub fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: Rgba) -> &mut Self {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            self.set(x0, y0, c);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        self
    }

    /// Recolours every set pixel using a function of its position and current colour.
    pub fn shade(&mut self, shader: impl Fn(i32, i32, Rgba) -> Rgba) -> &mut Self {
        for y in 0..self.height {
            for x in 0..self.width {
                let i = (y * self.width + x) as usize;
                if self.pixels[i].a > 0 {
                    self.pixels[i] = shader(x, y, self.pixels[i]);
                }
            }
        }
        self
    }

    /// Adds a 1px outline around all opaque pixels, matching the dark outlines on the 7Soul sprites.
    pub fn outline(&mut self) -> &mut Self {
        self.outline_with(palette::OUTLINE)
    }

    pub fn outline_with(&mut self, c: Rgba) -> &mut Self {
        let copy = self.pixels.clone();
        let (w, h) = (self.width, self.height);
        let opaque = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && copy[(y * w + x) as usize].a > 0;
        for y in 0..h {
            for x in 0..w {
                if !opaque(x, y) && (opaque(x - 1, y) || opaque(x + 1, y) || opaque(x, y - 1) || opaque(x, y + 1)) {
                    self.pixels[(y * w + x) as usize] = c;
                }
            }
        }
        self
    }

    pub fn blit(&mut self, source: &Canvas, ox: i32, oy: i32) -> &mut Self {
        for y in 0..source.height {
            for x in 0..source.width {
                let c = source.get(x, y);
                if c.a > 0 {
                    self.set(ox + x, oy + y, c);
                }
            }
        }
        self
    }

    /// Draws a pattern of characters mapped to colours; rows are separated by '|' and unmapped characters are skipped.
    pub fn pattern(&mut self, ox: i32, oy: i32, pattern: &str, colors: &[(char, Rgba)]) -> &mut Self {
        for (y, row) in pattern.split('|').enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if let Some(&(_, c)) = colors.iter().find(|(k, _)| *k == ch) {
                    self.set(ox + x as i32, oy + y as i32, c);
                }
            }
        }
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect()
    }

    /// Moves the builder result out so `Canvas::new(..).rect(..).outline().take()` chains read naturally.
    pub fn take(&mut self) -> Canvas {
        std::mem::replace(self, Canvas::new(0, 0))
    }
}
