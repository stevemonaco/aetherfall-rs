//! Loading a map: baking its terrain, laying the water layer and spawning the hero and monsters.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::actors::{Body, Enemy, Facing, Player, SpriteOffset};
use crate::art::{self, Appearance, Canvas, PersonSprite};
use crate::baker;
use crate::data::{EnemyDef, JsonDoc, parse_enemies};
use crate::map::{MapData, TILE, Terrain, TileMap};
use crate::noise;
use crate::theme::TerrainTheme;
use crate::{AppState, CameraState};

const CHUNK: i32 = 256;

/// The maps the N key cycles through, chosen to show off each terrain theme.
pub const SHOWCASE: [&str; 10] = [
    "fields",
    "leafhollow",
    "marsh",
    "frostpeak",
    "whisperwood",
    "temple_1",
    "ashen_reach",
    "cinderfall",
    "citadel_1",
    "ashford",
];

#[derive(Resource)]
pub struct Level {
    pub id: String,
    pub name: String,
    pub map: TileMap,
    pub spawn: Vec2,
    pub bake_ms: f32,
}

#[derive(Resource)]
pub struct Pending {
    pub index: usize,
    pub id: String,
    pub map: Handle<JsonDoc>,
    pub enemies: Handle<JsonDoc>,
}

#[derive(Resource)]
pub struct EnemyDefs(pub HashMap<String, EnemyDef>);

/// Everything belonging to the current map, despawned when the map changes.
#[derive(Component)]
pub struct LevelEntity;

#[derive(Component)]
pub struct WaterTile(pub u32);

pub fn request_map(index: usize, id: &str, assets: &AssetServer, enemies: Handle<JsonDoc>) -> Pending {
    Pending { index, id: id.into(), map: assets.load(format!("data/maps/{id}.json")), enemies }
}

#[allow(clippy::too_many_arguments)]
pub fn finish_loading(
    mut commands: Commands,
    pending: Res<Pending>,
    docs: Res<Assets<JsonDoc>>,
    defs: Option<Res<EnemyDefs>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut camera: ResMut<CameraState>,
    mut next: ResMut<NextState<AppState>>,
) {
    let (Some(map_doc), Some(enemy_doc)) = (docs.get(&pending.map), docs.get(&pending.enemies)) else {
        return;
    };
    let defs = match defs {
        Some(defs) => defs.0.clone(),
        None => {
            let parsed = parse_enemies(&enemy_doc.0).unwrap_or_else(|e| panic!("{e}"));
            commands.insert_resource(EnemyDefs(parsed.clone()));
            parsed
        }
    };
    let id = pending.id.as_str();
    let data: MapData = serde_json::from_str(&map_doc.0).unwrap_or_else(|e| panic!("map {id}: {e}"));
    let map = TileMap::new(&data).unwrap_or_else(|e| panic!("map {id}: {e}"));
    let theme = TerrainTheme::get(&data.theme);

    let started = Instant::now();
    let baked = baker::bake(&map, &theme, data.lava);
    let bake_ms = started.elapsed().as_secs_f32() * 1000.0;
    info!("baked {id} ({}x{} px) in {bake_ms:.1} ms", baked.width, baked.height);

    spawn_terrain(&mut commands, &mut images, &baked);
    spawn_water(&mut commands, &mut images, &mut layouts, &map, &theme);

    let shadow = images.add(image(&art::shadow(12, 5)));
    let spawn = data
        .entities
        .iter()
        .find(|e| e.kind == "spawn")
        .map(|e| tile_feet(e.x, e.y))
        .unwrap_or(Vec2::new(map.pixel_width() as f32 / 2.0, map.pixel_height() as f32 / 2.0));
    spawn_hero(&mut commands, &mut images, &mut layouts, spawn, &shadow);

    let monster_layout = layouts.add(TextureAtlasLayout::from_grid(UVec2::splat(16), 2, 1, None, None));
    for entity in data.entities.iter().filter(|e| e.kind == "enemy") {
        let Some(def) = entity.str("kind").and_then(|k| defs.get(k)) else {
            continue;
        };
        let feet = tile_feet(entity.x, entity.y);
        let blocked = |x: f32, y: f32| map.at((x / TILE as f32) as i32, (y / TILE as f32) as i32);
        if !def.flying && blocked(feet.x, feet.y - 1.0).blocks_walking() {
            continue;
        }
        let sheet = assets.load(format!("external/Monsters/{}.png", def.sprite));
        commands.spawn((
            LevelEntity,
            Enemy::new(def),
            Body { feet, width: def.size[0], height: def.size[1] },
            SpriteOffset(8.0),
            Sprite::from_atlas_image(sheet, TextureAtlas { layout: monster_layout.clone(), index: 0 }),
            children![shadow_sprite(&shadow, 8.0)],
        ));
    }

    let max = Vec2::new(map.pixel_width() as f32, map.pixel_height() as f32);
    camera.snap(spawn, max);
    commands.insert_resource(Level { id: id.into(), name: data.name.clone(), map, spawn, bake_ms });
    next.set(AppState::Playing);
}

/// Map entities are placed by tile; actors stand near the bottom of their tile.
fn tile_feet(x: f32, y: f32) -> Vec2 {
    Vec2::new(x * TILE as f32 + 8.0, y * TILE as f32 + 14.0)
}

pub fn shadow_sprite(shadow: &Handle<Image>, offset: f32) -> impl Bundle {
    (Sprite::from_image(shadow.clone()), Transform::from_xyz(0.0, -offset + 1.0, -0.5))
}

fn spawn_terrain(commands: &mut Commands, images: &mut Assets<Image>, baked: &baker::Baked) {
    for cy in (0..baked.height).step_by(CHUNK as usize) {
        for cx in (0..baked.width).step_by(CHUNK as usize) {
            let (cw, ch) = (CHUNK.min(baked.width - cx), CHUNK.min(baked.height - cy));
            let mut chunk = Canvas::new(cw, ch);
            for y in 0..ch {
                let row = ((cy + y) * baked.width + cx) as usize;
                chunk.pixels[(y * cw) as usize..((y + 1) * cw) as usize]
                    .copy_from_slice(&baked.pixels[row..row + cw as usize]);
            }
            let center = Vec2::new((cx + cw / 2) as f32, (cy + ch / 2) as f32);
            commands.spawn((
                LevelEntity,
                Sprite::from_image(images.add(image(&chunk))),
                Transform::from_xyz(center.x, -center.y, 1.0),
            ));
        }
    }
}

fn spawn_water(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    layouts: &mut Assets<TextureAtlasLayout>,
    map: &TileMap,
    theme: &TerrainTheme,
) {
    let (w, h, pixels) = baker::water_frames(theme.ramp(Terrain::Water));
    let frames = images.add(image(&Canvas { width: w, height: h, pixels }));
    let layout = layouts.add(TextureAtlasLayout::from_grid(UVec2::splat(TILE as u32), 4, 1, None, None));
    for y in 0..map.height {
        for x in 0..map.width {
            let near_water = (-1..=1).any(|oy| (-1..=1).any(|ox| map.at(x + ox, y + oy) == Terrain::Water));
            if !near_water {
                continue;
            }
            // Per-cell phase and mirroring break up the tiled pattern.
            let hash = noise::hash(x, y, 17);
            let mut sprite = Sprite::from_atlas_image(
                frames.clone(),
                TextureAtlas { layout: layout.clone(), index: (hash % 4) as usize },
            );
            sprite.flip_x = hash & 16 != 0;
            let center = Vec2::new((x * TILE + 8) as f32, (y * TILE + 8) as f32);
            commands.spawn((LevelEntity, WaterTile(hash % 4), sprite, Transform::from_xyz(center.x, -center.y, 0.0)));
        }
    }
}

fn spawn_hero(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    layouts: &mut Assets<TextureAtlasLayout>,
    feet: Vec2,
    shadow: &Handle<Image>,
) {
    let sheet = images.add(image(&PersonSprite::build(&Appearance::hero())));
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(PersonSprite::FRAME_WIDTH as u32, PersonSprite::FRAME_HEIGHT as u32),
        PersonSprite::columns() as u32,
        3,
        None,
        None,
    ));
    // The rig's feet sit one pixel above the frame's bottom edge.
    let offset = PersonSprite::FRAME_HEIGHT as f32 / 2.0 - 1.0;
    commands.spawn((
        LevelEntity,
        Player::default(),
        Facing::Down,
        Body { feet, width: 10.0, height: 8.0 },
        SpriteOffset(offset),
        Sprite::from_atlas_image(sheet, TextureAtlas { layout, index: 0 }),
        children![shadow_sprite(shadow, offset)],
    ));
}

pub fn image(canvas: &Canvas) -> Image {
    Image::new(
        Extent3d { width: canvas.width as u32, height: canvas.height as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        canvas.to_bytes(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
