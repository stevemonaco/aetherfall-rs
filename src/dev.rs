//! Command-line switches for jumping to a map and testing headlessly, matching the C# game's:
//! `--map <id> --script "right:20;attack:9" --shot out.png --frames <n>`.

use bevy::app::AppExit;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

#[derive(Resource, Default, Clone)]
pub struct LaunchOptions {
    pub map: Option<String>,
    shot: Option<String>,
    frames: u32,
    script: Vec<(Vec<KeyCode>, u32)>,
}

impl LaunchOptions {
    #[cfg(target_arch = "wasm32")]
    pub fn parse() -> LaunchOptions {
        LaunchOptions::default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn parse() -> LaunchOptions {
        let args: Vec<String> = std::env::args().collect();
        let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
        LaunchOptions {
            map: value("--map"),
            shot: value("--shot"),
            frames: value("--frames").and_then(|f| f.parse().ok()).unwrap_or(60),
            script: value("--script").map(|s| parse_script(&s)).unwrap_or_default(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_script(script: &str) -> Vec<(Vec<KeyCode>, u32)> {
    script
        .split(';')
        .filter_map(|segment| {
            let (actions, frames) = segment.trim().split_once(':')?;
            let keys = actions
                .split('+')
                .filter_map(|a| match a {
                    "up" => Some(KeyCode::ArrowUp),
                    "down" => Some(KeyCode::ArrowDown),
                    "left" => Some(KeyCode::ArrowLeft),
                    "right" => Some(KeyCode::ArrowRight),
                    "attack" => Some(KeyCode::KeyJ),
                    "next" => Some(KeyCode::KeyN),
                    _ => None,
                })
                .collect();
            Some((keys, frames.parse().ok()?))
        })
        .collect()
}

pub struct DevPlugin(pub LaunchOptions);

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone()).init_resource::<FrameCount>();
        if !self.0.script.is_empty() {
            app.add_systems(PreUpdate, play_script.after(InputSystems));
        }
        if self.0.shot.is_some() {
            app.add_systems(Update, take_shot);
        }
    }
}

#[derive(Resource, Default)]
struct FrameCount(u32);

fn play_script(options: Res<LaunchOptions>, frames: Res<FrameCount>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let mut start = 0;
    let mut held: &[KeyCode] = &[];
    let mut fresh = false;
    for (segment_keys, length) in &options.script {
        if frames.0 < start + length {
            held = segment_keys;
            fresh = frames.0 == start;
            break;
        }
        start += length;
    }
    for key in keys.get_pressed().copied().collect::<Vec<_>>() {
        if !held.contains(&key) || fresh {
            keys.release(key);
        }
    }
    for key in held {
        keys.press(*key);
    }
}

fn take_shot(
    mut commands: Commands,
    options: Res<LaunchOptions>,
    mut frames: ResMut<FrameCount>,
    mut exit: MessageWriter<AppExit>,
) {
    frames.0 += 1;
    if frames.0 == options.frames {
        let path = options.shot.clone().unwrap();
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    // Give the screenshot a few frames to come back from the GPU before quitting.
    if frames.0 == options.frames + 5 {
        exit.write(AppExit::Success);
    }
}
