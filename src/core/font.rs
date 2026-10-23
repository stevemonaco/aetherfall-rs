//! A hand-drawn variable-width bitmap font. Glyph rows are separated by '|'; rows 0-6 sit above the baseline and rows
//! 7-8 hold descenders.

use std::collections::HashMap;
use std::sync::LazyLock;

use bevy::math::Vec2;

use super::canvas::Canvas;
use super::color::Rgba;
use super::geom::IRect;
use super::gfx::{self, Batch, TexId};

pub const LINE_HEIGHT: i32 = 11;
pub const CURSOR: char = '\u{1}';
pub const HEART: char = '\u{2}';
const CELL_WIDTH: i32 = 6;
const CELL_HEIGHT: i32 = 9;

const GLYPHS: &[(char, &str)] = &[
    ('A', ".###.|#...#|#...#|#####|#...#|#...#|#...#"),
    ('B', "####.|#...#|#...#|####.|#...#|#...#|####."),
    ('C', ".###.|#...#|#....|#....|#....|#...#|.###."),
    ('D', "####.|#...#|#...#|#...#|#...#|#...#|####."),
    ('E', "#####|#....|#....|####.|#....|#....|#####"),
    ('F', "#####|#....|#....|####.|#....|#....|#...."),
    ('G', ".###.|#...#|#....|#.###|#...#|#...#|.####"),
    ('H', "#...#|#...#|#...#|#####|#...#|#...#|#...#"),
    ('I', "###|.#.|.#.|.#.|.#.|.#.|###"),
    ('J', "..###|...#.|...#.|...#.|#..#.|#..#.|.##.."),
    ('K', "#...#|#..#.|#.#..|##...|#.#..|#..#.|#...#"),
    ('L', "#....|#....|#....|#....|#....|#....|#####"),
    ('M', "#...#|##.##|#.#.#|#.#.#|#...#|#...#|#...#"),
    ('N', "#...#|##..#|#.#.#|#..##|#...#|#...#|#...#"),
    ('O', ".###.|#...#|#...#|#...#|#...#|#...#|.###."),
    ('P', "####.|#...#|#...#|####.|#....|#....|#...."),
    ('Q', ".###.|#...#|#...#|#...#|#.#.#|#..#.|.##.#"),
    ('R', "####.|#...#|#...#|####.|#.#..|#..#.|#...#"),
    ('S', ".####|#....|#....|.###.|....#|....#|####."),
    ('T', "#####|..#..|..#..|..#..|..#..|..#..|..#.."),
    ('U', "#...#|#...#|#...#|#...#|#...#|#...#|.###."),
    ('V', "#...#|#...#|#...#|#...#|#...#|.#.#.|..#.."),
    ('W', "#...#|#...#|#...#|#.#.#|#.#.#|##.##|#...#"),
    ('X', "#...#|#...#|.#.#.|..#..|.#.#.|#...#|#...#"),
    ('Y', "#...#|#...#|.#.#.|..#..|..#..|..#..|..#.."),
    ('Z', "#####|....#|...#.|..#..|.#...|#....|#####"),
    ('a', ".....|.....|.###.|....#|.####|#...#|.####"),
    ('b', "#....|#....|####.|#...#|#...#|#...#|####."),
    ('c', "....|....|.###|#...|#...|#...|.###"),
    ('d', "....#|....#|.####|#...#|#...#|#...#|.####"),
    ('e', ".....|.....|.###.|#...#|#####|#....|.###."),
    ('f', "..##|.#..|####|.#..|.#..|.#..|.#.."),
    ('g', ".....|.....|.####|#...#|#...#|#...#|.####|....#|.###."),
    ('h', "#....|#....|####.|#...#|#...#|#...#|#...#"),
    ('i', ".#.|...|##.|.#.|.#.|.#.|###"),
    ('j', "...#|....|..##|...#|...#|...#|...#|#..#|.##."),
    ('k', "#...|#...|#..#|#.#.|##..|#.#.|#..#"),
    ('l', "##.|.#.|.#.|.#.|.#.|.#.|###"),
    ('m', ".....|.....|##.#.|#.#.#|#.#.#|#.#.#|#.#.#"),
    ('n', ".....|.....|####.|#...#|#...#|#...#|#...#"),
    ('o', ".....|.....|.###.|#...#|#...#|#...#|.###."),
    ('p', ".....|.....|####.|#...#|#...#|#...#|####.|#....|#...."),
    ('q', ".....|.....|.####|#...#|#...#|#...#|.####|....#|....#"),
    ('r', "....|....|#.##|##..|#...|#...|#..."),
    ('s', "....|....|.###|#...|.##.|...#|###."),
    ('t', ".#..|.#..|####|.#..|.#..|.#..|..##"),
    ('u', ".....|.....|#...#|#...#|#...#|#...#|.####"),
    ('v', ".....|.....|#...#|#...#|#...#|.#.#.|..#.."),
    ('w', ".....|.....|#...#|#...#|#.#.#|#.#.#|.#.#."),
    ('x', ".....|.....|#...#|.#.#.|..#..|.#.#.|#...#"),
    ('y', ".....|.....|#...#|#...#|#...#|#...#|.####|....#|.###."),
    ('z', ".....|.....|#####|...#.|..#..|.#...|#####"),
    ('0', ".###.|#...#|#..##|#.#.#|##..#|#...#|.###."),
    ('1', ".#.|##.|.#.|.#.|.#.|.#.|###"),
    ('2', ".###.|#...#|....#|...#.|..#..|.#...|#####"),
    ('3', "#####|...#.|..#..|...#.|....#|#...#|.###."),
    ('4', "...#.|..##.|.#.#.|#..#.|#####|...#.|...#."),
    ('5', "#####|#....|####.|....#|....#|#...#|.###."),
    ('6', "..##.|.#...|#....|####.|#...#|#...#|.###."),
    ('7', "#####|....#|...#.|..#..|.#...|.#...|.#..."),
    ('8', ".###.|#...#|#...#|.###.|#...#|#...#|.###."),
    ('9', ".###.|#...#|#...#|.####|....#|...#.|.##.."),
    (' ', "..."),
    ('!', "#|#|#|#|#|.|#"),
    ('"', "#.#|#.#"),
    ('#', ".#.#.|.#.#.|#####|.#.#.|#####|.#.#.|.#.#."),
    ('$', "..#..|.####|#.#..|.###.|..#.#|####.|..#.."),
    ('%', "##..#|##.#.|...#.|..#..|.#...|.#.##|#..##"),
    ('&', ".##..|#..#.|#.#..|.#...|#.#.#|#..#.|.##.#"),
    ('\'', "#|#"),
    ('(', ".#|#.|#.|#.|#.|#.|.#"),
    (')', "#.|.#|.#|.#|.#|.#|#."),
    ('*', ".....|..#..|#.#.#|.###.|#.#.#|..#.."),
    ('+', ".....|..#..|..#..|#####|..#..|..#.."),
    (',', "..|..|..|..|..|.#|.#|#."),
    ('-', "....|....|....|####"),
    ('.', ".|.|.|.|.|.|#"),
    ('/', "....#|....#|...#.|..#..|.#...|#....|#...."),
    (':', ".|.|#|.|.|#"),
    (';', "..|..|.#|..|..|.#|.#|#."),
    ('<', "...#|..#.|.#..|#...|.#..|..#.|...#"),
    ('=', "....|....|####|....|####"),
    ('>', "#...|.#..|..#.|...#|..#.|.#..|#..."),
    ('?', ".###.|#...#|....#|...#.|..#..|.....|..#.."),
    ('@', ".###.|#...#|#.###|#.#.#|#.##.|#....|.###."),
    ('[', "##|#.|#.|#.|#.|#.|##"),
    ('\\', "#....|#....|.#...|..#..|...#.|....#|....#"),
    (']', "##|.#|.#|.#|.#|.#|##"),
    ('^', "..#..|.#.#.|#...#"),
    ('_', ".....|.....|.....|.....|.....|.....|.....|#####"),
    ('`', "#.|.#"),
    ('{', "..#|.#.|.#.|#..|.#.|.#.|..#"),
    ('|', "#|#|#|#|#|#|#"),
    ('}', "#..|.#.|.#.|..#|.#.|.#.|#.."),
    ('~', ".....|.....|.#...|#.#.#|...#."),
    (CURSOR, "#...|##..|###.|####|###.|##..|#..."),
    (HEART, ".....|##.##|#####|#####|.###.|..#.."),
];

struct Font {
    texture: TexId,
    layout: HashMap<char, (IRect, i32)>,
}

static FONT: LazyLock<Font> = LazyLock::new(|| {
    const COLUMNS: i32 = 16;
    let rows = (GLYPHS.len() as i32 + COLUMNS - 1) / COLUMNS;
    let mut canvas = Canvas::new(COLUMNS * CELL_WIDTH, rows * CELL_HEIGHT);
    let mut layout = HashMap::new();
    for (i, &(ch, glyph)) in GLYPHS.iter().enumerate() {
        let (ox, oy) = (i as i32 % COLUMNS * CELL_WIDTH, i as i32 / COLUMNS * CELL_HEIGHT);
        let rows: Vec<&str> = glyph.split('|').collect();
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '#' {
                    canvas.set(ox + x as i32, oy + y as i32, Rgba::WHITE);
                }
            }
        }
        layout.insert(ch, (IRect::new(ox, oy, width, CELL_HEIGHT), width));
    }
    Font { texture: gfx::from_canvas(canvas), layout }
});

fn advance(c: char) -> i32 {
    FONT.layout.get(&c).map(|g| g.1 + 1).unwrap_or(4)
}

pub fn measure(text: &str) -> i32 {
    let (mut width, mut best) = (0, 0);
    for c in text.chars() {
        if c == '\n' {
            best = best.max(width);
            width = 0;
            continue;
        }
        width += advance(c);
    }
    best.max(width) - if text.is_empty() { 0 } else { 1 }
}

pub fn draw(batch: &mut Batch, text: &str, pos: Vec2, color: Rgba, shadow: Option<Rgba>) {
    if let Some(s) = shadow {
        draw_raw(batch, text, pos + Vec2::ONE, s);
    }
    draw_raw(batch, text, pos, color);
}

pub fn draw_scaled(batch: &mut Batch, text: &str, pos: Vec2, color: Rgba, scale: i32) {
    let mut pen = pos.round();
    for c in text.chars() {
        if let Some(&(src, _)) = FONT.layout.get(&c) {
            batch.draw_ex(FONT.texture, pen, Some(src), color, 0.0, Vec2::ZERO, Vec2::splat(scale as f32), false, false);
        }
        pen.x += (advance(c) * scale) as f32;
    }
}

pub fn draw_centered(batch: &mut Batch, text: &str, center: Vec2, color: Rgba, shadow: Option<Rgba>) {
    draw(batch, text, Vec2::new((center.x - measure(text) as f32 / 2.0).round(), center.y), color, shadow);
}

/// Splits text into lines that fit within `max_width` pixels.
pub fn wrap(text: &str, max_width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if measure(&candidate) > max_width && !line.is_empty() {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

fn draw_raw(batch: &mut Batch, text: &str, pos: Vec2, color: Rgba) {
    // Half-pixel positions lose a glyph row to sampling in the low-resolution target.
    let pos = pos.round();
    let mut pen = pos;
    for c in text.chars() {
        if c == '\n' {
            pen = Vec2::new(pos.x, pen.y + LINE_HEIGHT as f32);
            continue;
        }
        if let Some(&(src, _)) = FONT.layout.get(&c) {
            batch.draw_src(FONT.texture, pen, Some(src), color);
        }
        pen.x += advance(c) as f32;
    }
}
