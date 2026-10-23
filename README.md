# Aetherfall (Rust / Bevy)

A port of [Aetherfall](https://github.com/stevemonaco/aetherfall) from C# / MonoGame to Rust and Bevy 0.20, with one
codebase that runs natively and in the browser. Content tooling stays in the C# repository: its PowerShell map
scripts write the JSON copied under `assets/data`, and `assets/external` is the private `aetherfall-resources`
submodule (licensed sprites and music; keep this repository private).

## Status

Act 1 is playable from the title screen through the Ember Warlock: the Sleeper's Vault, Leafhollow and its interiors,
Windward Fields, the Mossy Grotto and the boss arena, plus the optional Whisperwood. Everything those areas use is
ported: the terrain baker, chemistry (burning grass, ice, electricity), lighting, weather, the full player combat kit
(combos, bolts, dodges, guards, element modes, overload, Combat Arts), data-driven enemies, rifts, souls, puzzles,
dialogue, shops and menus.

Not yet ported (needed from Act 2 on): the Aether Tether, ice blocks and pressure plates, memory shards, the other
bosses, and the Blighted expeditions.

## Running

```powershell
git clone --recursive https://github.com/stevemonaco/aetherfall-rs.git
cargo run
```

Running the built exe directly needs `BEVY_ASSET_ROOT` set to the project folder (`cargo run` sets it).

Test switches (native only), matching the C# game:

```text
--map <id> [--at x,y]        start on a map, optionally at a tile
--give wind,fire             give items, swords or keys
--level <n> --flags a,b      set level / story flags (pact:<id> enables a Pact)
--script "right:20;attack:9" scripted input (action[+action]:frames; ...)
--fuzz <seed>                random input for crash testing
--boss-hp <scale>            scale boss life to reach late phases quickly
--shot out.png --frames <n>  run n updates, save a screenshot, exit
--dump                       print the game state when the screenshot is taken
--mute
```

F1 toggles hitboxes, F2 gains a level, F3 refills SP, F5 reloads the map, F11 toggles fullscreen and F12 saves a
screenshot.

### Web

```powershell
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve                        # http://127.0.0.1:8080
trunk build --release              # static site in dist/
```

`dist/` contains the licensed sprites and music, so it must not be published anywhere public.

## How the port is organised

The C# game is an object model drawn through MonoGame's SpriteBatch, and the port keeps that shape so the code ports
almost line for line. Bevy is the platform: window, input, audio, assets, GPU rendering and the web target.

- `core/` – colours, the pixel canvas, textures and the `Batch` draw list (SpriteBatch), the bitmap font, input
  mapping and scripts, synthesized sound effects and music control, settings and storage.
- `world/` – map data, the per-pixel terrain baker, themes, water, chemistry, lighting, weather, camera.
- `art/` – procedural sprites (props, combat effects, bosses) and the person rig used by the hero and villagers.
- `gameplay/` – the save state, combat formulas, items, Arts, the Circuit, souls, pacts, enemy and dialogue data.
- `entities/` – the player, enemies and bosses, projectiles, effects, props, puzzles, rifts, souls. Entities are
  updated one at a time with exclusive access to the rest of the world; reactions to an entity mid-update (a parry,
  a reflected shot) travel back through the `Hit` it passed.
- `scenes/` – the world, dialogue, menus, shops, the title screen and the rest of the scene stack.
- `platform.rs` – the Bevy app: asset preloading, turning each frame's draw list into sprites, the multiplied light
  map, device input, audio playback and the command-line switches.
