//! Textures and immediate-mode drawing. Game code draws through a [`Batch`] exactly as the C# game drew through
//! MonoGame's SpriteBatch; the platform layer turns the command list into GPU sprites each frame.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use bevy::math::Vec2;

use super::canvas::Canvas;
use super::color::Rgba;
use super::geom::IRect;
use super::palette;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TexId(pub u32);

struct Texture {
    width: i32,
    height: i32,
    pixels: Option<Arc<Vec<Rgba>>>,
}

#[derive(Default)]
struct Store {
    textures: Vec<Texture>,
    by_key: HashMap<String, TexId>,
    silhouettes: HashMap<TexId, TexId>,
    uploads: Vec<(TexId, Canvas)>,
}

static STORE: LazyLock<Mutex<Store>> = LazyLock::new(|| Mutex::new(Store::default()));

fn store() -> std::sync::MutexGuard<'static, Store> {
    STORE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Registers a canvas as a new texture and queues it for upload.
pub fn from_canvas(canvas: Canvas) -> TexId {
    let mut s = store();
    let id = TexId(s.textures.len() as u32);
    s.textures.push(Texture {
        width: canvas.width,
        height: canvas.height,
        pixels: Some(Arc::new(canvas.pixels.clone())),
    });
    s.uploads.push((id, canvas));
    id
}

/// Returns the texture built for `key`, building it on first use.
pub fn cached(key: &str, build: impl FnOnce() -> Canvas) -> TexId {
    if let Some(&id) = store().by_key.get(key) {
        return id;
    }
    let id = from_canvas(build());
    store().by_key.insert(key.to_string(), id);
    id
}

/// Registers a texture the platform layer loaded from disk (licensed sprites), keyed by its asset path.
pub fn register_loaded(path: &str, width: i32, height: i32, pixels: Vec<Rgba>) -> TexId {
    let mut s = store();
    let id = TexId(s.textures.len() as u32);
    s.textures.push(Texture { width, height, pixels: Some(Arc::new(pixels)) });
    s.by_key.insert(path.to_lowercase(), id);
    id
}

/// A texture loaded from `assets/<path>`, or a magenta checkerboard when it is missing.
pub fn external(path: &str) -> TexId {
    if let Some(&id) = store().by_key.get(&path.to_lowercase()) {
        return id;
    }
    cached("placeholder", || {
        let mut c = Canvas::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                let on = ((x / 4 + y / 4) & 1) == 0;
                c.set(x, y, if on { Rgba::rgb(255, 0, 255) } else { Rgba::rgb(40, 0, 40) });
            }
        }
        c
    })
}

pub fn exists(path: &str) -> bool {
    store().by_key.contains_key(&path.to_lowercase())
}

pub fn size(id: TexId) -> (i32, i32) {
    let s = store();
    s.textures.get(id.0 as usize).map(|t| (t.width, t.height)).unwrap_or((1, 1))
}

pub fn pixels(id: TexId) -> Option<Arc<Vec<Rgba>>> {
    store().textures.get(id.0 as usize).and_then(|t| t.pixels.clone())
}

/// A cached white silhouette of a texture, used for hit flashes without a shader.
pub fn silhouette(id: TexId) -> TexId {
    if let Some(&sil) = store().silhouettes.get(&id) {
        return sil;
    }
    let (w, h) = size(id);
    let mut canvas = Canvas::new(w, h);
    if let Some(src) = pixels(id) {
        for (i, p) in src.iter().enumerate() {
            if p.a > 0 {
                canvas.pixels[i] = Rgba::WHITE;
            }
        }
    }
    let sil = from_canvas(canvas);
    store().silhouettes.insert(id, sil);
    sil
}

pub fn pixel() -> TexId {
    cached("pixel", || {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, Rgba::WHITE);
        c
    })
}

/// Textures created since the last call, for the platform layer to upload.
pub fn take_uploads() -> Vec<(TexId, Canvas)> {
    std::mem::take(&mut store().uploads)
}

pub fn texture_count() -> usize {
    store().textures.len()
}

#[derive(Clone, Copy, Debug)]
pub struct DrawCmd {
    pub tex: TexId,
    pub src: IRect,
    pub pos: Vec2,
    pub origin: Vec2,
    pub scale: Vec2,
    pub rotation: f32,
    pub color: Rgba,
    pub flip_x: bool,
    pub flip_y: bool,
}

/// A frame's worth of draw commands in painter's order, in virtual-screen pixels.
#[derive(Default)]
pub struct Batch {
    pub cmds: Vec<DrawCmd>,
    offset: Vec2,
    light_mark: Option<usize>,
}

impl Batch {
    /// Starts a group of draws translated by `offset` (the negated camera position for world drawing).
    pub fn begin(&mut self, offset: Vec2) {
        self.offset = offset;
    }

    pub fn begin_screen(&mut self) {
        self.offset = Vec2::ZERO;
    }

    pub fn clear(&mut self) {
        self.cmds.clear();
        self.offset = Vec2::ZERO;
        self.light_mark = None;
    }

    /// Marks the end of world drawing: the light map is composited here, beneath everything drawn after.
    pub fn mark_light(&mut self) {
        self.light_mark = Some(self.cmds.len());
    }

    pub fn light_mark(&self) -> Option<usize> {
        self.light_mark
    }

    pub fn draw(&mut self, tex: TexId, pos: Vec2, color: Rgba) {
        self.draw_ex(tex, pos, None, color, 0.0, Vec2::ZERO, Vec2::ONE, false, false);
    }

    pub fn draw_src(&mut self, tex: TexId, pos: Vec2, src: Option<IRect>, color: Rgba) {
        self.draw_ex(tex, pos, src, color, 0.0, Vec2::ZERO, Vec2::ONE, false, false);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_ex(
        &mut self,
        tex: TexId,
        pos: Vec2,
        src: Option<IRect>,
        color: Rgba,
        rotation: f32,
        origin: Vec2,
        scale: Vec2,
        flip_x: bool,
        flip_y: bool,
    ) {
        if color.a == 0 {
            return;
        }
        let src = src.unwrap_or_else(|| {
            let (w, h) = size(tex);
            IRect::new(0, 0, w, h)
        });
        self.cmds.push(DrawCmd { tex, src, pos: pos + self.offset, origin, scale, rotation, color, flip_x, flip_y });
    }

    /// Draws a texture stretched over a destination rectangle.
    pub fn draw_dest(&mut self, tex: TexId, dest: IRect, src: Option<IRect>, color: Rgba) {
        let src = src.unwrap_or_else(|| {
            let (w, h) = size(tex);
            IRect::new(0, 0, w, h)
        });
        if src.w == 0 || src.h == 0 {
            return;
        }
        let scale = Vec2::new(dest.w as f32 / src.w as f32, dest.h as f32 / src.h as f32);
        self.draw_ex(
            tex,
            Vec2::new(dest.x as f32, dest.y as f32),
            Some(src),
            color,
            0.0,
            Vec2::ZERO,
            scale,
            false,
            false,
        );
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        self.draw_ex(pixel(), Vec2::new(x, y), None, color, 0.0, Vec2::ZERO, Vec2::new(w, h), false, false);
    }

    pub fn fill(&mut self, r: IRect, color: Rgba) {
        self.fill_rect(r.x as f32, r.y as f32, r.w as f32, r.h as f32, color);
    }

    pub fn draw_rect(&mut self, r: IRect, color: Rgba) {
        self.fill(IRect::new(r.x, r.y, r.w, 1), color);
        self.fill(IRect::new(r.x, r.bottom() - 1, r.w, 1), color);
        self.fill(IRect::new(r.x, r.y, 1, r.h), color);
        self.fill(IRect::new(r.right() - 1, r.y, 1, r.h), color);
    }

    /// Draws the game's standard bevelled menu window.
    pub fn draw_window(&mut self, r: IRect) {
        self.fill(IRect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), palette::WINDOW_FILL);
        self.fill(IRect::new(r.x + 2, r.y + 2, r.w - 4, 1), palette::WINDOW_FILL_LIGHT);
        self.draw_rect(IRect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), palette::WINDOW_BORDER);
        // The outer outline skips the corner pixels so the window reads as rounded.
        self.fill(IRect::new(r.x + 1, r.y, r.w - 2, 1), palette::OUTLINE);
        self.fill(IRect::new(r.x + 1, r.bottom() - 1, r.w - 2, 1), palette::OUTLINE);
        self.fill(IRect::new(r.x, r.y + 1, 1, r.h - 2), palette::OUTLINE);
        self.fill(IRect::new(r.right() - 1, r.y + 1, 1, r.h - 2), palette::OUTLINE);
    }
}

/// A strip of equally sized frames in one texture (FrameStrip in the C# game).
#[derive(Clone, Copy, Debug)]
pub struct FrameStrip {
    pub texture: TexId,
    pub frame_width: i32,
    pub frame_height: i32,
    pub frame_count: i32,
}

impl FrameStrip {
    pub fn new(texture: TexId, frame_width: i32, frame_height: i32) -> FrameStrip {
        let (w, _) = size(texture);
        FrameStrip { texture, frame_width, frame_height, frame_count: (w / frame_width).max(1) }
    }

    pub fn silhouette(&self) -> TexId {
        silhouette(self.texture)
    }

    pub fn source(&self, frame: i32) -> IRect {
        IRect::new(frame.rem_euclid(self.frame_count) * self.frame_width, 0, self.frame_width, self.frame_height)
    }

    /// Draws a frame anchored at its bottom-centre ("feet") position.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, batch: &mut Batch, feet: Vec2, frame: i32, color: Rgba, flip_x: bool, flash: bool, scale: f32) {
        let origin = Vec2::new(self.frame_width as f32 / 2.0, self.frame_height as f32);
        let tex = if flash { self.silhouette() } else { self.texture };
        batch.draw_ex(
            tex,
            super::geom::snap(feet),
            Some(self.source(frame)),
            color,
            0.0,
            origin,
            Vec2::splat(scale),
            flip_x,
            false,
        );
    }
}
