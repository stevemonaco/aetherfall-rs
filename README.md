# Aetherfall

An action RPG of crystals, storms and sky-cities. Wake the dormant Aether Blade, master four elements, and restore the
towns the rifts tore apart. Built in Rust with [Bevy](https://bevyengine.org/).

## Play

- **In your browser:** [stevemonaco.github.io/aetherfall-rs](https://stevemonaco.github.io/aetherfall-rs/)
- **Windows:** download the latest `win-x64` zip from [Releases](https://github.com/stevemonaco/aetherfall-rs/releases),
  extract it anywhere and run `aetherfall.exe`.

Saves and settings are kept in `%APPDATA%\Aetherfall\rs` on Windows and in browser storage on the web.

## Controls

| Action                     | Keyboard and mouse  | Gamepad             |
| -------------------------- | ------------------- | ------------------- |
| Move                       | WASD / arrows       | Left stick          |
| Aim                        | Mouse               | Right stick         |
| Melee (and talk)           | J or right click    | A                   |
| Throw (hold to charge)     | K or left click     | RT                  |
| Dodge                      | Space               | B                   |
| Guard (tap early to parry) | Shift               | LT                  |
| Jump                       | C or L              | Y                   |
| Tether                     | F or I              | X                   |
| Skill (hold to charge)     | R                   | RB                  |
| Element mode               | Q / E, wheel, 1-4   | LB                  |
| Map                        | M                   | Back                |
| Menu                       | Esc / Tab           | Start               |

F11 toggles fullscreen.

## Features

- **The Aether Blade.** The hero wakes with a dormant blade that each sigil awakens a little further. Every sigil
  unlocks an element mode, and holding the mode key opens a quick-change wheel.
- **Aether.** Landing hits fills the Aether meter; taking hits drains it until the progress locks in. When it brims,
  Aether wakes for a few seconds: hits pierce to the target's weakness and the hero is hard to hurt.
- **Elemental mastery.** Each element grows its own mastery and teaches new skills. Tap the skill button for level 1,
  or hold it while the world slows to charge levels 2 and 3, unlocked by orbs and bracers hidden far from each sigil.
- **The Weave.** A lattice of about 180 passives themed by playstyle, free to re-pick at any save crystal. A full
  playthrough fills only about 60% of it.
- **Support gems.** Socket gems into each mode's attack, ranged and special groups, and hone them through Cut,
  Polished, Flawless and Perfect tiers with Gemcutter's Prisms.
- **Rifts.** Come near a rift and a swarm pours out. Beat it, seal the rift, and watch a piece of the town come back.
  Golden rifts are themed arenas of escalating waves, rolled hardships and a Rift Champion.
- **Elites and bosses.** Elites carry affixes and roam in packs, and seven bosses wait at the ends of the acts, with
  Echo remixes to face later.
- **A guardian spirit** fights beside the hero and learns beast forms (Hound, Hawk, Lantern Wisp, Ember Sprite), each
  with its own attack and an exploration perk.
- **Dungeons** each have a signature puzzle mechanic: wind, water, conduction, or past and present.
- **Beyond the main story:** side zones, the Bastion's eras, Aether Tether puzzles, Blighted expeditions, the Hollow
  Below, and the Hall of Echoes for testing builds on training dummies.
- **The pause menu** pages through each element's skills and sockets, keeps a bestiary of weaknesses and drops, and
  tracks every region's chests, Prisms and rifts in the Atlas.

## Building from source

The source is public, but the game can't be built in full from this repository. Its sprites and music are licensed
third-party assets that can't be redistributed, so they live in a private submodule (`assets/external`) and ship only
inside the official releases. The web and Windows builds are produced by `.github/workflows/release.yml` whenever a
`v*` tag is pushed.

With access to the assets:

```powershell
git clone --recursive https://github.com/stevemonaco/aetherfall-rs.git
cargo run
```

Running the built exe directly needs `BEVY_ASSET_ROOT` set to the project folder (`cargo run` sets it).

For the web build:

```powershell
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve                        # http://127.0.0.1:8080
trunk build --release              # static site in dist/
```

Releases copy the licensed files in scrambled (`tools/pack-assets.ps1`, which needs `pwsh`) and enable the
`packed-assets` feature, which unscrambles them as they load. The assets are never shipped as standalone files.

### Test switches

Native builds accept these switches for testing:

```text
--map <id> [--at x,y]        start on a map, optionally at a tile
--give wind_sigil,cleave_gem give items, sigils, gems or keys
--level <n> --flags a,b      set level / story flags (pact:<id> enables a Pact)
--mastery <n>                set every mode's mastery
--skills all                 learn every skill, unlock all three levels, full-strength Aether form
--weave all                  take every node of the Weave
--supports all               give every support gem and fill the open sockets
--script "right:20;attack:9" scripted input (action[+action]:frames; ...)
--fuzz <seed>                random input for crash testing
--boss-hp <scale>            scale boss life to reach late phases quickly
--shot out.png --frames <n>  run n updates, save a screenshot, exit
--dump                       print the game state when the screenshot is taken
--mute
```

In game, F1 toggles hitboxes, F2 gains a level, F3 refills SP, F5 reloads the map and F12 saves a screenshot.

### Balancing log

Normal play appends events to one log per playthrough, `%APPDATA%\Aetherfall\rs\telemetry\slotN_RUN.jsonl`, each line
stamped with the game version and the commit it was built from (`+dirty` for uncommitted builds). It records the hero's
level on first entering each zone and on beating each rift, golden rift and boss (with how long it took), plus
per-visit zone summaries of XP and gold gained, damage by element and attack kind, kills, deaths and Aether uptime. Test
runs log nothing. Summarise with `pwsh tools/telemetry-report.ps1` (`-Slot n`, `-Commit abc123`).

## Code layout

The game is an object model that records each frame into a draw list; Bevy is the platform underneath, providing the
window, input, audio, assets, GPU rendering and the web target. One codebase runs natively and in the browser.

- `core/` – colours, the pixel canvas, textures and the `Batch` draw list, the bitmap font, input mapping and scripts,
  synthesized sound effects and music control, settings and storage.
- `world/` – map data, the per-pixel terrain baker, themes, water, chemistry, lighting, weather, camera.
- `art/` – procedural sprites (props, combat effects, bosses) and the person rig used by the hero and villagers.
- `gameplay/` – the save state, combat formulas, items, skills, supports, mastery, the Weave, the guardian spirit's
  forms, pacts, and enemy and dialogue data.
- `entities/` – the player, enemies and bosses, projectiles, effects, props, puzzles, rifts, the guardian spirit.
  Entities are updated one at a time with exclusive access to the rest of the world; reactions to an entity mid-update
  (a parry, a reflected shot) travel back through the `Hit` it passed.
- `scenes/` – the world, dialogue, menus, shops, the title screen and the rest of the scene stack.
- `platform.rs` – the Bevy app: asset preloading, turning each frame's draw list into sprites, the multiplied light
  map, device input, audio playback and the command-line switches.
- `packed_assets.rs` – the asset reader that unscrambles the licensed files in release builds.

Map, dialogue, enemy and quest data live as JSON in `assets/data` and are compiled into the game.

## Credits

- Monsters and objects: "RPG Graphics - Sprites Pack" by Henrique Lazarini (7Soul), licensed
  [CC BY-ND 4.0](https://creativecommons.org/licenses/by-nd/4.0/)
- Animals: "Animals RPG Sprites" (GameDev Market)
- Music: "16-Bit Fantasy Action Music Pack" by Joel Steudler; "Retro Music Pack" by JDB
- Engine: [Bevy](https://bevyengine.org/)

Everything else (the hero, terrain, effects, bosses and sound effects) is generated procedurally in code.
