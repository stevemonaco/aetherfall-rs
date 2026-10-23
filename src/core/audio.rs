//! Chiptune sound effects synthesized at startup, and looping music with fades between tracks. Game code queues
//! requests here; the platform layer plays them.

use std::sync::{LazyLock, Mutex, MutexGuard};

use super::rng::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sound {
    Swing,
    Hit,
    Clink,
    EnemyDie,
    PlayerHurt,
    ChargeTick,
    ChargeLevel,
    ShootWind,
    ShootFire,
    Coin,
    Pickup,
    Heal,
    LevelUp,
    MenuMove,
    MenuSelect,
    MenuCancel,
    TextBlip,
    Door,
    ChestOpen,
    Break,
    BossRoar,
    BossDie,
    Burn,
}

impl Sound {
    pub const ALL: [Sound; 23] = [
        Sound::Swing,
        Sound::Hit,
        Sound::Clink,
        Sound::EnemyDie,
        Sound::PlayerHurt,
        Sound::ChargeTick,
        Sound::ChargeLevel,
        Sound::ShootWind,
        Sound::ShootFire,
        Sound::Coin,
        Sound::Pickup,
        Sound::Heal,
        Sound::LevelUp,
        Sound::MenuMove,
        Sound::MenuSelect,
        Sound::MenuCancel,
        Sound::TextBlip,
        Sound::Door,
        Sound::ChestOpen,
        Sound::Break,
        Sound::BossRoar,
        Sound::BossDie,
        Sound::Burn,
    ];
}

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Saw,
    Noise,
}

#[derive(Clone, Copy)]
struct Voice {
    wave: Wave,
    from: f32,
    to: f32,
    duration: f32,
    volume: f32,
    delay: f32,
    duty: f32,
}

const fn v(wave: Wave, from: f32, to: f32, duration: f32, volume: f32) -> Voice {
    Voice { wave, from, to, duration, volume, delay: 0.0, duty: 0.5 }
}

impl Voice {
    const fn delay(mut self, delay: f32) -> Self {
        self.delay = delay;
        self
    }

    const fn duty(mut self, duty: f32) -> Self {
        self.duty = duty;
        self
    }
}

const SAMPLE_RATE: u32 = 22050;

fn arpeggio(wave: Wave, notes: &[f32], step: f32, volume: f32) -> Vec<Voice> {
    notes
        .iter()
        .enumerate()
        .map(|(i, &n)| {
            let length = step * if i == notes.len() - 1 { 3.0 } else { 1.3 };
            v(wave, n, n, length, volume).delay(i as f32 * step).duty(0.25)
        })
        .collect()
}

fn voices(sound: Sound) -> Vec<Voice> {
    use Wave::*;
    match sound {
        Sound::Swing => vec![v(Noise, 3000.0, 800.0, 0.09, 0.25), v(Square, 900.0, 300.0, 0.07, 0.12).duty(0.25)],
        Sound::Hit => vec![v(Square, 300.0, 80.0, 0.10, 0.35), v(Noise, 2000.0, 500.0, 0.08, 0.25)],
        Sound::Clink => vec![
            v(Square, 2200.0, 2100.0, 0.05, 0.25).duty(0.125),
            v(Square, 1600.0, 1600.0, 0.08, 0.2).delay(0.04).duty(0.125),
        ],
        Sound::EnemyDie => vec![v(Noise, 1500.0, 100.0, 0.35, 0.4), v(Square, 600.0, 60.0, 0.3, 0.2)],
        Sound::PlayerHurt => {
            vec![v(Square, 500.0, 150.0, 0.18, 0.35).duty(0.25), v(Square, 480.0, 140.0, 0.18, 0.2).delay(0.02)]
        }
        Sound::ChargeTick => vec![v(Triangle, 900.0, 1000.0, 0.03, 0.15)],
        Sound::ChargeLevel => {
            vec![v(Square, 700.0, 1400.0, 0.12, 0.22).duty(0.25), v(Triangle, 1400.0, 2800.0, 0.12, 0.2).delay(0.05)]
        }
        Sound::ShootWind => vec![v(Noise, 600.0, 4000.0, 0.25, 0.3), v(Triangle, 300.0, 900.0, 0.2, 0.3)],
        Sound::ShootFire => vec![v(Noise, 400.0, 150.0, 0.35, 0.4), v(Saw, 220.0, 90.0, 0.3, 0.2)],
        Sound::Coin => vec![
            v(Square, 988.0, 988.0, 0.06, 0.25).duty(0.25),
            v(Square, 1319.0, 1319.0, 0.18, 0.25).delay(0.06).duty(0.25),
        ],
        Sound::Pickup => arpeggio(Square, &[523.0, 659.0, 784.0, 1047.0], 0.06, 0.25),
        Sound::Heal => arpeggio(Triangle, &[523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0], 0.05, 0.35),
        Sound::LevelUp => arpeggio(Square, &[392.0, 523.0, 659.0, 784.0, 659.0, 784.0, 1047.0], 0.08, 0.25),
        Sound::MenuMove => vec![v(Square, 1200.0, 1200.0, 0.03, 0.15).duty(0.25)],
        Sound::MenuSelect => vec![
            v(Square, 880.0, 880.0, 0.05, 0.2).duty(0.25),
            v(Square, 1320.0, 1320.0, 0.08, 0.2).delay(0.05).duty(0.25),
        ],
        Sound::MenuCancel => vec![v(Square, 660.0, 440.0, 0.08, 0.2).duty(0.25)],
        Sound::TextBlip => vec![v(Square, 1500.0, 1500.0, 0.015, 0.08).duty(0.125)],
        Sound::Door => vec![v(Noise, 300.0, 200.0, 0.2, 0.3), v(Triangle, 120.0, 80.0, 0.2, 0.4)],
        Sound::ChestOpen => arpeggio(Triangle, &[262.0, 330.0, 392.0, 523.0], 0.07, 0.4),
        Sound::Break => vec![v(Noise, 800.0, 60.0, 0.45, 0.5), v(Square, 120.0, 40.0, 0.3, 0.3)],
        Sound::BossRoar => vec![v(Saw, 110.0, 55.0, 0.8, 0.35), v(Noise, 300.0, 100.0, 0.8, 0.3)],
        Sound::BossDie => vec![v(Noise, 2000.0, 40.0, 1.6, 0.5), v(Square, 400.0, 30.0, 1.4, 0.3)],
        Sound::Burn => vec![v(Noise, 900.0, 300.0, 0.25, 0.3)],
    }
}

/// Renders a sound to a mono 16-bit WAV file in memory.
pub fn synthesize(sound: Sound) -> Vec<u8> {
    let voices = voices(sound);
    let length = voices.iter().map(|v| v.delay + v.duration).fold(0.0, f32::max);
    let count = (length * SAMPLE_RATE as f32) as usize + 1;
    let mut mix = vec![0.0f32; count];
    let rng = Rng::new(sound as u64 + 1);
    let rate = SAMPLE_RATE as f32;
    for v in &voices {
        let start = (v.delay * rate) as usize;
        let samples = (v.duration * rate) as usize;
        let (mut phase, mut noise, mut noise_clock) = (0.0f32, 0.0f32, 0.0f32);
        let mut i = 0;
        while i < samples && start + i < count {
            let t = i as f32 / samples as f32;
            let freq = v.from * (v.to / v.from).powf(t);
            phase = (phase + freq / rate) % 1.0;
            let sample = match v.wave {
                Wave::Square => {
                    if phase < v.duty {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                Wave::Saw => 2.0 * phase - 1.0,
                Wave::Noise => noise,
            };
            if let Wave::Noise = v.wave {
                noise_clock += freq / rate;
                if noise_clock >= 1.0 {
                    noise_clock -= 1.0;
                    noise = rng.next_f32() * 2.0 - 1.0;
                }
            }
            let attack = (i as f32 / (rate * 0.003)).min(1.0);
            let envelope = attack * (1.0 - t) * (1.0 - t * 0.3);
            mix[start + i] += sample * envelope * v.volume;
            i += 1;
        }
    }
    wav(&mix)
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    out
}

#[derive(Clone, Copy, Debug)]
pub struct SfxRequest {
    pub sound: Sound,
    pub volume: f32,
    /// Pitch shift in octaves (-1 to 1), as MonoGame's SoundEffect.Play takes it.
    pub pitch: f32,
}

const FADE_SECONDS: f32 = 0.6;

struct Audio {
    requests: Vec<SfxRequest>,
    sfx_muted: bool,
    music_muted: bool,
    sfx_volume: f32,
    music_volume: f32,
    current: Option<String>,
    pending: Option<String>,
    fade: f32,
    switching: bool,
    playing: bool,
}

static AUDIO: LazyLock<Mutex<Audio>> = LazyLock::new(|| {
    Mutex::new(Audio {
        requests: Vec::new(),
        sfx_muted: false,
        music_muted: false,
        sfx_volume: 0.8,
        music_volume: 0.55,
        current: None,
        pending: None,
        fade: 1.0,
        switching: false,
        playing: false,
    })
});

fn audio() -> MutexGuard<'static, Audio> {
    AUDIO.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn play(sound: Sound, volume: f32, pitch: f32) {
    let mut a = audio();
    if a.sfx_muted {
        return;
    }
    let volume = (volume * a.sfx_volume).clamp(0.0, 1.0);
    a.requests.push(SfxRequest { sound, volume, pitch });
}

pub fn sfx(sound: Sound) {
    play(sound, 1.0, 0.0);
}

pub fn take_requests() -> Vec<SfxRequest> {
    std::mem::take(&mut audio().requests)
}

pub fn set_muted(muted: bool) {
    let mut a = audio();
    a.sfx_muted = muted;
    a.music_muted = muted;
}

pub fn set_volumes(sfx: f32, music: f32) {
    let mut a = audio();
    a.sfx_volume = sfx;
    a.music_volume = music;
}

pub fn volumes() -> (f32, f32) {
    let a = audio();
    (a.sfx_volume, a.music_volume)
}

/// Switches music (fading out the old track); `None` stops it.
pub fn play_music(track: Option<&str>) {
    let mut a = audio();
    if a.music_muted || (track.map(str::to_string) == a.current && !a.switching) {
        return;
    }
    a.pending = track.map(str::to_string);
    a.switching = true;
    if !a.playing {
        a.fade = 0.0;
    }
}

/// What the platform layer should be playing this frame.
pub struct MusicState {
    pub track: Option<String>,
    pub volume: f32,
}

/// Advances fades and returns the track and volume to play.
pub fn update_music(dt: f32) -> MusicState {
    let mut a = audio();
    if a.switching {
        a.fade -= dt / FADE_SECONDS;
        if a.fade <= 0.0 || !a.playing {
            a.current = a.pending.take();
            a.playing = a.current.is_some();
            a.switching = false;
            a.fade = 0.0;
        }
    } else if a.fade < 1.0 {
        a.fade = (a.fade + dt / FADE_SECONDS).min(1.0);
    }
    MusicState { track: a.current.clone(), volume: (a.music_volume * a.fade.max(0.0)).clamp(0.0, 1.0) }
}
