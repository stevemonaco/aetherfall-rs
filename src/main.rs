//! Aetherfall on Bevy: a spike porting the terrain baker, procedural hero art, movement and basic monsters from the
//! C# / MonoGame game, built for both native and the web.

mod actors;
mod art;
mod baker;
mod color;
mod data;
mod dev;
mod level;
mod map;
mod noise;
mod theme;

use bevy::asset::AssetMetaCheck;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::{PrimaryWindow, WindowResolution};

use crate::actors::{HERO_LIFE, Player};
use crate::data::{JsonDoc, JsonDocLoader};
use crate::level::{LevelEntity, Pending, WaterTile};

/// The virtual screen the game is drawn at, then scaled up by a whole number to fit the window.
pub const SCREEN: UVec2 = UVec2::new(480, 270);

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum AppState {
    #[default]
    Loading,
    Playing,
}

/// The world camera's eased position (top-left of the view, in world pixels).
#[derive(Resource, Default)]
pub struct CameraState {
    exact: Vec2,
}

impl CameraState {
    fn target(feet: Vec2, world: Vec2) -> Vec2 {
        let view = SCREEN.as_vec2();
        let desired = feet - view / 2.0;
        let fit = |d: f32, w: f32, v: f32| if w <= v { (w - v) / 2.0 } else { d.clamp(0.0, w - v) };
        Vec2::new(fit(desired.x, world.x, view.x), fit(desired.y, world.y, view.y))
    }

    pub fn snap(&mut self, feet: Vec2, world: Vec2) {
        self.exact = Self::target(feet, world);
    }
}

#[derive(Component)]
struct WorldCamera;

#[derive(Component)]
struct ScreenCamera;

#[derive(Component)]
struct Hud;

fn main() {
    let options = dev::LaunchOptions::parse();
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                // There are no .meta files; skipping the lookup avoids a 404 per asset on the web.
                .set(AssetPlugin { meta_check: AssetMetaCheck::Never, ..default() })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Aetherfall".into(),
                        resolution: WindowResolution::new(SCREEN.x * 3, SCREEN.y * 3),
                        canvas: Some("#aetherfall".into()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                }),
            FrameTimeDiagnosticsPlugin::default(),
            dev::DevPlugin(options),
        ))
        .init_asset::<JsonDoc>()
        .init_asset_loader::<JsonDocLoader>()
        .init_state::<AppState>()
        .init_resource::<CameraState>()
        .add_systems(Startup, setup)
        .add_systems(Update, level::finish_loading.run_if(in_state(AppState::Loading)))
        .add_systems(
            Update,
            (
                actors::control_player,
                actors::update_enemies,
                actors::animate_player,
                actors::sync_sprites,
                follow_camera,
                animate_water,
                next_map,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(Update, (fit_screen, update_hud))
        .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    options: Res<dev::LaunchOptions>,
) {
    // The world renders into a 480x270 image; a second camera shows that image scaled by a whole number.
    let screen = images.add(Image::new_target_texture(SCREEN.x, SCREEN.y, TextureFormat::Rgba8UnormSrgb, None));
    commands.spawn((
        WorldCamera,
        Camera2d,
        Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::srgb_u8(8, 7, 14)), ..default() },
        RenderTarget::Image(screen.clone().into()),
        Msaa::Off,
    ));
    commands.spawn((Sprite::from_image(screen), RenderLayers::layer(1)));
    commands.spawn((
        ScreenCamera,
        Camera2d,
        Camera { clear_color: ClearColorConfig::Custom(Color::srgb_u8(8, 7, 14)), ..default() },
        Msaa::Off,
        RenderLayers::layer(1),
        IsDefaultUiCamera,
    ));
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::srgb_u8(244, 240, 224)),
        Node { position_type: PositionType::Absolute, top: px(6), left: px(8), ..default() },
    ));

    let enemies = assets.load("data/enemies.json");
    let start = options.map.clone().unwrap_or_else(|| level::SHOWCASE[0].into());
    commands.insert_resource(level::request_map(0, &start, &assets, enemies));
}

fn follow_camera(
    time: Res<Time>,
    level: Res<level::Level>,
    mut state: ResMut<CameraState>,
    player: Single<&actors::Body, With<Player>>,
    mut camera: Single<&mut Transform, With<WorldCamera>>,
) {
    let world = Vec2::new(level.map.pixel_width() as f32, level.map.pixel_height() as f32);
    let desired = CameraState::target(player.feet, world);
    state.exact = state.exact.lerp(desired, 1.0 - (-time.delta_secs() * 9.0).exp());
    if state.exact.distance_squared(desired) < 0.25 {
        state.exact = desired;
    }
    let center = state.exact.round() + SCREEN.as_vec2() / 2.0;
    camera.translation = Vec3::new(center.x, -center.y, camera.translation.z);
}

fn animate_water(time: Res<Time>, mut tiles: Query<(&WaterTile, &mut Sprite)>) {
    let tick = (time.elapsed_secs() * 3.0) as u32;
    for (tile, mut sprite) in &mut tiles {
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = ((tick + tile.0) % 4) as usize;
        }
    }
}

fn next_map(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    assets: Res<AssetServer>,
    pending: Res<Pending>,
    level_entities: Query<Entity, With<LevelEntity>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let step = if keys.just_pressed(KeyCode::KeyN) {
        1
    } else if keys.just_pressed(KeyCode::KeyP) {
        level::SHOWCASE.len() - 1
    } else {
        return;
    };
    for entity in &level_entities {
        commands.entity(entity).despawn();
    }
    let index = (pending.index + step) % level::SHOWCASE.len();
    commands.insert_resource(level::request_map(index, level::SHOWCASE[index], &assets, pending.enemies.clone()));
    next.set(AppState::Loading);
}

/// Scales the 480x270 screen by the largest whole number of physical pixels that fits the window.
fn fit_screen(window: Single<&Window, With<PrimaryWindow>>, mut camera: Single<&mut Projection, With<ScreenCamera>>) {
    let (w, h) = (window.physical_width() as f32, window.physical_height() as f32);
    let scale = (w / SCREEN.x as f32).min(h / SCREEN.y as f32).floor().max(1.0);
    if let Projection::Orthographic(ortho) = &mut **camera {
        let wanted = window.scale_factor() / scale;
        if ortho.scale != wanted {
            ortho.scale = wanted;
        }
    }
}

fn update_hud(
    diagnostics: Res<DiagnosticsStore>,
    level: Option<Res<level::Level>>,
    player: Query<&Player>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let Some(level) = level else {
        hud.0 = "Loading...".into();
        return;
    };
    let life = player.iter().next().map(|p| p.life).unwrap_or(HERO_LIFE);
    hud.0 = format!(
        "{} ({})   bake {:.0} ms   {:.0} fps   life {}/{}\nMove: arrows/WASD   Attack: J/Z   Next/prev map: N/P",
        level.name, level.id, level.bake_ms, fps, life, HERO_LIFE
    );
}
