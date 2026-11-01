//! The Bevy side of the game: window, asset loading, turning the frame's draw list into GPU sprites, device input,
//! audio playback, the light map, and the command-line test switches. Game code never touches Bevy directly.

use std::collections::HashMap;

use bevy::app::AppExit;
use bevy::asset::{AssetMetaCheck, RenderAssetUsages};
use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackMode, PlaybackSettings, Volume};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::input::mouse::{MouseButton, MouseWheel};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d, RenderPipelineDescriptor,
    SpecializedMeshPipelineError, TextureDimension, TextureFormat,
};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline, Material2dPlugin};
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode, WindowResolution};

use crate::core::audio::{self, Sound};
use crate::core::canvas::Canvas;
use crate::core::color::Rgba;
use crate::core::gfx::{self, Batch, TexId};
use crate::core::input::{self, ACTION_COUNT, GameAction, RawInput};
use crate::core::settings;
use crate::entities::boss;
use crate::gameplay::element::{Element, MODES};
use crate::gameplay::game_data::{IMAGE_FILES, data};
use crate::gameplay::game_state::{self, GameState};
use crate::gameplay::{mastery, skills, supports, weave};
use crate::scenes::restore::RestoreScene;
use crate::scenes::scene::SceneManager;
use crate::scenes::title::TitleScene;
use crate::scenes::world::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::scenes::world_scene::WorldScene;
use crate::world::lighting;
use crate::world::map::tile_feet;

const STEP: f32 = 1.0 / 60.0;
const LIGHT_SCALE: i32 = 4;
const LIGHT_Z: f32 = 500.0;
const HUD_Z: f32 = 600.0;

/// Command-line switches for jumping into a map and testing headlessly. Test runs never write saves or settings.
#[derive(Clone, Default, Resource)]
pub struct LaunchOptions {
    pub map: Option<String>,
    pub at: Option<(i32, i32)>,
    pub give: Vec<String>,
    pub level: Option<i32>,
    /// Sets every mode's mastery to this level.
    pub mastery: Option<i32>,
    /// Learns every skill and unlocks all three levels.
    pub all_skills: bool,
    /// Takes every node of the Weave.
    pub all_tree: bool,
    /// Gives every support gem and fills each open socket with the first that fits.
    pub all_supports: bool,
    pub flags: Vec<String>,
    pub script: Option<String>,
    pub fuzz: Option<u64>,
    pub shot: Option<String>,
    pub frames: u32,
    pub mute: bool,
    pub boss_hp: f32,
    pub dump: bool,
    /// Plays a town's restoration cutscene at once: `town,flag`.
    pub restore: Option<(String, String)>,
}

impl LaunchOptions {
    #[cfg(target_arch = "wasm32")]
    pub fn parse() -> LaunchOptions {
        LaunchOptions { frames: 120, boss_hp: 1.0, ..LaunchOptions::default() }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn parse() -> LaunchOptions {
        let mut options = LaunchOptions { frames: 120, boss_hp: 1.0, ..LaunchOptions::default() };
        {
            let args: Vec<String> = std::env::args().collect();
            let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
            let list = |name: &str| value(name).map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default();
            options.map = value("--map");
            options.at = value("--at")
                .and_then(|v| v.split_once(',').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))));
            options.give = list("--give");
            options.level = value("--level").and_then(|v| v.parse().ok());
            options.mastery = value("--mastery").and_then(|v| v.parse().ok());
            options.all_skills = value("--skills").as_deref() == Some("all");
            options.all_tree = value("--weave").as_deref() == Some("all") || value("--tree").as_deref() == Some("all");
            options.all_supports = value("--supports").as_deref() == Some("all");
            options.flags = list("--flags");
            options.script = value("--script");
            options.fuzz = value("--fuzz").and_then(|v| v.parse().ok());
            options.shot = value("--shot");
            options.frames = value("--frames").and_then(|v| v.parse().ok()).unwrap_or(120);
            options.mute = args.iter().any(|a| a == "--mute");
            options.boss_hp = value("--boss-hp").and_then(|v| v.parse().ok()).unwrap_or(1.0);
            options.dump = args.iter().any(|a| a == "--dump");
            options.restore =
                value("--restore").and_then(|v| v.split_once(',').map(|(t, f)| (t.to_string(), f.to_string())));
        }
        options
    }

    fn testing(&self) -> bool {
        self.shot.is_some() || self.fuzz.is_some() || self.script.is_some()
    }
}

#[derive(States, Default, Clone, Copy, Eq, PartialEq, Hash, Debug)]
enum Phase {
    #[default]
    Loading,
    Running,
}

/// The game itself, owned by the main thread.
struct Game {
    scenes: SceneManager,
    batch: Batch,
    accumulator: f32,
    updates: u32,
    shot_taken: bool,
}

#[derive(Resource, Default)]
struct Textures {
    handles: Vec<Option<Handle<Image>>>,
}

#[derive(Resource)]
struct Loading {
    images: Vec<(&'static str, Handle<Image>)>,
}

#[derive(Resource, Default)]
struct SpritePool {
    sprites: Vec<Entity>,
}

#[derive(Resource)]
struct Sounds {
    effects: HashMap<Sound, Handle<AudioSource>>,
}

#[derive(Component)]
struct MusicPlayer {
    track: String,
}

#[derive(Component)]
struct WorldCamera;

#[derive(Component)]
struct ScreenCamera;

#[derive(Component)]
struct LightOverlay;

#[derive(Resource)]
struct LightMap {
    image: Handle<Image>,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct LightMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
}

impl Material2d for LightMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/light.wesl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        _pipeline: &Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let multiply = BlendComponent {
            src_factor: BlendFactor::Dst,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        };
        let keep_alpha = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState { color: multiply, alpha: keep_alpha });
            }
        }
        Ok(())
    }
}

pub fn run() {
    let options = LaunchOptions::parse();
    if options.testing() {
        game_state::SAVES_DISABLED.store(true, std::sync::atomic::Ordering::Relaxed);
        settings::SAVES_DISABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    boss::DEBUG_HP_SCALE.store(options.boss_hp.to_bits(), std::sync::atomic::Ordering::Relaxed);
    settings::load();
    audio::set_muted(options.mute);
    if let Some(script) = &options.script {
        input::load_script(script);
    } else if let Some(seed) = options.fuzz {
        input::load_fuzz(seed, options.frames as i32);
    }

    let fullscreen = settings::current().fullscreen && options.shot.is_none();
    let mut app = App::new();
    #[cfg(feature = "packed-assets")]
    crate::packed_assets::register(&mut app);
    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(AssetPlugin { meta_check: AssetMetaCheck::Never, ..default() })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Aetherfall".into(),
                    resolution: WindowResolution::new(SCREEN_WIDTH as u32 * 3, SCREEN_HEIGHT as u32 * 3),
                    canvas: Some("#aetherfall".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    mode: if fullscreen {
                        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                    } else {
                        WindowMode::Windowed
                    },
                    ..default()
                }),
                ..default()
            }),
        Material2dPlugin::<LightMaterial>::default(),
    ))
    .insert_resource(options)
    .insert_resource(ClearColor(Color::BLACK))
    .init_state::<Phase>()
    .init_resource::<Textures>()
    .init_resource::<SpritePool>()
    .add_systems(Startup, setup)
    .add_systems(Update, finish_loading.run_if(in_state(Phase::Loading)))
    .add_systems(
        Update,
        (run_game, sync_textures, draw_frame, play_audio, update_lights, take_shot)
            .chain()
            .run_if(in_state(Phase::Running)),
    )
    .add_systems(Update, (fit_screen, window_keys))
    .run();
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut sounds: ResMut<Assets<AudioSource>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LightMaterial>>,
) {
    let screen = images.add(Image::new_target_texture(
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        WorldCamera,
        Camera2d,
        Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        RenderTarget::Image(screen.clone().into()),
        Msaa::Off,
    ));
    commands.spawn((Sprite::from_image(screen), RenderLayers::layer(1)));
    commands.spawn((
        ScreenCamera,
        Camera2d,
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Msaa::Off,
        RenderLayers::layer(1),
    ));

    let (lw, lh) = (SCREEN_WIDTH / LIGHT_SCALE, SCREEN_HEIGHT / LIGHT_SCALE + 1);
    let mut light = Image::new(
        Extent3d { width: lw as u32, height: lh as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![255; (lw * lh * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    light.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    let light = images.add(light);
    commands.insert_resource(LightMap { image: light.clone() });
    commands.spawn((
        LightOverlay,
        Mesh2d(meshes.add(Rectangle::new(SCREEN_WIDTH as f32, (lh * LIGHT_SCALE) as f32))),
        MeshMaterial2d(materials.add(LightMaterial { texture: light })),
        Transform::from_xyz(0.0, -((lh * LIGHT_SCALE - SCREEN_HEIGHT) as f32) / 2.0, LIGHT_Z),
        Visibility::Hidden,
    ));

    let effects =
        Sound::ALL.iter().map(|&s| (s, sounds.add(AudioSource { bytes: audio::synthesize(s).into() }))).collect();
    commands.insert_resource(Sounds { effects });

    let images = IMAGE_FILES.iter().map(|&path| (path, assets.load::<Image>(path))).collect();
    commands.insert_resource(Loading { images });
    let _ = data();
}

fn finish_loading(world: &mut bevy::prelude::World) {
    let loading_done = {
        let loading = world.resource::<Loading>();
        let server = world.resource::<AssetServer>();
        loading.images.iter().all(|(_, h)| {
            matches!(server.load_state(h.id()), bevy::asset::LoadState::Loaded | bevy::asset::LoadState::Failed(_))
        })
    };
    if !loading_done {
        return;
    }
    let loading = world.remove_resource::<Loading>().expect("loading");
    let mut registered = Vec::new();
    {
        let images = world.resource::<Assets<Image>>();
        for (path, handle) in &loading.images {
            let Some(image) = images.get(handle) else { continue };
            let size = image.size();
            let Some(bytes) = image.data.as_ref() else { continue };
            if bytes.len() < (size.x * size.y * 4) as usize {
                continue;
            }
            let pixels = bytes.as_chunks::<4>().0.iter().map(|p| Rgba::new(p[0], p[1], p[2], p[3])).collect();
            let id = gfx::register_loaded(path, size.x as i32, size.y as i32, pixels);
            registered.push((id, handle.clone()));
        }
    }
    {
        let mut textures = world.resource_mut::<Textures>();
        for (id, handle) in registered {
            set_handle(&mut textures, id, handle);
        }
    }

    let options = world.resource::<LaunchOptions>().clone();
    let mut scenes = SceneManager::default();
    if let Some(map) = &options.map {
        let mut state = GameState::new_game(1);
        state.map_id = map.clone();
        state.set_position(options.at.map(|(x, y)| tile_feet(x, y)));
        if let Some(level) = options.level {
            state.debug_set_level(level);
        }
        for item in &options.give {
            state.give(item, 1);
        }
        for flag in &options.flags {
            match flag.strip_prefix("pact:") {
                Some(pact) => state.pacts.insert(pact.to_string()),
                None => state.flags.insert(flag.clone()),
            };
        }
        if let Some(level) = options.mastery {
            for mode in MODES.into_iter().filter(|&m| mastery::trains(m)) {
                state.mastery.insert(mode.key().to_string(), mastery::xp_for(level));
            }
            state.sync_skills();
        }
        if options.all_skills {
            for skill in skills::ALL {
                state.learn(skill.id);
            }
            state.flags.insert("debug:skills".into());
        }
        if options.all_supports {
            for gem in supports::ALL {
                state.give(gem.id, 1);
            }
            for mode in state.blade_pages() {
                for &group in GameState::socket_groups(mode) {
                    for index in 0..state.socket_count(mode) {
                        let free = supports::ALL.iter().find(|g| {
                            g.fits_group(group)
                                && g.prism == (mode == Element::Aether)
                                && !state.sockets.values().any(|s| s == g.id)
                        });
                        if let Some(gem) = free {
                            state.set_socket(&GameState::socket_key(mode, group, index), Some(gem.id));
                        }
                    }
                }
            }
        }
        if options.all_tree {
            state.inventory.insert("insight_tome".into(), 200);
            state.inventory.insert("weave_knot".into(), 20);
            for node in weave::NODES.iter() {
                state.take_node(&node.id);
            }
        }
        state.sp = state.max_sp();
        let restore =
            options.restore.clone().map(|(town, flag)| RestoreScene::new(state.clone(), &town, &flag, "(test)"));
        scenes.push(Box::new(WorldScene::new(state, false)));
        if let Some(scene) = restore {
            scenes.push(Box::new(scene));
        }
    } else {
        scenes.push(Box::new(TitleScene::new()));
    }
    world.insert_non_send(Game { scenes, batch: Batch::default(), accumulator: 0.0, updates: 0, shot_taken: false });
    world.resource_mut::<NextState<Phase>>().set(Phase::Running);
}

fn set_handle(textures: &mut Textures, id: TexId, handle: Handle<Image>) {
    let index = id.0 as usize;
    if textures.handles.len() <= index {
        textures.handles.resize(index + 1, None);
    }
    textures.handles[index] = Some(handle);
}

#[allow(clippy::too_many_arguments)]
fn gather_input(
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    wheel: &mut MessageReader<MouseWheel>,
    gamepads: &Query<&Gamepad>,
    window: &Window,
) -> RawInput {
    use GameAction::*;
    let key_map: [(GameAction, &[KeyCode]); ACTION_COUNT] = [
        (Up, &[KeyCode::ArrowUp, KeyCode::KeyW]),
        (Down, &[KeyCode::ArrowDown, KeyCode::KeyS]),
        (Left, &[KeyCode::ArrowLeft, KeyCode::KeyA]),
        (Right, &[KeyCode::ArrowRight, KeyCode::KeyD]),
        (Attack, &[KeyCode::KeyJ, KeyCode::KeyZ]),
        (Throw, &[KeyCode::KeyK, KeyCode::KeyX]),
        (Dodge, &[KeyCode::Space]),
        (Guard, &[KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        (Jump, &[KeyCode::KeyC, KeyCode::KeyL]),
        (Tether, &[KeyCode::KeyF, KeyCode::KeyI]),
        (Skill, &[KeyCode::KeyR, KeyCode::KeyU]),
        (PrevMode, &[KeyCode::KeyQ]),
        (NextMode, &[KeyCode::KeyE]),
        (Mode1, &[KeyCode::Digit1]),
        (Mode2, &[KeyCode::Digit2]),
        (Mode3, &[KeyCode::Digit3]),
        (Mode4, &[KeyCode::Digit4]),
        (Confirm, &[KeyCode::KeyJ, KeyCode::KeyZ, KeyCode::Space, KeyCode::Enter]),
        (Cancel, &[KeyCode::KeyK, KeyCode::KeyX, KeyCode::Escape, KeyCode::Backspace]),
        (Menu, &[KeyCode::Enter, KeyCode::Escape, KeyCode::Tab]),
        (Map, &[KeyCode::KeyM]),
        (Soul, &[KeyCode::KeyV]),
    ];
    let pad_map: [(GameAction, &[GamepadButton]); 16] = [
        (Up, &[GamepadButton::DPadUp]),
        (Down, &[GamepadButton::DPadDown]),
        (Left, &[GamepadButton::DPadLeft]),
        (Right, &[GamepadButton::DPadRight]),
        (Attack, &[GamepadButton::South]),
        (Throw, &[GamepadButton::RightTrigger2]),
        (Dodge, &[GamepadButton::East]),
        (Guard, &[GamepadButton::LeftTrigger2]),
        (Jump, &[GamepadButton::North]),
        (Tether, &[GamepadButton::West]),
        (Skill, &[GamepadButton::RightTrigger]),
        (NextMode, &[GamepadButton::LeftTrigger]),
        (Confirm, &[GamepadButton::South]),
        (Cancel, &[GamepadButton::East]),
        (Menu, &[GamepadButton::Start]),
        (Map, &[GamepadButton::Select]),
    ];
    let mut raw = RawInput { window_active: window.focused, ..RawInput::default() };
    for (action, codes) in key_map {
        raw.actions[action as usize] = codes.iter().any(|k| keys.pressed(*k));
    }
    raw.keyboard_used = keys.get_pressed().next().is_some();
    raw.keys_pressed = keys.get_just_pressed().map(|k| format!("{k:?}")).collect();

    if let Some(pad) = gamepads.iter().next() {
        raw.gamepad_connected = true;
        for (action, buttons) in pad_map {
            if buttons.iter().any(|b| pad.pressed(*b)) {
                raw.actions[action as usize] = true;
            }
        }
        if pad.pressed(GamepadButton::RightThumb) {
            raw.actions[Soul as usize] = true;
        }
        let left = pad.left_stick();
        let right = pad.right_stick();
        raw.left_stick = Vec2::new(left.x, -left.y);
        raw.right_stick = Vec2::new(right.x, -right.y);
        if left.y > 0.5 {
            raw.actions[Up as usize] = true;
        }
        if left.y < -0.5 {
            raw.actions[Down as usize] = true;
        }
        if left.x < -0.5 {
            raw.actions[Left as usize] = true;
        }
        if left.x > 0.5 {
            raw.actions[Right as usize] = true;
        }
        raw.gamepad_used = pad.get_pressed().next().is_some() || left.length_squared() > 0.09;
    }

    if window.focused {
        raw.actions[Throw as usize] |= mouse.pressed(MouseButton::Left);
        raw.actions[Attack as usize] |= mouse.pressed(MouseButton::Right);
        raw.actions[Skill as usize] |= mouse.pressed(MouseButton::Middle);
        raw.mouse_buttons = mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Right);
        let steps: f32 = wheel.read().map(|w| w.y).sum();
        raw.wheel = if steps > 0.0 {
            -1
        } else if steps < 0.0 {
            1
        } else {
            0
        };
    }
    if let Some(cursor) = window.cursor_position() {
        let factor = window.scale_factor();
        let (pw, ph) = (window.physical_width() as f32, window.physical_height() as f32);
        let scale = (pw / SCREEN_WIDTH as f32).min(ph / SCREEN_HEIGHT as f32).floor().max(1.0) / factor;
        let origin =
            Vec2::new(window.width() - SCREEN_WIDTH as f32 * scale, window.height() - SCREEN_HEIGHT as f32 * scale)
                / 2.0;
        raw.mouse_screen = Some((cursor - origin) / scale);
    }
    raw
}

#[allow(clippy::too_many_arguments)]
fn run_game(
    mut game: NonSendMut<Game>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    mut cursor_moved: MessageReader<CursorMoved>,
    gamepads: Query<&Gamepad>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut raw = gather_input(&keys, &mouse, &mut wheel, &gamepads, &window);
    raw.mouse_moved = cursor_moved.read().count() > 0;
    let testing = input::scripted();
    game.accumulator += if testing { STEP } else { time.delta_secs().min(0.25) };
    let mut steps = 0;
    while game.accumulator >= STEP && steps < 4 {
        game.accumulator -= STEP;
        input::update(if steps == 0 {
            raw.clone()
        } else {
            RawInput { mouse_moved: false, keys_pressed: Vec::new(), wheel: 0, ..raw.clone() }
        });
        game.scenes.update(STEP);
        game.updates += 1;
        steps += 1;
    }
    if game.scenes.is_empty() {
        exit.write(AppExit::Success);
    }
    let Game { scenes, batch, .. } = &mut *game;
    batch.clear();
    scenes.draw(batch);
}

fn sync_textures(mut textures: ResMut<Textures>, mut images: ResMut<Assets<Image>>) {
    for (id, canvas) in gfx::take_uploads() {
        let handle = images.add(canvas_image(&canvas));
        set_handle(&mut textures, id, handle);
    }
}

fn canvas_image(canvas: &Canvas) -> Image {
    Image::new(
        Extent3d { width: canvas.width.max(1) as u32, height: canvas.height.max(1) as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        if canvas.width > 0 && canvas.height > 0 { canvas.to_bytes() } else { vec![0; 4] },
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Turns the frame's draw commands into sprites, in order, reusing a pool of sprite entities.
fn draw_frame(
    mut commands: Commands,
    game: NonSend<Game>,
    textures: Res<Textures>,
    mut pool: ResMut<SpritePool>,
    mut sprites: Query<(&mut Sprite, &mut Transform, &mut Visibility)>,
) {
    let cmds = &game.batch.cmds;
    let light_mark = game.batch.light_mark().unwrap_or(cmds.len());
    while pool.sprites.len() < cmds.len() {
        let entity = commands.spawn((Sprite::default(), Transform::default(), Visibility::Hidden)).id();
        pool.sprites.push(entity);
    }
    let half = Vec2::new(SCREEN_WIDTH as f32 / 2.0, SCREEN_HEIGHT as f32 / 2.0);
    for (i, &entity) in pool.sprites.iter().enumerate() {
        let Ok((mut sprite, mut transform, mut visibility)) = sprites.get_mut(entity) else { continue };
        let Some(cmd) = cmds.get(i) else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        let Some(Some(handle)) = textures.handles.get(cmd.tex.0 as usize) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let (w, h) = (cmd.src.w as f32, cmd.src.h as f32);
        let local = (Vec2::new(w / 2.0, h / 2.0) - cmd.origin) * cmd.scale;
        let center = cmd.pos + crate::core::geom::rotate(local, cmd.rotation);
        let z = if i < light_mark { i as f32 * 0.001 } else { HUD_Z + (i - light_mark) as f32 * 0.001 };
        transform.translation = Vec3::new(center.x - half.x, half.y - center.y, z);
        transform.rotation = Quat::from_rotation_z(-cmd.rotation);
        sprite.image = handle.clone();
        sprite.rect = Some(Rect::new(
            cmd.src.x as f32,
            cmd.src.y as f32,
            (cmd.src.x + cmd.src.w) as f32,
            (cmd.src.y + cmd.src.h) as f32,
        ));
        sprite.custom_size = Some(Vec2::new(w * cmd.scale.x.abs(), h * cmd.scale.y.abs()));
        sprite.flip_x = cmd.flip_x;
        sprite.flip_y = cmd.flip_y;
        let c = cmd.color;
        sprite.color = Color::srgba_u8(c.r, c.g, c.b, c.a);
        *visibility = Visibility::Inherited;
    }
}

/// Recomputes the light map (a quarter-resolution image multiplied over the world) from what the world published.
fn update_lights(
    light_map: Res<LightMap>,
    mut images: ResMut<Assets<Image>>,
    mut overlay: Single<&mut Visibility, With<LightOverlay>>,
) {
    let frame = lighting::current();
    let Some(ambient) = frame.ambient else {
        **overlay = Visibility::Hidden;
        return;
    };
    **overlay = Visibility::Inherited;
    let Some(mut image) = images.get_mut(&light_map.image) else { return };
    let size = image.size();
    let (lw, lh) = (size.x as i32, size.y as i32);
    let base = [ambient.r as f32 / 255.0, ambient.g as f32 / 255.0, ambient.b as f32 / 255.0];
    let mut buffer = vec![base; (lw * lh) as usize];
    for (at, radius, color, intensity) in &frame.lights {
        let tint = [
            color.r as f32 / 255.0 * intensity,
            color.g as f32 / 255.0 * intensity,
            color.b as f32 / 255.0 * intensity,
        ];
        let (x0, x1) = (
            ((at.x - radius) / LIGHT_SCALE as f32).floor() as i32,
            ((at.x + radius) / LIGHT_SCALE as f32).ceil() as i32,
        );
        let (y0, y1) = (
            ((at.y - radius) / LIGHT_SCALE as f32).floor() as i32,
            ((at.y + radius) / LIGHT_SCALE as f32).ceil() as i32,
        );
        for y in y0.max(0)..=y1.min(lh - 1) {
            for x in x0.max(0)..=x1.min(lw - 1) {
                let p = Vec2::new((x * LIGHT_SCALE) as f32 + 2.0, (y * LIGHT_SCALE) as f32 + 2.0);
                let d = p.distance(*at) / radius;
                if d >= 1.0 {
                    continue;
                }
                let falloff = (1.0 - d) * (1.0 - d);
                let cell = &mut buffer[(y * lw + x) as usize];
                for (c, t) in cell.iter_mut().zip(tint) {
                    *c += t * falloff;
                }
            }
        }
    }
    if let Some(data) = image.data.as_mut() {
        for (i, cell) in buffer.iter().enumerate() {
            data[i * 4] = (cell[0].min(1.0) * 255.0) as u8;
            data[i * 4 + 1] = (cell[1].min(1.0) * 255.0) as u8;
            data[i * 4 + 2] = (cell[2].min(1.0) * 255.0) as u8;
            data[i * 4 + 3] = 255;
        }
    }
}

fn play_audio(
    mut commands: Commands,
    sounds: Res<Sounds>,
    assets: Res<AssetServer>,
    time: Res<Time>,
    mut music: Query<(Entity, &MusicPlayer, Option<&mut AudioSink>)>,
) {
    for request in audio::take_requests() {
        if let Some(handle) = sounds.effects.get(&request.sound) {
            let settings = PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(request.volume),
                speed: 2f32.powf(request.pitch),
                ..PlaybackSettings::DESPAWN
            };
            commands.spawn((AudioPlayer::new(handle.clone()), settings));
        }
    }
    let state = audio::update_music(time.delta_secs());
    let mut playing = false;
    for (entity, player, sink) in &mut music {
        if Some(&player.track) == state.track.as_ref() {
            playing = true;
            if let Some(mut sink) = sink {
                sink.set_volume(Volume::Linear(state.volume));
            }
        } else {
            commands.entity(entity).despawn();
        }
    }
    if !playing && let Some(track) = state.track {
        let path = format!("external/Music/{track}.ogg");
        commands.spawn((
            MusicPlayer { track },
            AudioPlayer::<AudioSource>(assets.load(path)),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(state.volume),
                ..PlaybackSettings::LOOP
            },
        ));
    }
}

/// Scales the 480x270 screen by the largest whole number of physical pixels that fits the window.
fn fit_screen(window: Single<&Window, With<PrimaryWindow>>, mut camera: Single<&mut Projection, With<ScreenCamera>>) {
    let (w, h) = (window.physical_width() as f32, window.physical_height() as f32);
    let scale = (w / SCREEN_WIDTH as f32).min(h / SCREEN_HEIGHT as f32).floor().max(1.0);
    if let Projection::Orthographic(ortho) = &mut **camera {
        let wanted = window.scale_factor() / scale;
        if ortho.scale != wanted {
            ortho.scale = wanted;
        }
    }
}

fn take_shot(
    mut commands: Commands,
    options: Res<LaunchOptions>,
    mut game: NonSendMut<Game>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(path) = options.shot.clone() else { return };
    if !game.shot_taken && game.updates >= options.frames {
        game.shot_taken = true;
        if options.dump
            && let Some(world) = game.scenes.world()
        {
            let s = &world.state;
            println!(
                "DUMP map={} pos={:?} hp={}/{} level={} gold={} mode={:?}",
                s.map_id,
                world.hero.pos,
                s.hp,
                s.max_hp(),
                s.level,
                s.gold,
                s.mode
            );
            println!("DUMP flags={:?}", s.flags);
            println!("DUMP weave={:?} skills={:?}", s.weave_nodes, s.equipped_skills);
            println!(
                "DUMP inventory={:?} sigils={:?} mastery={:?} equipment={:?}",
                s.inventory, s.sigils, s.mastery, s.equipment
            );
        }
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if game.shot_taken && game.updates >= options.frames + 8 {
        exit.write(AppExit::Success);
    }
}

/// Keeps the window mode in step with the fullscreen setting (F11 toggles it); F12 saves a screenshot beside the game.
#[cfg_attr(target_arch = "wasm32", allow(unused_mut, unused_variables))]
fn window_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<LaunchOptions>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::F11) {
        let mut current = settings::current();
        current.fullscreen = !current.fullscreen;
        settings::set(current);
        settings::save();
    }
    let fullscreen = settings::current().fullscreen;
    if fullscreen != matches!(window.mode, WindowMode::BorderlessFullscreen(_)) && options.shot.is_none() {
        window.mode =
            if fullscreen { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed };
    }
    #[cfg(not(target_arch = "wasm32"))]
    if keys.just_pressed(KeyCode::F12) {
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = std::env::current_dir().unwrap_or_default().join("screenshots").join(format!("shot-{stamp}.png"));
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(".")));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}
