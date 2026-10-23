//! Procedural sprites: a CPU pixel canvas and the person rig shared by the hero and villagers (ported from
//! Canvas.cs and HeroArt.cs).

use crate::color::{OUTLINE, Rgba, ramp};

/// A small CPU-side pixel buffer for drawing procedural sprites before uploading them as images.
pub struct Canvas {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<Rgba>,
}

impl Canvas {
    pub fn new(width: i32, height: i32) -> Canvas {
        Canvas { width, height, pixels: vec![Rgba::TRANSPARENT; (width * height) as usize] }
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

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) {
        for y in (cy - ry - 1.0) as i32..=(cy + ry + 1.0) as i32 {
            for x in (cx - rx - 1.0) as i32..=(cx + rx + 1.0) as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// Adds a 1px outline around all opaque pixels, matching the dark outlines on the 7Soul sprites.
    pub fn outline(&mut self, c: Rgba) {
        let copy = self.pixels.clone();
        let opaque = |x: i32, y: i32| {
            x >= 0 && y >= 0 && x < self.width && y < self.height && copy[(y * self.width + x) as usize].a > 0
        };
        let mut edges = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                if !opaque(x, y) && (opaque(x - 1, y) || opaque(x + 1, y) || opaque(x, y - 1) || opaque(x, y + 1)) {
                    edges.push((x, y));
                }
            }
        }
        for (x, y) in edges {
            self.set(x, y, c);
        }
    }

    pub fn blit(&mut self, source: &Canvas, ox: i32, oy: i32) {
        for y in 0..source.height {
            for x in 0..source.width {
                let c = source.get(x, y);
                if c.a > 0 {
                    self.set(ox + x, oy + y, c);
                }
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeroPose {
    Idle,
    Run,
    Slash,
    Throw,
    Guard,
    Dodge,
    Hurt,
    Jump,
}

const POSES: [HeroPose; 8] = [
    HeroPose::Idle,
    HeroPose::Run,
    HeroPose::Slash,
    HeroPose::Throw,
    HeroPose::Guard,
    HeroPose::Dodge,
    HeroPose::Hurt,
    HeroPose::Jump,
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HairStyle {
    Short,
    Long,
    Bald,
    Bun,
    Spiky,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Headwear {
    None,
    Hood,
    WizardHat,
    Helmet,
    Cap,
    Headband,
}

/// What a person looks like: palette ramps (dark to light) and a few style switches.
pub struct Appearance {
    pub skin: Vec<Rgba>,
    pub hair: Vec<Rgba>,
    pub top: Vec<Rgba>,
    pub legs: Vec<Rgba>,
    pub boots: Vec<Rgba>,
    pub hair_style: HairStyle,
    pub headwear: Headwear,
    pub hat: Option<Vec<Rgba>>,
    pub scarf: Option<Vec<Rgba>>,
    pub robe: bool,
    pub beard: bool,
    pub belt: Option<Rgba>,
    pub cape: bool,
}

impl Appearance {
    pub fn hero() -> Appearance {
        Appearance {
            skin: ramp(&[0xB07850, 0xE0A87C, 0xF6D2AC]),
            hair: ramp(&[0x3A2216, 0x6A3E22, 0x9A5E30, 0xC08A48]),
            top: ramp(&[0x22305A, 0x3A5498, 0x5A7ECC, 0x8AAEEC]),
            legs: ramp(&[0x221C2C, 0x3C3248]),
            boots: ramp(&[0x2E1C12, 0x6A4226]),
            hair_style: HairStyle::Short,
            headwear: Headwear::None,
            hat: None,
            scarf: Some(ramp(&[0x16645A, 0x26A08C, 0x66DCC4])),
            robe: false,
            beard: false,
            belt: None,
            cape: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Front,
    Back,
    Side,
}

#[derive(Clone, Copy, Default)]
struct Rig {
    bob: i32,
    lean: i32,
    lift_l: i32,
    lift_r: i32,
    stride_l: i32,
    stride_r: i32,
    swing_l: i32,
    swing_r: i32,
    reach: i32,
    arms_up: bool,
    crouch: bool,
    scarf_wave: i32,
}

/// People drawn from a small rig so the hero and every villager share one look. Frames are 20x26 with the feet at
/// the bottom centre; rows are front, back and side views, and side views face right.
pub struct PersonSprite;

impl PersonSprite {
    pub const FRAME_WIDTH: i32 = 20;
    pub const FRAME_HEIGHT: i32 = 26;
    const FRAME_COUNTS: [i32; 8] = [2, 6, 3, 2, 1, 1, 1, 1];

    pub fn columns() -> i32 {
        Self::FRAME_COUNTS.iter().sum()
    }

    pub fn frame_count(pose: HeroPose) -> i32 {
        Self::FRAME_COUNTS[pose as usize]
    }

    /// The sheet column for a pose's frame; the row is 0 front, 1 back, 2 side.
    pub fn column(pose: HeroPose, frame: i32) -> i32 {
        let start: i32 = Self::FRAME_COUNTS[..pose as usize].iter().sum();
        start + frame.clamp(0, Self::frame_count(pose) - 1)
    }

    pub fn build(look: &Appearance) -> Canvas {
        let mut sheet = Canvas::new(Self::columns() * Self::FRAME_WIDTH, 3 * Self::FRAME_HEIGHT);
        for (v, view) in [View::Front, View::Back, View::Side].into_iter().enumerate() {
            for pose in POSES {
                for f in 0..Self::frame_count(pose) {
                    let mut frame = Canvas::new(Self::FRAME_WIDTH, Self::FRAME_HEIGHT);
                    Painter { c: &mut frame, look }.paint(view, rig_for(pose, f));
                    frame.outline(OUTLINE);
                    sheet.blit(&frame, Self::column(pose, f) * Self::FRAME_WIDTH, v as i32 * Self::FRAME_HEIGHT);
                }
            }
        }
        sheet
    }
}

fn rig_for(pose: HeroPose, f: i32) -> Rig {
    let r = Rig::default();
    match pose {
        HeroPose::Idle => Rig { bob: f, scarf_wave: f, ..r },
        HeroPose::Run => match f {
            0 => Rig { stride_l: 1, stride_r: -1, lift_r: 1, swing_l: -1, swing_r: 1, scarf_wave: 1, ..r },
            1 => Rig { bob: -1, lift_r: 2, swing_l: -1, swing_r: 1, scarf_wave: 2, ..r },
            2 => Rig { stride_l: -1, stride_r: 1, scarf_wave: 1, ..r },
            3 => Rig { stride_l: -1, stride_r: 1, lift_l: 1, swing_l: 1, swing_r: -1, scarf_wave: 1, ..r },
            4 => Rig { bob: -1, lift_l: 2, swing_l: 1, swing_r: -1, scarf_wave: 2, ..r },
            _ => Rig { stride_l: 1, stride_r: -1, scarf_wave: 1, ..r },
        },
        HeroPose::Slash => match f {
            0 => Rig { lean: -1, swing_r: -3, arms_up: true, ..r },
            1 => Rig { lean: 1, reach: 3, stride_l: 1, stride_r: -1, scarf_wave: 2, ..r },
            _ => Rig { lean: 1, reach: 1, swing_r: 1, scarf_wave: 1, ..r },
        },
        HeroPose::Throw if f == 0 => Rig { lean: -1, swing_r: -2, ..r },
        HeroPose::Throw => Rig { lean: 1, reach: 3, scarf_wave: 2, ..r },
        HeroPose::Guard => Rig { bob: 1, reach: 2, stride_l: -1, stride_r: 1, ..r },
        HeroPose::Dodge => Rig {
            bob: 2,
            lean: 2,
            lift_r: 2,
            stride_l: 1,
            stride_r: -2,
            swing_l: 2,
            swing_r: 2,
            crouch: true,
            scarf_wave: 3,
            ..r
        },
        HeroPose::Hurt => Rig { lean: -2, swing_l: -2, swing_r: -2, arms_up: true, scarf_wave: 3, ..r },
        HeroPose::Jump => Rig { bob: -1, lift_l: 2, lift_r: 2, arms_up: true, scarf_wave: 2, ..r },
    }
}

const EYE: Rgba = Rgba::hex(0x1E1830);
const BLUSH: Rgba = Rgba::hex(0xF0A0A0);

struct Painter<'a> {
    c: &'a mut Canvas,
    look: &'a Appearance,
}

impl Painter<'_> {
    fn paint(&mut self, view: View, r: Rig) {
        let feet = PersonSprite::FRAME_HEIGHT - 2;
        let look = self.look;
        let top = &look.top;
        let body = r.bob;
        let lean = if view == View::Side { r.lean } else { 0 };

        if look.cape && view == View::Back {
            for y in 13 + body..=feet - 2 {
                for x in 5..=14 {
                    self.c.set(x, y, if x == 5 || x == 14 { top[0] } else { top[1] });
                }
            }
        }

        if !look.robe {
            if view == View::Side {
                self.leg(9 + r.stride_l, feet - r.lift_l, true);
                self.leg(10 + r.stride_r, feet - r.lift_r, false);
            } else {
                let spread = if r.crouch { 1 } else { 0 };
                self.leg(7 - spread, feet - r.lift_l, true);
                self.leg(11 + spread, feet - r.lift_r, false);
            }
        }

        let torso_top = 13 + body;
        let torso_bottom = if look.robe { feet - 1 } else { 19 + (body - 1).max(0) };
        let (tx0, tx1) = if view == View::Side { (7 + lean, 12 + lean) } else { (6, 13) };

        if view == View::Side {
            self.arm(tx0 - 1, torso_top, r.swing_l, top[1], 0, false);
        }

        for y in torso_top..=torso_bottom {
            // Robes flare toward the hem.
            let flare = if look.robe { ((y - torso_top - 5) / 3).max(0) } else { 0 };
            for x in tx0 - flare..=tx1 + flare {
                let c = if x == tx0 - flare {
                    top[3]
                } else if x == tx1 + flare {
                    top[1]
                } else {
                    top[2]
                };
                self.c.set(x, y, c);
            }
        }
        if look.robe {
            let hem = torso_bottom + 1;
            for x in tx0 - 2..=tx1 + 2 {
                self.c.set(x, hem, top[1]);
            }
            self.c.set(tx0 + 1, hem + 1, look.boots[0]);
            self.c.set(tx1 - 1, hem + 1, look.boots[0]);
        } else {
            for x in tx0 - 1..=tx1 + 1 {
                self.c.set(x, torso_bottom + 1, top[1]);
            }
        }
        let belt_y = if look.robe { torso_top + 5 } else { torso_bottom - 1 };
        let belt = look.belt.unwrap_or(Rgba::hex(0x7A5A32));
        for x in tx0..=tx1 {
            self.c.set(x, belt_y, belt);
        }
        if view == View::Front {
            self.c.set(9, belt_y, Rgba::hex(0xE8C050));
            self.c.set(10, belt_y, Rgba::hex(0xE8C050));
            for y in torso_top + 1..belt_y {
                self.c.set(10, y, top[1]);
            }
        }

        if view == View::Side {
            self.arm(tx1 - 1, torso_top, r.swing_r, top[3], r.reach, r.arms_up);
        } else {
            self.arm(tx0 - 2, torso_top, r.swing_l, top[2], 0, r.arms_up && view == View::Front && r.lean < 0);
            self.arm(tx1 + 1, torso_top, r.swing_r, top[2], 0, r.arms_up);
        }

        let crouch = if r.crouch { 1 } else { 0 };
        self.head(view, 7 + body + crouch, lean);
        if let Some(scarf) = &look.scarf {
            self.scarf(view, 12 + body + crouch, lean, r.scarf_wave, scarf);
        }
    }

    fn leg(&mut self, x: i32, bottom: i32, left: bool) {
        let look = self.look;
        for y in bottom - 5..=bottom {
            for i in 0..2 {
                let c = if y >= bottom - 1 {
                    look.boots[if i == 0 && left { 1 } else { 0 }]
                } else {
                    look.legs[if i == 0 { 1 } else { 0 }]
                };
                self.c.set(x + i, y, c);
            }
        }
        self.c.set(x + 2, bottom, look.boots[0]);
    }

    fn arm(&mut self, x: i32, top: i32, swing: i32, sleeve: Rgba, reach: i32, up: bool) {
        let skin = &self.look.skin;
        if up {
            for y in top - 4..=top + 1 {
                self.c.set(x, y, sleeve);
                self.c.set(x + 1, y, sleeve);
            }
            self.c.set(x, top - 5, skin[2]);
            self.c.set(x + 1, top - 5, skin[2]);
            return;
        }
        let y0 = top + swing;
        let length = 5 - (reach - 1).max(0);
        for y in y0..y0 + length {
            self.c.set(x + reach, y, sleeve);
            self.c.set(x + 1 + reach, y, sleeve.lerp(self.look.top[0], 0.3));
        }
        self.c.set(x + reach, y0 + length, skin[2]);
        self.c.set(x + 1 + reach, y0 + length, skin[1]);
    }

    fn head(&mut self, view: View, base_y: i32, lean: i32) {
        let look = self.look;
        let (skin, hair) = (&look.skin, &look.hair);
        let top = base_y - 6;
        let (x0, x1) = if view == View::Side { (6 + lean, 13 + lean) } else { (5, 14) };
        for y in top + 2..=base_y + 3 {
            for x in x0 + 1..=x1 - 1 {
                self.c.set(x, y, skin[2]);
            }
        }
        for x in x0 + 1..=x1 - 1 {
            self.c.set(x, base_y + 3, skin[1]);
        }

        let bald = look.hair_style == HairStyle::Bald;
        if !bald {
            for y in top..=top + 3 {
                let inset = if y == top { 2 } else { 0 };
                for x in x0 + inset..=x1 - inset {
                    self.c.set(x, y, if y != top && (x + y) % 5 == 0 { hair[3] } else { hair[2] });
                }
            }
        } else {
            for y in top + 1..=top + 3 {
                for x in x0 + 1..=x1 - 1 {
                    let shine = y == top + 1 && x > x0 + 2 && x < x1 - 2;
                    self.c.set(x, y, if shine { skin[2].lerp(Rgba::WHITE, 0.3) } else { skin[2] });
                }
            }
        }

        match view {
            View::Front => {
                if !bald {
                    for x in x0 + 1..=x1 - 1 {
                        self.c.set(x, top + 4, if x % 3 == 0 { skin[2] } else { hair[1] });
                    }
                    let side_down = if look.hair_style == HairStyle::Long { base_y + 5 } else { base_y + 1 };
                    for y in top + 3..=side_down {
                        self.c.set(x0, y, hair[1]);
                        self.c.set(x1, y, hair[1]);
                    }
                }
                for (x, y) in [(x0 + 3, base_y), (x0 + 3, base_y + 1), (x1 - 3, base_y), (x1 - 3, base_y + 1)] {
                    self.c.set(x, y, EYE);
                }
                self.c.set(x0 + 3, base_y - 1, skin[1]);
                self.c.set(x1 - 3, base_y - 1, skin[1]);
                self.c.set(x0 + 2, base_y + 2, BLUSH);
                self.c.set(x1 - 2, base_y + 2, BLUSH);
                if look.beard {
                    for y in base_y + 2..=base_y + 5 {
                        let taper = (y - base_y - 2) / 2;
                        for x in x0 + 2 + taper..=x1 - 2 - taper {
                            let light = y == base_y + 2 && x > x0 + 3 && x < x1 - 3;
                            self.c.set(x, y, if light { hair[3] } else { hair[2] });
                        }
                    }
                }
            }
            View::Back => {
                if !bald {
                    let bottom = base_y + if look.hair_style == HairStyle::Long { 6 } else { 3 };
                    for y in top..=bottom {
                        for x in x0..=x1 {
                            if y > top || (x > x0 + 1 && x < x1 - 1) {
                                let c = if (x * 3 + y) % 7 == 0 {
                                    hair[3]
                                } else if y > base_y + 1 {
                                    hair[1]
                                } else {
                                    hair[2]
                                };
                                self.c.set(x, y, c);
                            }
                        }
                    }
                }
            }
            View::Side => {
                if !bald {
                    let back = if look.hair_style == HairStyle::Long { base_y + 6 } else { base_y + 3 };
                    for y in top + 3..=back {
                        for x in x0..=x0 + 3 {
                            self.c.set(x, y, if y > base_y + 1 { hair[1] } else { hair[2] });
                        }
                    }
                    for x in x0 + 4..=x1 - 1 {
                        self.c.set(x, top + 4, if x % 3 == 1 { skin[2] } else { hair[1] });
                    }
                }
                self.c.set(x1 - 2, base_y, EYE);
                self.c.set(x1 - 2, base_y + 1, EYE);
                self.c.set(x1, base_y + 1, skin[2]);
                self.c.set(x1 - 3, base_y + 2, BLUSH);
                if look.beard {
                    for y in base_y + 2..=base_y + 5 {
                        for x in x1 - 4..=x1 - if y > base_y + 3 { 1 } else { 0 } {
                            self.c.set(x, y, hair[2]);
                        }
                    }
                }
            }
        }
        if look.hair_style == HairStyle::Bun {
            let cx = if view == View::Side { x0 as f32 + 1.5 } else { (x0 + x1) as f32 / 2.0 + 0.5 };
            self.c.ellipse(cx, (top - 1) as f32, 2.2, 2.2, hair[2]);
        }
        if look.hair_style == HairStyle::Spiky {
            for x in (x0 + 1..=x1 - 1).step_by(2) {
                self.c.set(x, top - 1, hair[3]);
            }
        }
        if !bald {
            self.c.set(x0 + 1, top + 1, hair[3]);
        }
        self.hat(view, top, base_y, x0, x1);
    }

    fn hat(&mut self, view: View, top: i32, base_y: i32, x0: i32, x1: i32) {
        let look = self.look;
        let hat = look.hat.as_ref().unwrap_or(&look.top);
        match look.headwear {
            Headwear::None => {}
            Headwear::Hood => {
                for y in top - 1..=base_y + 3 {
                    for x in x0 - 1..=x1 + 1 {
                        let face = view != View::Back
                            && y >= top + 3
                            && if view == View::Front { x > x0 && x < x1 } else { x > x0 + 3 };
                        if face || (y == top - 1 && (x == x0 - 1 || x == x1 + 1)) {
                            continue;
                        }
                        self.c.set(x, y, if x == x0 - 1 || y == top - 1 { hat[1] } else { hat[2] });
                    }
                }
            }
            Headwear::WizardHat => {
                let mid = (x0 + x1) / 2;
                for y in top - 7..=top + 2 {
                    let half = ((y - (top - 7)) / 2).max(0);
                    for x in mid - half..=mid + 1 + half {
                        self.c.set(x, y, if x == mid - half { hat[3] } else { hat[2] });
                    }
                }
                for x in x0 - 2..=x1 + 2 {
                    self.c.set(x, top + 2, hat[1]);
                }
                self.c.set(mid + 1, top - 2, Rgba::hex(0xF8D850));
            }
            Headwear::Helmet => {
                for y in top - 1..=top + 4 {
                    for x in x0..=x1 {
                        let c = if y == top - 1 && (x == x0 || x == x1) {
                            Rgba::TRANSPARENT
                        } else if x < x0 + 3 {
                            hat[3]
                        } else {
                            hat[2]
                        };
                        self.c.set(x, y, c);
                    }
                }
                if view != View::Back {
                    for x in if view == View::Side { x0 + 4 } else { x0 + 1 }..=x1 - 1 {
                        self.c.set(x, top + 4, hat[0]);
                    }
                }
                self.c.set((x0 + x1) / 2, top - 2, Rgba::hex(0xC84A3A));
            }
            Headwear::Cap => {
                for y in top..=top + 2 {
                    for x in x0..=x1 {
                        self.c.set(x, y, hat[2]);
                    }
                }
                if view == View::Side {
                    for x in x1..=x1 + 2 {
                        self.c.set(x, top + 2, hat[1]);
                    }
                } else if view == View::Front {
                    for x in x0 + 1..=x1 - 1 {
                        self.c.set(x, top + 3, hat[1]);
                    }
                }
            }
            Headwear::Headband => {
                for x in x0..=x1 {
                    self.c.set(x, top + 3, hat[2]);
                }
                if view != View::Front {
                    self.c.set(x0 - 1, top + 4, hat[2]);
                    self.c.set(x0 - 2, top + 5, hat[2]);
                }
            }
        }
    }

    fn scarf(&mut self, view: View, y: i32, lean: i32, wave: i32, scarf: &[Rgba]) {
        let (x0, x1) = if view == View::Side { (7 + lean, 12 + lean) } else { (5, 14) };
        for x in x0..=x1 {
            self.c.set(x, y, scarf[1]);
            self.c.set(x, y + 1, if x % 2 == 0 { scarf[0] } else { scarf[1] });
        }
        self.c.set(x0 + 1, y, scarf[2]);
        match view {
            View::Side => {
                for i in 0..5 {
                    let (tx, ty) = (x0 - 1 - i, y + 1 + (i + wave) % 3 / 2);
                    self.c.set(tx, ty, if i % 2 == 0 { scarf[1] } else { scarf[2] });
                    self.c.set(tx, ty + 1, scarf[0]);
                }
            }
            View::Back => {
                for i in 0..6 {
                    let (tx, ty) = (10 + (i + wave) % 4 / 2 - 1, y + 2 + i);
                    self.c.set(tx, ty, scarf[1]);
                    self.c.set(tx + 1, ty, scarf[0]);
                }
            }
            View::Front => {
                self.c.set(x1, y + 2, scarf[1]);
                self.c.set(x1, y + 3, scarf[0]);
                self.c.set(x1 + 1, y + 3 + wave % 2, scarf[1]);
            }
        }
    }
}

/// A soft ground shadow drawn under every actor.
pub fn shadow(width: i32, height: i32) -> Canvas {
    let mut c = Canvas::new(width, height);
    c.ellipse(width as f32 / 2.0, height as f32 / 2.0, width as f32 / 2.0, height as f32 / 2.0, Rgba::new(0, 0, 0, 90));
    c
}
