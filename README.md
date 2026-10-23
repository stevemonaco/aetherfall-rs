# Aetherfall (Rust / Bevy)

A port of [Aetherfall](https://github.com/stevemonaco/aetherfall) from C# / MonoGame to Rust and Bevy 0.20, aiming
for one codebase that runs natively and in the browser. Content tooling stays in the C# repository: its PowerShell
map scripts write the JSON under `assets/data`, and `assets/external` is the private `aetherfall-resources`
submodule (licensed sprites and music; keep this repository private).

## Status: engine spike

Ported so far: the per-pixel terrain baker (identical output to the C# game), themes, animated water, the
procedural hero rig, tile collision with corner sliding, a 480x270 pixel-perfect screen scaled by whole numbers,
sword swings, and monsters that chase, take knockback and deal contact damage. Not yet ported: decor and buildings,
NPCs and dialogue, the combat system proper, rifts, menus, audio and saves.

## Running

```powershell
git clone --recursive https://github.com/stevemonaco/aetherfall-rs.git
cargo run                          # native; N / P cycle through showcase maps
cargo run -- --map temple_1        # start on a given map
```

Test switches (native only), matching the C# game: `--map <id>`, `--script "right:20;attack:9"`,
`--shot out.png --frames <n>`.

### Web

```powershell
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve                        # http://127.0.0.1:8080
trunk build --release              # static site in dist/
```

The web build only bundles the data and sprites it uses. `dist/` contains licensed sprites, so it must not be
published anywhere public.

## Layout

```text
src/
  main.rs     app setup, pixel-perfect screen, camera follow, HUD, map cycling
  map.rs      terrain types and the tile grid built from map JSON
  theme.rs    per-region colour ramps
  baker.rs    the per-pixel terrain baker and water frames
  art.rs      CPU canvas and the procedural person rig
  level.rs    map loading: bake, water, hero and monster spawning
  actors.rs   movement, collision, sword swings, monster AI
  data.rs     JSON asset loading and enemy definitions
  dev.rs      command-line test switches
```
