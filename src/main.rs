//! Aetherfall: a 2D action RPG ported from C# / MonoGame to Rust on Bevy.

mod art;
mod core;
mod entities;
mod gameplay;
mod platform;
mod scenes;
mod ui;
mod world;

fn main() {
    platform::run();
}
