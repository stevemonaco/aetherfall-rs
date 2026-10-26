use bevy::math::Vec2;

use super::scene::{Scene, SceneCtx, world_below};
use super::world::SCREEN_WIDTH;
use crate::art::props;
use crate::core::audio::{self, Sound};
use crate::core::color::Rgba;
use crate::core::font::{self, CURSOR};
use crate::core::geom::IRect;
use crate::core::gfx::Batch;
use crate::core::input::{self, GameAction};
use crate::core::palette;
use crate::gameplay::game_data::{ShopDef, data};
use crate::gameplay::game_state::GameState;
use crate::gameplay::items::{self, ItemDef, ItemKind, consumable_cap};
use crate::scene_any;
use crate::ui::widgets::{self, SLOT};

const VISIBLE: usize = 9;
const LINE: i32 = 20;
const LIST_W: i32 = 256;
const DETAIL_W: i32 = 176;
const GAP: i32 = 4;

pub struct ShopScene {
    shop: ShopDef,
    row: usize,
    scroll: usize,
    message: String,
}

impl ShopScene {
    pub fn new(id: &str) -> ShopScene {
        let shop = data().shops.get(id).cloned().unwrap_or_default();
        ShopScene { shop, row: 0, scroll: 0, message: String::new() }
    }

    fn rect(&self) -> IRect {
        let shown = (self.shop.items.len() + 1).min(VISIBLE) as i32;
        let x = (SCREEN_WIDTH - LIST_W - GAP - DETAIL_W) / 2;
        IRect::new(x, 14, LIST_W, 150.max(30 + shown * LINE))
    }

    fn detail_rect(&self) -> IRect {
        let list = self.rect();
        IRect::new(list.right() + GAP, list.y, DETAIL_W, list.h)
    }

    fn item(&self, index: usize) -> &'static ItemDef {
        items::get(&self.shop.items[index]).expect("shop items are filtered on enter")
    }

    fn buy(&mut self, state: &mut GameState) {
        let item = self.item(self.row);
        if owned(state, item) {
            self.message = "You already own that.".into();
            audio::sfx(Sound::MenuCancel);
        } else if item.kind == ItemKind::Consumable && state.count(item.id) >= consumable_cap(item.id) {
            self.message = format!("You can only carry {}.", consumable_cap(item.id));
            audio::sfx(Sound::MenuCancel);
        } else if state.gold < item.price {
            self.message = "You can't afford that.".into();
            audio::sfx(Sound::MenuCancel);
        } else {
            state.gold -= item.price;
            state.give(item.id, 1);
            self.message = if is_gear(item) {
                format!("The {} suits you! (Equip it in the Gear tab.)", item.name)
            } else {
                format!("Bought a {}.", item.name)
            };
            audio::sfx(Sound::Coin);
        }
    }

    fn draw_detail(&self, state: &GameState, batch: &mut Batch) {
        let rect = self.detail_rect();
        batch.draw_window(rect);
        let shadow = Some(palette::TEXT_SHADOW);
        let (x, y, right) = (rect.x as f32, rect.y as f32, rect.right() as f32);
        if self.row >= self.shop.items.len() {
            font::draw(batch, "Come again!", Vec2::new(x + 10.0, y + 10.0), palette::TEXT_DIM, shadow);
            return;
        }
        let item = self.item(self.row);
        let (kind, accent) = kind_style(item.kind);
        let frame = IRect::new(rect.x + 8, rect.y + 8, SLOT + 10, SLOT + 10);
        batch.fill(frame, palette::WINDOW_FILL_LIGHT.mul(0.4));
        batch.draw_rect(frame, accent.mul(0.8));
        let (icon, at, scale) = (props::icon(item.id), Vec2::new(x + 10.0, y + 10.0), Vec2::splat(2.0));
        batch.draw_ex(icon, at, None, Rgba::WHITE, 0.0, Vec2::ZERO, scale, false, false);
        let name_x = (frame.right() + 6) as f32;
        font::draw(batch, item.name, Vec2::new(name_x, y + 11.0), palette::TEXT, shadow);
        font::draw(batch, kind, Vec2::new(name_x, y + 23.0), accent, None);
        widgets::rule(batch, rect.x + 6, frame.bottom() + 5, rect.w - 12);

        for (i, line) in font::wrap(item.description, rect.w - 20).iter().enumerate() {
            let pos = Vec2::new(x + 10.0, (frame.bottom() + 11) as f32 + i as f32 * 11.0);
            font::draw(batch, line, pos, palette::TEXT, shadow);
        }

        let mut line_y = rect.bottom() as f32 - 18.0;
        if item.kind == ItemKind::Consumable {
            let (held, cap) = (state.count(item.id), consumable_cap(item.id));
            let meter = IRect::new(rect.x + 10, rect.bottom() - 13, rect.w - 20, 4);
            widgets::bar(batch, meter, held as f32 / cap.max(1) as f32, accent);
            line_y -= 8.0;
            font::draw(batch, "Carrying", Vec2::new(x + 10.0, line_y), palette::TEXT_DIM, None);
            let full = if held >= cap { palette::HIGHLIGHT } else { palette::TEXT };
            widgets::right_text(batch, &format!("{held} / {cap}"), right - 10.0, line_y, full);
            line_y -= 12.0;
        } else if item.kind == ItemKind::Material {
            font::draw(batch, "Carrying", Vec2::new(x + 10.0, line_y), palette::TEXT_DIM, None);
            widgets::right_text(batch, &state.count(item.id).to_string(), right - 10.0, line_y, palette::TEXT);
            line_y -= 12.0;
        }
        if owned(state, item) {
            font::draw(batch, "Already yours", Vec2::new(x + 10.0, line_y), palette::TEXT_DIM, None);
        } else {
            font::draw(batch, "Price", Vec2::new(x + 10.0, line_y), palette::TEXT_DIM, None);
            widgets::right_text(batch, &format!("{} G", item.price), right - 10.0, line_y, price_color(state, item));
        }
        widgets::rule(batch, rect.x + 6, line_y as i32 - 6, rect.w - 12);
    }
}

fn is_gear(item: &ItemDef) -> bool {
    matches!(item.kind, ItemKind::Armor | ItemKind::Shield | ItemKind::Accessory)
}

fn owned(state: &GameState, item: &ItemDef) -> bool {
    is_gear(item) && state.equipment.iter().any(|e| e == item.id)
}

fn price_color(state: &GameState, item: &ItemDef) -> Rgba {
    if state.gold < item.price { palette::GOLD.lerp(palette::TEXT_DIM, 0.6) } else { palette::GOLD }
}

fn kind_style(kind: ItemKind) -> (&'static str, Rgba) {
    match kind {
        ItemKind::Consumable => ("Consumable", Rgba::rgb(140, 220, 130)),
        ItemKind::Armor => ("Armor", Rgba::rgb(150, 190, 240)),
        ItemKind::Shield => ("Shield", Rgba::rgb(150, 190, 240)),
        ItemKind::Accessory => ("Accessory", Rgba::rgb(220, 160, 255)),
        ItemKind::Charm => ("Charm", Rgba::rgb(220, 160, 255)),
        ItemKind::Material => ("Material", Rgba::rgb(220, 180, 120)),
        ItemKind::Support => ("Support gem", Rgba::rgb(120, 230, 230)),
        ItemKind::Powerup => ("Relic", palette::HIGHLIGHT),
        ItemKind::Key => ("Key item", palette::HIGHLIGHT),
    }
}

impl Scene for ShopScene {
    scene_any!();

    fn is_overlay(&self) -> bool {
        true
    }

    fn enter(&mut self, _ctx: &mut SceneCtx) {
        input::consume();
        self.shop.items.retain(|id| items::get(id).is_some());
        self.message = self.shop.greeting.clone();
    }

    fn update(&mut self, ctx: &mut SceneCtx, _dt: f32) {
        let count = self.shop.items.len() + 1;
        let before = self.row;
        if input::pressed(GameAction::Up) {
            self.row = (self.row + count - 1) % count;
        }
        if input::pressed(GameAction::Down) {
            self.row = (self.row + 1) % count;
        }
        if before != self.row {
            audio::sfx(Sound::MenuMove);
            self.message.clear();
        }
        self.scroll = self.scroll.clamp((self.row + 1).saturating_sub(VISIBLE), self.row);

        let leave = self.row == self.shop.items.len();
        if input::pressed(GameAction::Cancel) || (input::pressed(GameAction::Confirm) && leave) {
            audio::sfx(Sound::MenuCancel);
            ctx.pop();
            input::consume();
            return;
        }

        if input::pressed(GameAction::Confirm)
            && let Some(world) = ctx.world()
        {
            self.buy(&mut world.state);
        }
    }
    fn draw(&self, below: &[Box<dyn Scene>], batch: &mut Batch) {
        let Some(world) = world_below(below) else { return };
        let state = &world.state;
        batch.begin_screen();
        let rect = self.rect();
        batch.draw_window(rect);
        let shadow = Some(palette::TEXT_SHADOW);
        let (x, right) = (rect.x as f32, rect.right() as f32);
        font::draw(batch, "SHOP", Vec2::new(x + 8.0, (rect.y + 6) as f32), palette::HIGHLIGHT, shadow);
        let gold = format!("{} G", state.gold);
        let gold_pos = Vec2::new(right - 10.0 - font::measure(&gold) as f32, (rect.y + 6) as f32);
        font::draw(batch, &gold, gold_pos, palette::GOLD, shadow);
        widgets::rule(batch, rect.x + 6, rect.y + 18, rect.w - 12);

        let count = self.shop.items.len() + 1;
        let top = rect.y + 24;
        for i in self.scroll..count.min(self.scroll + VISIBLE) {
            let slot_y = top + (i - self.scroll) as i32 * LINE;
            let y = (slot_y + 5) as f32;
            if i == self.shop.items.len() {
                font::draw(batch, "Leave", Vec2::new(x + 42.0, y), palette::TEXT, shadow);
            } else {
                let item = self.item(i);
                let owned = owned(state, item);
                let accent = kind_style(item.kind).1;
                let at = Vec2::new(x + 18.0, slot_y as f32);
                widgets::icon_slot(batch, at, Some(props::icon(item.id)), Some(accent), owned);
                let color = if owned { palette::TEXT_DIM } else { palette::TEXT };
                font::draw(batch, item.name, Vec2::new(x + 42.0, y), color, shadow);
                let (price, color) = if owned {
                    ("Owned".to_string(), palette::TEXT_DIM)
                } else {
                    (format!("{} G", item.price), price_color(state, item))
                };
                widgets::right_text(batch, &price, right - 10.0, y, color);
                if matches!(item.kind, ItemKind::Consumable | ItemKind::Material) {
                    let held = format!("x{}", state.count(item.id));
                    widgets::right_text(batch, &held, right - 54.0, y, palette::TEXT_DIM);
                }
            }
            if i == self.row {
                font::draw(batch, &CURSOR.to_string(), Vec2::new(x + 8.0, y), palette::HIGHLIGHT, None);
            }
        }
        let arrow_x = right - 12.0;
        if self.scroll > 0 {
            font::draw(batch, "^", Vec2::new(arrow_x, (top - 4) as f32), palette::TEXT_DIM, None);
        }
        if self.scroll + VISIBLE < count {
            font::draw(batch, "v", Vec2::new(arrow_x, (rect.bottom() - 10) as f32), palette::TEXT_DIM, None);
        }

        self.draw_detail(state, batch);
        if !self.message.is_empty() {
            let detail = self.detail_rect();
            let footer = IRect::new(rect.x, rect.bottom() + 2, detail.right() - rect.x, 20);
            batch.draw_window(footer);
            let first = font::wrap(&self.message, footer.w - 16).into_iter().next().unwrap_or_default();
            let pos = Vec2::new((footer.x + 8) as f32, (footer.y + 6) as f32);
            font::draw(batch, &first, pos, palette::TEXT, shadow);
        }
    }
}
