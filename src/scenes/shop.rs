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
use crate::gameplay::items::{self, CONSUMABLE_CAP, ItemDef, ItemKind};
use crate::scene_any;

pub struct ShopScene {
    shop: ShopDef,
    row: usize,
    message: String,
}

impl ShopScene {
    pub fn new(id: &str) -> ShopScene {
        let shop = data().shops.get(id).cloned().unwrap_or_default();
        ShopScene { shop, row: 0, message: String::new() }
    }

    fn rect(&self) -> IRect {
        IRect::new((SCREEN_WIDTH - 300) / 2, 16, 300, 110.max(30 + (self.shop.items.len() as i32 + 1) * 13))
    }

    fn item(&self, index: usize) -> &'static ItemDef {
        items::get(&self.shop.items[index]).expect("shop items are filtered on enter")
    }

    fn buy(&mut self, state: &mut GameState) {
        let item = self.item(self.row);
        if owned(state, item) {
            self.message = "You already own that.".into();
            audio::sfx(Sound::MenuCancel);
        } else if item.kind == ItemKind::Consumable && state.count(item.id) >= CONSUMABLE_CAP {
            self.message = format!("You can only carry {CONSUMABLE_CAP}.");
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
}

fn is_gear(item: &ItemDef) -> bool {
    matches!(item.kind, ItemKind::Armor | ItemKind::Shield | ItemKind::Accessory)
}

fn owned(state: &GameState, item: &ItemDef) -> bool {
    is_gear(item) && state.equipment.iter().any(|e| e == item.id)
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
        font::draw(batch, "SHOP", Vec2::new((rect.x + 8) as f32, (rect.y + 6) as f32), palette::HIGHLIGHT, shadow);
        let gold = format!("{} G", state.gold);
        let gold_pos = Vec2::new((rect.right() - 8 - font::measure(&gold)) as f32, (rect.y + 6) as f32);
        font::draw(batch, &gold, gold_pos, palette::GOLD, shadow);

        for i in 0..=self.shop.items.len() {
            let y = rect.y + 22 + i as i32 * 13;
            if i == self.shop.items.len() {
                font::draw(batch, "Leave", Vec2::new((rect.x + 30) as f32, y as f32), palette::TEXT, shadow);
            } else {
                let item = self.item(i);
                let owned = owned(state, item);
                batch.draw(props::icon(item.id), Vec2::new((rect.x + 16) as f32, (y - 2) as f32), Rgba::WHITE);
                let color = if owned { palette::TEXT_DIM } else { palette::TEXT };
                font::draw(batch, item.name, Vec2::new((rect.x + 30) as f32, y as f32), color, shadow);
                let price = if owned {
                    "Owned".to_string()
                } else if item.kind == ItemKind::Consumable {
                    format!("({})  {} G", state.count(item.id), item.price)
                } else {
                    format!("{} G", item.price)
                };
                let price_pos = Vec2::new((rect.right() - 10 - font::measure(&price)) as f32, y as f32);
                font::draw(batch, &price, price_pos, if owned { palette::TEXT_DIM } else { palette::GOLD }, None);
            }
            if i == self.row {
                font::draw(
                    batch,
                    &CURSOR.to_string(),
                    Vec2::new((rect.x + 7) as f32, y as f32),
                    palette::HIGHLIGHT,
                    None,
                );
            }
        }

        let footer = IRect::new(rect.x, rect.bottom() + 2, rect.w, 30);
        batch.draw_window(footer);
        let text = if !self.message.is_empty() {
            self.message.as_str()
        } else if self.row < self.shop.items.len() {
            self.item(self.row).description
        } else {
            "Come again!"
        };
        let first = font::wrap(text, footer.w - 16).into_iter().next().unwrap_or_default();
        font::draw(batch, &first, Vec2::new((footer.x + 8) as f32, (footer.y + 6) as f32), palette::TEXT, shadow);
    }
}
