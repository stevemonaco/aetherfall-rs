//! People drawn procedurally from a small rig (head, hair or hat, scarf, torso, arms, legs) so the hero and every
//! villager share one look. Frames are 20x26 with the feet at the bottom centre; side views face right.

use bevy::math::Vec2;

use crate::core::canvas::Canvas;
use crate::core::color::{Rgba, ramp};
use crate::core::direction::Direction;
use crate::core::geom::{IRect, snap};
use crate::core::gfx::{self, Batch, TexId};
use crate::core::noise;
use crate::core::palette::OUTLINE;
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

/// What a person looks like: palette ramps (dark to light) and a few style switches. `key` names the cached sheet.
#[derive(Clone)]
pub struct Appearance {
    pub key: String,
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
    fn new(key: &str, skin: Vec<Rgba>, hair: Vec<Rgba>, top: Vec<Rgba>, legs: Vec<Rgba>, boots: Vec<Rgba>, hair_style: HairStyle) -> Appearance {
        Appearance {
            key: key.to_string(),
            skin,
            hair,
            top,
            legs,
            boots,
            hair_style,
            headwear: Headwear::None,
            hat: None,
            scarf: None,
            robe: false,
            beard: false,
            belt: None,
            cape: false,
        }
    }

    fn wear(mut self, headwear: Headwear, hat: Option<Vec<Rgba>>) -> Appearance {
        self.headwear = headwear;
        self.hat = hat;
        self
    }

    fn robed(mut self) -> Appearance {
        self.robe = true;
        self
    }

    fn bearded(mut self) -> Appearance {
        self.beard = true;
        self
    }

    pub fn hero() -> Appearance {
        let mut look = Appearance::new(
            "hero",
            people::skin(1),
            ramp(&[0x3A2216, 0x6A3E22, 0x9A5E30, 0xC08A48]),
            ramp(&[0x22305A, 0x3A5498, 0x5A7ECC, 0x8AAEEC]),
            ramp(&[0x221C2C, 0x3C3248]),
            ramp(&[0x2E1C12, 0x6A4226]),
            HairStyle::Short,
        );
        look.scarf = Some(ramp(&[0x16645A, 0x26A08C, 0x66DCC4]));
        look
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
                    frame.outline_with(OUTLINE);
                    sheet.blit(&frame, Self::column(pose, f) * Self::FRAME_WIDTH, v as i32 * Self::FRAME_HEIGHT);
                }
            }
        }
        sheet
    }
}

impl PersonSprite {
    pub fn sheet(look: &Appearance) -> TexId {
        gfx::cached(&format!("person_{}", look.key), || PersonSprite::build(look))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(batch: &mut Batch, look: &Appearance, feet: Vec2, facing: Direction, pose: HeroPose, frame: i32, color: Rgba, silhouette: bool, scale: f32, rotation: f32) {
        let view = match facing {
            Direction::Up => 1,
            Direction::Left | Direction::Right => 2,
            Direction::Down => 0,
        };
        let src = IRect::new(Self::column(pose, frame) * Self::FRAME_WIDTH, view * Self::FRAME_HEIGHT, Self::FRAME_WIDTH, Self::FRAME_HEIGHT);
        let sheet = Self::sheet(look);
        let tex = if silhouette { gfx::silhouette(sheet) } else { sheet };
        let origin = Vec2::new(Self::FRAME_WIDTH as f32 / 2.0, (Self::FRAME_HEIGHT - 1) as f32);
        batch.draw_ex(tex, snap(feet), Some(src), color, rotation, origin, Vec2::splat(scale), facing == Direction::Left, false);
    }
}

/// The hero's look and convenience drawing.
pub mod hero_sprite {
    use std::sync::LazyLock;

    use super::*;

    pub static LOOK: LazyLock<Appearance> = LazyLock::new(Appearance::hero);

    #[allow(clippy::too_many_arguments)]
    pub fn draw(batch: &mut Batch, feet: Vec2, facing: Direction, pose: HeroPose, frame: i32, color: Rgba, silhouette: bool, scale: f32, rotation: f32) {
        PersonSprite::draw(batch, &LOOK, feet, facing, pose, frame, color, silhouette, scale, rotation);
    }
}

/// Named looks for the townsfolk, keyed by the sprite names map files use.
pub mod people {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    use super::*;

    pub fn skin(tone: i32) -> Vec<Rgba> {
        match tone {
            0 => ramp(&[0xC08A64, 0xF0BE98, 0xFFE0C4]),
            2 => ramp(&[0x7A4A2A, 0xB07850, 0xD8A070]),
            3 => ramp(&[0x4A2A1A, 0x7A4A30, 0xA06A44]),
            _ => ramp(&[0xB07850, 0xE0A87C, 0xF6D2AC]),
        }
    }

    fn white() -> Vec<Rgba> {
        ramp(&[0x8A8A9A, 0xC0C0CC, 0xE8E8F0, 0xFFFFFF])
    }
    fn grey() -> Vec<Rgba> {
        ramp(&[0x4A4A58, 0x7A7A8A, 0xA8A8B8, 0xD0D0DC])
    }
    fn brown() -> Vec<Rgba> {
        ramp(&[0x2E1C12, 0x5A3A22, 0x8A5A32, 0xB07A48])
    }
    fn black() -> Vec<Rgba> {
        ramp(&[0x14121C, 0x26222E, 0x3A3444, 0x524A5E])
    }
    fn blond() -> Vec<Rgba> {
        ramp(&[0x8A6A2A, 0xC8A040, 0xE8C860, 0xFFE8A0])
    }
    fn red() -> Vec<Rgba> {
        ramp(&[0x5A1A12, 0x8A2E1E, 0xB84A2A, 0xE07A4A])
    }
    fn dark_pants() -> Vec<Rgba> {
        ramp(&[0x221C2C, 0x3C3248])
    }
    fn brown_pants() -> Vec<Rgba> {
        ramp(&[0x3A2618, 0x5A3E28])
    }
    fn boots() -> Vec<Rgba> {
        ramp(&[0x2E1C12, 0x6A4226])
    }
    fn cloth(dark: u32, mid: u32, light: u32, lit: u32) -> Vec<Rgba> {
        ramp(&[dark, mid, light, lit])
    }

    fn looks() -> HashMap<String, Appearance> {
        use HairStyle::*;
        use Headwear::*;
        let a = Appearance::new;
        let list = vec![
            a("oldman1", skin(0), white(), cloth(0x3A2A1A, 0x6A4A2A, 0x8A6A3A, 0xB08A5A), brown_pants(), boots(), Bald).robed().bearded(),
            a("oldman2", skin(1), grey(), cloth(0x1E3A2A, 0x2E5A3E, 0x427A52, 0x6AA070), brown_pants(), boots(), Short).robed().bearded(),
            a("oldman3", skin(2), white(), cloth(0x22305A, 0x3A4A88, 0x5A6AB0, 0x8A9AD0), dark_pants(), boots(), Long).robed().bearded(),
            a("adventurer_f2", skin(1), red(), cloth(0x2A4A1E, 0x3E6A2E, 0x5A8A3E, 0x8AB060), brown_pants(), boots(), Long),
            a("adventurer_f3", skin(3), black(), cloth(0x5A1E3A, 0x8A2E5A, 0xB04A7A, 0xD87AA0), dark_pants(), boots(), Bun),
            a("adventurer_f4", skin(0), blond(), cloth(0x2A3A6A, 0x3E5A9A, 0x5A7ACA, 0x8AA8E8), dark_pants(), boots(), Long)
                .wear(Headband, Some(ramp(&[0x8A1E1E, 0xC03A3A, 0xE05A5A, 0xF08A8A]))),
            a("adventurer_m3", skin(2), brown(), cloth(0x5A3A12, 0x8A5A1E, 0xB07A30, 0xD8A050), dark_pants(), boots(), Spiky),
            a("adventurer_m4", skin(1), black(), cloth(0x3A1A1A, 0x6A2A2A, 0x9A3E3E, 0xC06A5A), brown_pants(), boots(), Short)
                .wear(Cap, Some(ramp(&[0x1A2A4A, 0x2A4A7A, 0x3E6AA8, 0x6A9AD0]))),
            {
                let mut l = a("cleric1", skin(1), blond(), white(), dark_pants(), boots(), Short).wear(Hood, Some(white())).robed();
                l.belt = Some(Rgba::hex(0xC8A040));
                l
            },
            {
                let mut l = a("cleric2", skin(2), grey(), cloth(0x8A8A9A, 0xC0C0CC, 0xE8E8F0, 0xFFFFFF), dark_pants(), boots(), Bald).robed();
                l.belt = Some(Rgba::hex(0x3A6AB0));
                l
            },
            a("cleric3", skin(3), black(), cloth(0x5A4A12, 0x8A7A2A, 0xB0A048, 0xD8C870), dark_pants(), boots(), Bun).robed(),
            a("fighter1", skin(1), brown(), grey(), dark_pants(), boots(), Short)
                .wear(Headband, Some(ramp(&[0x6A1A1A, 0x9A2A2A, 0xC03A3A, 0xE06A6A]))),
            a("fighter3", skin(2), red(), grey(), brown_pants(), boots(), Spiky),
            a("warrior2", skin(3), black(), cloth(0x4A2A12, 0x6A4A22, 0x8A6A3A, 0xB08A5A), brown_pants(), boots(), Short).wear(Helmet, Some(grey())),
            {
                let mut l = a("knight3", skin(0), blond(), grey(), dark_pants(), boots(), Short).wear(Helmet, Some(grey()));
                l.cape = true;
                l
            },
            a("guard1", skin(1), brown(), cloth(0x1A2A5A, 0x2A4A8A, 0x3E6AB8, 0x6A9AE0), dark_pants(), boots(), Short).wear(Helmet, Some(grey())),
            a("guard2", skin(2), black(), cloth(0x5A1A1A, 0x8A2A2A, 0xB03E3E, 0xD86A6A), dark_pants(), boots(), Short).wear(Helmet, Some(grey())),
            a("guard3", skin(3), brown(), cloth(0x1A4A3A, 0x2A6A54, 0x3E8A70, 0x6AB09A), dark_pants(), boots(), Short).wear(Helmet, Some(grey())),
            a("hunter1", skin(1), brown(), cloth(0x1E3A1A, 0x2E5A26, 0x427A36, 0x6AA05A), brown_pants(), boots(), Short)
                .wear(Hood, Some(cloth(0x1E3A1A, 0x2E5A26, 0x427A36, 0x6AA05A))),
            a("hunter2", skin(2), blond(), cloth(0x3A2A12, 0x5A4220, 0x7A5A30, 0xA07A48), brown_pants(), boots(), Long),
            a("hunter3", skin(0), red(), cloth(0x2A3A1A, 0x3E5426, 0x566E36, 0x7A9450), dark_pants(), boots(), Bun).wear(Cap, Some(brown())),
            a("mage3", skin(1), white(), cloth(0x2A1A4A, 0x4A2E7A, 0x6A46A8, 0x9A72D0), dark_pants(), boots(), Long)
                .wear(WizardHat, Some(cloth(0x2A1A4A, 0x4A2E7A, 0x6A46A8, 0x9A72D0)))
                .robed(),
            a("wizard1", skin(0), grey(), cloth(0x1A2A4A, 0x2A447A, 0x3E62A8, 0x6A8ED0), dark_pants(), boots(), Short)
                .wear(WizardHat, Some(cloth(0x1A2A4A, 0x2A447A, 0x3E62A8, 0x6A8ED0)))
                .robed()
                .bearded(),
            a("ninja1", skin(1), black(), black(), dark_pants(), boots(), Short).wear(Hood, Some(black())),
            {
                let mut l = a("thief2", skin(2), brown(), cloth(0x2A2A3A, 0x3E3E54, 0x56566E, 0x7A7A94), dark_pants(), boots(), Spiky);
                l.scarf = Some(ramp(&[0x5A1A1A, 0x8A2A2A, 0xC04A4A]));
                l
            },
            a("thief3", skin(1), red(), cloth(0x3A2A12, 0x5A4220, 0x7A5A30, 0xA07A48), dark_pants(), boots(), Short)
                .wear(Hood, Some(cloth(0x2A2A3A, 0x3E3E54, 0x56566E, 0x7A7A94))),
            {
                let mut l = a("rogue1", skin(1), brown(), cloth(0x3A2A1A, 0x5A4228, 0x7A5A3A, 0xA07A54), brown_pants(), boots(), Short)
                    .wear(Cap, Some(cloth(0x5A1A1A, 0x8A2A2A, 0xB03E3E, 0xD86A6A)));
                l.scarf = Some(ramp(&[0x6A5A1A, 0x9A8A2A, 0xC8B850]));
                l
            },
            a("rogue2", skin(3), black(), cloth(0x1E3A3A, 0x2E5A5A, 0x427A7A, 0x6AA0A0), dark_pants(), boots(), Long)
                .wear(Headband, Some(ramp(&[0x6A5A1A, 0x9A8A2A, 0xC8B850, 0xE8D880]))),
        ];
        list.into_iter().map(|l| (l.key.clone(), l)).collect()
    }

    static LOOKS: LazyLock<Mutex<HashMap<String, Appearance>>> = LazyLock::new(|| Mutex::new(looks()));

    /// The look registered under `name`, or a stable look generated from the name.
    pub fn get(name: &str) -> Appearance {
        let mut table = LOOKS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(look) = table.get(name) {
            return look.clone();
        }
        let seed = name.bytes().fold(0i32, |h, b| h.wrapping_mul(31).wrapping_add(b as i32));
        let h = noise::hash(seed, 0, 7);
        let hairs = [white(), grey(), brown(), black(), blond(), red()];
        let cloths = [
            cloth(0x3A2A1A, 0x6A4A2A, 0x8A6A3A, 0xB08A5A),
            cloth(0x1E3A2A, 0x2E5A3E, 0x427A52, 0x6AA070),
            cloth(0x22305A, 0x3A4A88, 0x5A6AB0, 0x8A9AD0),
            cloth(0x5A1E3A, 0x8A2E5A, 0xB04A7A, 0xD87AA0),
            cloth(0x5A3A12, 0x8A5A1E, 0xB07A30, 0xD8A050),
            cloth(0x3A1A1A, 0x6A2A2A, 0x9A3E3E, 0xC06A5A),
        ];
        let styles = [HairStyle::Short, HairStyle::Long, HairStyle::Bald, HairStyle::Bun, HairStyle::Spiky];
        let mut look = Appearance::new(
            name,
            skin((h % 4) as i32),
            hairs[((h >> 3) % hairs.len() as u32) as usize].clone(),
            cloths[((h >> 6) % cloths.len() as u32) as usize].clone(),
            if h & 512 != 0 { dark_pants() } else { brown_pants() },
            boots(),
            styles[((h >> 10) % 5) as usize],
        );
        if (h >> 13).is_multiple_of(4) {
            look.headwear = Headwear::Cap;
        }
        look.hat = Some(cloths[((h >> 15) % cloths.len() as u32) as usize].clone());
        look.robe = (h >> 18).is_multiple_of(5);
        table.insert(name.to_string(), look.clone());
        look
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