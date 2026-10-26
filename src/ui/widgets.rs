//! Small drawing pieces shared by the menus: framed icon slots, meter bars, section headings and rules, so every
//! screen dresses its text the same way.

use bevy::math::Vec2;

use crate::core::color::Rgba;
use crate::core::font;
use crate::core::geom::IRect;
use crate::core::gfx::{Batch, TexId};
use crate::core::palette;

/// The side of a framed icon slot: a 16px icon plus a one-pixel border and padding.
pub const SLOT: i32 = 18;

/// A framed square for an icon. `accent` tints the border (an element, a rarity); dim slots are locked or empty.
pub fn slot(batch: &mut Batch, at: Vec2, accent: Option<Rgba>, dim: bool) -> IRect {
    let frame = IRect::new(at.x as i32, at.y as i32, SLOT, SLOT);
    let fill = if dim { Rgba::BLACK.mul(0.35) } else { palette::WINDOW_FILL_LIGHT.mul(0.4) };
    let border = match accent {
        Some(c) if !dim => c.mul(0.8),
        _ => palette::WINDOW_BORDER.mul(if dim { 0.3 } else { 0.6 }),
    };
    batch.fill(frame, fill);
    batch.draw_rect(frame, border);
    frame
}

/// A slot holding an icon, scaled down if the texture is larger than 16px.
pub fn icon_slot(batch: &mut Batch, at: Vec2, icon: Option<TexId>, accent: Option<Rgba>, dim: bool) {
    slot(batch, at, accent, dim);
    if let Some(icon) = icon {
        let (w, h) = crate::core::gfx::size(icon);
        let scale = (16.0 / w.max(h) as f32).min(1.0);
        let tint = if dim { Rgba::WHITE.mul(0.4) } else { Rgba::WHITE };
        let offset = Vec2::new((SLOT as f32 - w as f32 * scale) / 2.0, (SLOT as f32 - h as f32 * scale) / 2.0);
        batch.draw_ex(icon, at + offset, None, tint, 0.0, Vec2::ZERO, Vec2::splat(scale), false, false);
    }
}

/// A horizontal meter filled to `progress` (0 to 1).
pub fn bar(batch: &mut Batch, rect: IRect, progress: f32, color: Rgba) {
    batch.fill(rect, palette::WINDOW_FILL_LIGHT.mul(0.6));
    let filled = (rect.w as f32 * progress.clamp(0.0, 1.0)).round() as i32;
    batch.fill(IRect::new(rect.x, rect.y, filled, rect.h), color);
    batch.draw_rect(IRect::new(rect.x - 1, rect.y - 1, rect.w + 2, rect.h + 2), palette::WINDOW_BORDER.mul(0.6));
}

/// A faint divider line.
pub fn rule(batch: &mut Batch, x: i32, y: i32, w: i32) {
    batch.fill(IRect::new(x, y, w, 1), palette::WINDOW_BORDER.mul(0.4));
}

/// A dim heading with a rule running from its end to `right`.
pub fn section(batch: &mut Batch, text: &str, at: Vec2, right: i32) {
    font::draw(batch, text, at, palette::TEXT_DIM, Some(palette::TEXT_SHADOW));
    let start = at.x as i32 + font::measure(text) + 4;
    if right > start {
        rule(batch, start, at.y as i32 + 4, right - start);
    }
}

/// Text drawn right-aligned so it ends at `right`.
pub fn right_text(batch: &mut Batch, text: &str, right: f32, y: f32, color: Rgba) {
    font::draw(batch, text, Vec2::new(right - font::measure(text) as f32, y), color, None);
}
