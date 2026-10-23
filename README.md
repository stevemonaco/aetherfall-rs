# Aetherfall (Rust / Bevy)

A port of [Aetherfall](https://github.com/stevemonaco/aetherfall) from C# / MonoGame to Rust and Bevy 0.20, with one
codebase that runs natively and in the browser. Content tooling stays in the C# repository: its PowerShell map
scripts write the JSON copied under `assets/data`, and `assets/external` is the private `aetherfall-resources`
submodule (licensed sprites and music; keep this repository private).

## Status

The whole game is ported: all acts and side zones, all seven bosses (including the Echo remixes), the Memory Lens
eras, the Aether Tether puzzles, the Blighted expeditions with their grades, and the Hollow Below.

Since the port, the combat systems have been reworked here (the C# game stays at 2.0); see
[docs/combat-rework.md](docs/combat-rework.md):

- The hero wakes with the dormant Aether Blade. Sigils set into its hilt unlock the element modes, and Corvin's forge
  awakens the blade for timed Aether bursts fuelled by a Resonance gauge.
- Each mode grows its own mastery from fighting in it.
- One skill per mode on a single button: tap for level 1, hold (the world slows) for levels 2 and 3.
- The Circuit is a single web of passives, triggers and keystones, reset for free at save crystals.
- Support gems socket into each mode's attack, ranged and special groups; they come from golden Trial Rifts.
- The Hall of Echoes (an Echo Stone by the Leafhollow shrine) has training dummies that show damage per second.

Map and dialogue data in `assets/data` started identical to the C# game's and now differs where the rework needed
it (sigils, the trials, the training hall).

## Running

```powershell
git clone --recursive https://github.com/stevemonaco/aetherfall-rs.git
cargo run
```

Running the built exe directly needs `BEVY_ASSET_ROOT` set to the project folder (`cargo run` sets it).

Test switches (native only), matching the C# game:

```text
--map <id> [--at x,y]        start on a map, optionally at a tile
--give wind_sigil,cleave_gem give items, sigils, gems or keys
--level <n> --flags a,b      set level / story flags (pact:<id> enables a Pact)
--mastery <n>                set every mode's mastery
--skills all                 learn every skill and unlock all three levels
--tree all                   learn every Circuit node whose arm is open
--supports all               give every support gem and fill the open sockets
--resonance                  start with a full Aether Resonance gauge
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
- `gameplay/` – the save state, combat formulas, items, skills, supports, mastery, the Circuit, souls, pacts, and
  enemy and dialogue data.
- `entities/` – the player, enemies and bosses, projectiles, effects, props, puzzles, rifts, souls. Entities are
  updated one at a time with exclusive access to the rest of the world; reactions to an entity mid-update (a parry,
  a reflected shot) travel back through the `Hit` it passed.
- `scenes/` – the world, dialogue, menus, shops, the title screen and the rest of the scene stack.
- `platform.rs` – the Bevy app: asset preloading, turning each frame's draw list into sprites, the multiplied light
  map, device input, audio playback and the command-line switches.
