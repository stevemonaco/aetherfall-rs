//! Maps keyboard, mouse and gamepad state onto [`GameAction`]s and an aim direction. The platform layer feeds raw
//! device state in each frame; a test script can drive the actions instead (`"none:30;right:20;throw+aim45:90"`
//! holds each '+'-joined action set for the given number of frames; `aimN` aims N degrees clockwise from right).

use std::sync::{LazyLock, Mutex, MutexGuard};

use bevy::math::Vec2;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GameAction {
    Up,
    Down,
    Left,
    Right,
    Attack,
    Throw,
    Dodge,
    Guard,
    Jump,
    Tether,
    Art,
    PrevMode,
    NextMode,
    Mode1,
    Mode2,
    Mode3,
    Mode4,
    Mode5,
    Mode6,
    Confirm,
    Cancel,
    Menu,
    Map,
    Soul,
    Lens,
}

pub const ACTION_COUNT: usize = 25;

impl GameAction {
    pub const ALL: [GameAction; ACTION_COUNT] = [
        GameAction::Up,
        GameAction::Down,
        GameAction::Left,
        GameAction::Right,
        GameAction::Attack,
        GameAction::Throw,
        GameAction::Dodge,
        GameAction::Guard,
        GameAction::Jump,
        GameAction::Tether,
        GameAction::Art,
        GameAction::PrevMode,
        GameAction::NextMode,
        GameAction::Mode1,
        GameAction::Mode2,
        GameAction::Mode3,
        GameAction::Mode4,
        GameAction::Mode5,
        GameAction::Mode6,
        GameAction::Confirm,
        GameAction::Cancel,
        GameAction::Menu,
        GameAction::Map,
        GameAction::Soul,
        GameAction::Lens,
    ];

    pub fn parse(s: &str) -> Option<GameAction> {
        GameAction::ALL.iter().copied().find(|a| format!("{a:?}").eq_ignore_ascii_case(s))
    }
}

/// Raw device state for one frame, filled in by the platform layer.
#[derive(Default, Clone)]
pub struct RawInput {
    pub actions: [bool; ACTION_COUNT],
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub gamepad_connected: bool,
    pub gamepad_used: bool,
    pub keyboard_used: bool,
    pub mouse_screen: Option<Vec2>,
    pub mouse_moved: bool,
    pub mouse_buttons: bool,
    pub wheel: i32,
    pub window_active: bool,
    pub keys_pressed: Vec<String>,
}

struct Script {
    steps: Vec<(Vec<GameAction>, Option<f32>, i32)>,
    index: usize,
    frame: i32,
    aim: Option<f32>,
}

#[derive(Default)]
struct State {
    current: [bool; ACTION_COUNT],
    previous: [bool; ACTION_COUNT],
    raw: RawInput,
    script: Option<Script>,
    mouse_aiming: bool,
    mouse_screen: Vec2,
    direct_aim: Option<Vec2>,
    gamepad_active: bool,
    wheel: i32,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn load_script(script: &str) {
    let mut steps = Vec::new();
    for segment in script.split(';').filter(|s| !s.trim().is_empty()) {
        let parts: Vec<&str> = segment.trim().split(':').collect();
        let mut actions = Vec::new();
        let mut aim = None;
        if parts[0] != "none" {
            for token in parts[0].split('+').filter(|t| !t.is_empty()) {
                if let Some(deg) = token.strip_prefix("aim") {
                    aim = deg.parse().ok();
                } else if let Some(a) = GameAction::parse(token) {
                    actions.push(a);
                }
            }
        }
        let frames = parts.get(1).and_then(|f| f.parse().ok()).unwrap_or(1);
        steps.push((actions, aim, frames));
    }
    state().script = Some(Script { steps, index: 0, frame: 0, aim: None });
}

/// Drives the game with random held actions and aims, for crash testing.
pub fn load_fuzz(seed: u64, frames: i32) {
    let rng = super::rng::Rng::new(seed);
    let actions: Vec<GameAction> =
        GameAction::ALL.iter().copied().filter(|a| !matches!(a, GameAction::Menu | GameAction::Map)).collect();
    let mut parts = Vec::new();
    let mut total = 0;
    while total < frames {
        let length = 2 + rng.below(14);
        let mut held: Vec<String> = Vec::new();
        for _ in 0..rng.below(4) {
            let a = format!("{:?}", actions[rng.below(actions.len() as i32) as usize]);
            if !held.contains(&a) {
                held.push(a);
            }
        }
        held.push(format!("aim{}", rng.below(360)));
        parts.push(format!("{}:{}", held.join("+"), length));
        total += length;
    }
    load_script(&parts.join(";"));
}

pub fn scripted() -> bool {
    state().script.is_some()
}

/// Advances one frame with the latest raw device state.
pub fn update(raw: RawInput) {
    let mut s = state();
    s.previous = s.current;
    s.wheel = 0;
    if let Some(script) = s.script.as_mut() {
        let mut current = [false; ACTION_COUNT];
        if script.index < script.steps.len() {
            let (actions, aim, frames) = script.steps[script.index].clone();
            for a in actions {
                current[a as usize] = true;
            }
            if aim.is_some() {
                script.aim = aim;
            }
            script.frame += 1;
            if script.frame >= frames {
                script.index += 1;
                script.frame = 0;
            }
        }
        let aim = script.aim.map(|deg: f32| Vec2::new(deg.to_radians().cos(), deg.to_radians().sin()));
        s.current = current;
        s.direct_aim = aim;
        s.raw = raw;
        return;
    }

    if let Some(m) = raw.mouse_screen {
        s.mouse_screen = m;
    }
    let mouse_active = raw.mouse_moved || raw.mouse_buttons;
    if mouse_active && raw.window_active {
        s.mouse_aiming = true;
    }
    s.direct_aim = None;
    if raw.gamepad_connected {
        if raw.right_stick.length_squared() > 0.09 {
            s.direct_aim = Some(raw.right_stick.normalize());
            s.mouse_aiming = false;
        }
        if raw.gamepad_used {
            s.gamepad_active = true;
        }
    }
    if raw.keyboard_used || mouse_active {
        s.gamepad_active = false;
    }
    s.current = raw.actions;
    s.wheel = raw.wheel.signum();
    s.raw = raw;
}

pub fn held(action: GameAction) -> bool {
    state().current[action as usize]
}

pub fn pressed(action: GameAction) -> bool {
    let s = state();
    s.current[action as usize] && !s.previous[action as usize]
}

/// Whether a raw key (by its Bevy KeyCode debug name, e.g. "F1") went down this frame.
pub fn key_pressed(name: &str) -> bool {
    state().raw.keys_pressed.iter().any(|k| k == name)
}

/// Mouse wheel notches this frame (negative = up), used to cycle element modes.
pub fn wheel_steps() -> i32 {
    state().wheel
}

pub fn movement() -> Vec2 {
    let s = state();
    if s.script.is_none() && s.raw.gamepad_connected && s.raw.left_stick.length_squared() > 0.09 {
        return s.raw.left_stick;
    }
    let mut v = Vec2::ZERO;
    let c = &s.current;
    if c[GameAction::Left as usize] {
        v.x -= 1.0;
    }
    if c[GameAction::Right as usize] {
        v.x += 1.0;
    }
    if c[GameAction::Up as usize] {
        v.y -= 1.0;
    }
    if c[GameAction::Down as usize] {
        v.y += 1.0;
    }
    v
}

pub fn mouse_aiming() -> bool {
    state().mouse_aiming
}

pub fn mouse_screen() -> Vec2 {
    state().mouse_screen
}

pub fn direct_aim() -> Option<Vec2> {
    state().direct_aim
}

pub fn gamepad_active() -> bool {
    state().gamepad_active
}

/// Clears edge state so a press that closed one screen doesn't also trigger the next.
pub fn consume() {
    let mut s = state();
    s.previous = s.current;
}

pub fn glyph(action: GameAction) -> String {
    use GameAction::*;
    let pad = gamepad_active();
    let text = if pad {
        match action {
            Attack | Confirm => "A",
            Throw => "RT",
            Dodge | Cancel => "B",
            Guard => "LT",
            Jump => "Y",
            Tether => "X",
            Art => "RB",
            NextMode => "LB",
            Menu => "Start",
            Map => "Back",
            Soul => "R3",
            Lens => "L3",
            _ => return format!("{action:?}"),
        }
    } else {
        match action {
            Attack => "J/RMB",
            Throw => "K/LMB",
            Dodge => "Space",
            Guard => "Shift",
            Jump => "C",
            Tether => "F",
            Art => "R",
            PrevMode => "Q",
            NextMode => "E",
            Confirm => "J",
            Cancel => "K",
            Menu => "Esc",
            Map => "M",
            Soul => "V",
            Lens => "T",
            _ => return format!("{action:?}"),
        }
    };
    text.to_string()
}
