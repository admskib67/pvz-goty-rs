# Project structure

A Rust reconstruction of Plants vs. Zombies GOTY on Bevy 0.20. Cargo workspace, edition 2024, about 43k lines of Rust (snapshot 2026-10-26, branch `core`).

```
Cargo.toml              workspace; dev profile: our crates opt-level 1, deps 3
agents.md               binding rules + detailed per-feature notes (CLAUDE.md imports it)
readme.md               user-maintained: never edit
docs/
  architecture.md       short human overview
  todo.md               older TODO list (partly stale)
  llms/                 these docs
  superpowers/          past design specs and plans
dev/
  README.md             how the prebuilt libopenmpt is made
  build-libopenmpt/     tool that downloads, checks and compiles libopenmpt (run per OS by the user)
  libopenmpt-darwin/    prebuilt static libs (committed)
  libopenmpt-win/
  libopenmpt-linux/
crates/
  pak/                  main.pak reader          (no Bevy)
  sexy/                 PopCap framework ports   (no Bevy)
  openmpt/              libopenmpt wrapper       (no Bevy)
  pvz/                  the game (Bevy app, binary crate)
```

Dependency direction: `pvz → sexy → pak`, `pvz → openmpt`. The lower crates never depend on Bevy, so they stay unit-testable and stable across Bevy upgrades.

## `crates/pak`

`src/lib.rs`: decrypts the whole pak (XOR `0xF7`), parses the directory and indexes entries by normalised path (lowercase, `/`). `examples/extract.rs` lists or extracts a pak.

## `crates/sexy` (SexyAppFramework / TodLib ports)

| Module | Owns |
|--------|------|
| `files` | `GameFiles`: pak first, then loose files beside it |
| `resources` | `resources.xml` manifest: groups, `SetDefaults`, image attributes |
| `image` | decoding, PopCap alpha masks (`_name`/`name_`), `alphagrid`, extension order tga/jpg/png/gif |
| `font` | bitmap fonts from `data/*.txt` descriptors; `layout`, `word_wrap` |
| `strings` | `LawnStrings.txt`, `translate("[NAME]")` |
| `tod` | `TodAnimateCurve` and the curve helpers used for all screen timing |
| `matrix` | `Matrix3` (`SexyMatrix3`; `a * b` applies `b` first) |
| `reanim` | reanimation definitions (XML or compiled) and playback: frames, loops, blending, render groups, image overrides, attachment overlay matrices, `NO_BASE_POSE` |
| `particle` | `TodParticle`: definitions, `FloatTrack` syntax, emitters/particles, `draw_list()` |
| `definition` | shared reader for `compiled/*.compiled` files (zlib, `0xDEADFED4`) |
| `rand` | `MtRand`, the original's Mersenne Twister |
| `data_array` | `DataArray<T>`: generational slot map with `Id<T>` handles |

`examples/check_resources.rs` loads every manifest resource, reanim and particle from a pak (works for both Mac and Windows paks).

## `crates/openmpt`

Safe wrapper over libopenmpt's C API (load from memory, render stereo, order/row jumps, per-channel volume). `build.rs` links the committed prebuilt libraries in `dev/libopenmpt-<os>/<target>/`; it never compiles C++. `examples/render.rs` renders a module to WAV.

## `crates/pvz` (the game)

### App skeleton

- `main.rs`: plugin list, `AppState`, `UiSet`, CLI flags (`--clean`, `--lvl 2-1`, `--intro`), camera (800×600 logical, `ScalingMode::AutoMin`), `boot`.
- **States** (`AppState`): `Boot → Setup → Title → GameSelector ⇄ Board`, plus `Award`, `Store`, `Credits`. Each screen is a plugin with `OnEnter`/`OnExit` and its own systems.
- **Timing**: game logic in `FixedUpdate` at 100 Hz (`TICKS_PER_SECOND`), like the original's `Update()`. Drawing in `Update` every frame.
- **Frame order** (`UiSet`, chained): `DialogInput → DialogActions → ScreenInput → ScreenDraw → DialogDraw`. Dialogs take input first and draw last.

### Engine-side modules

| Module | Owns |
|--------|------|
| `pak_source` | the `pak://` Bevy asset source; `PakSourcePlugin` must be added **before** `DefaultPlugins` |
| `install` | finding `main.pak` from what the player picks; `config.toml`; `--clean` |
| `setup` | first-run install picker (`bevy_ui`, `rfd`) |
| `graphics` | immediate-mode `Graphics` resource shaped like SexyFramework's (`draw_image`, `draw_string`, `clip_rect`, `translate`, `push_state`, `draw_image_matrix`, `draw_mesh`, `set_additive`, `set_scale`...). Calls are collected each frame and turned into sprites/quad meshes in `PostUpdate`. `quad_material.wesl` is its shader |
| `resources` | `GameResources`: manifest groups → Bevy assets, `image()`, `font()`, `translate()`, background loading thread with the original's progress weights, `reanim_definition`, `particle_definition`, `modify_reanim_definition` |
| `reanim` | `ReanimationType` (full `gLawnReanimationArray`), `EffectSystem` (all running reanimations + particle systems, attachments, `attacher__` tracks, render groups), `draw_reanimation`/`draw_track_colored`, `draw_reanimator_frame` |
| `particle` | `ParticleEffect` enum → file, `draw_particle_system` |
| `audio` | `Music` (libopenmpt streams, tunes, fades), `Sounds`/`SoundSystem`, `FoleyType`, `UpdateCount` |
| `input` | `GameCursor` (mouse in game coordinates) |
| `widgets` | `GameButton`, `StoneButton`, `Slider`, `Checkbox`, `LabelButton`, `EditWidget`, `ListWidget`, `update_buttons` |
| `dialog` | `LawnDialog`, `DialogKind`, `DialogId`, the `Dialogs` stack (all modal), `DialogEvent`s |
| `lawn_app` | the `LawnApp` side: message boxes, user dialogs, what each dialog button does (`handle_dialog_events`), current player |
| `player` | `PlayerInfo`, `ProfileMgr`, saving in the original's binary formats (`users.dat`, `user<id>.dat`), `GameRng` |
| `options` | `Settings` (volumes, full screen, 3D accel), options dialog |
| `devshot` | `PVZ_SCREENSHOT` / `PVZ_SCREENSHOT_FRAME` (several frames, `{}` in the file name) |

### Screens

| Module | Screen |
|--------|--------|
| `title` | `TitleScreen`: logo, loading bar, sprouts, "click to start" |
| `game_selector` | main menu (`SelectorScreen.reanim`), buttons, locks, sign, `PVZ_AUTO_START` handling |
| `board/` | a level (see below) |
| `award` | `AwardScreen`: new plant/item, trophy, help note, credits note |
| `store` | Crazy Dave's store (rules are pure functions of `PlayerInfo`) |
| `challenge` | the challenge screen's survival and mini-games pages, and their trophy records (`userdata/challenge{id}.dat`) |
| `almanac` | `AlmanacDialog` (lives in the dialog stack as `DialogKind::Almanac`) |
| `credits`, `credits_timing` | the credits movie (three reanims with attacher sub-animations, singing brain, song, end names) |

### Game data tables (outside the board)

- `plant.rs`: `SeedType` (all 53, original order), `PlantDefinition` (`gPlantDefs`), upgrade/aquatic/nocturnal predicates, adventure award tables.
- `zombie.rs`: `ZombieType` (all 33), `gZombieDefs`, `gZombieWaves`, allowed-level tables.

### `board/`: the level

`Board` (in `board/mod.rs`) is the level's whole state. `Board::tick` is one 100 Hz game tick with nothing app-specific, which is what the tests drive. `update_board` adds the app's side (dialogs, profile, next screen). Drawing sorts every object by render order (`make_render_order`, `render_layer`), like `DrawGameObjects`.

| File | Owns |
|------|------|
| `mod.rs` | `Board`, `GameMode`, `GameScene`, level init, background picking, grid helpers, render list, `BoardPlugin`, pause/options, game over |
| `survival.rs` | survival's stages, stage tables and between-stage pause |
| `minigame.rs` | `MiniGame` (the challenge page's entries), each game's lawn, wave table, delays and shared bowling/trouble/whack rules |
| `beghouled.rs` | the 8×5 Beghouled / Twist match-3 board and shop |
| `zombiquarium.rs` | the fish tank's packets, brains and win/loss |
| `scary_potter.rs` | Vasebreaker's pots and phase completion |
| `plant.rs` | `Plant`: every plant's behaviour; acts through `PlantAction`s the board carries out |
| `zombie.rs` | `Zombie`: walking, eating, damage, every ported zombie type, pool/roof/vehicle logic, drawing; `ZombieEnv` + `Fx` are what it gets from the board |
| `zombie/boss.rs` | Dr. Zomboss (`update_boss`, `BossState`, `BossRequest`s) |
| `boss.rs` | the board's side of the boss level |
| `projectile.rs` | `Projectile`: straight, lobbed, homing, star, backwards, cob |
| `coin.rs` | sun, money, award coins, money bag |
| `cursor.rs` | cursor objects, planting rules (`can_plant_at`), shovel, mouse handling |
| `seed_bank.rs`, `conveyor.rs` | seed bank and packets; conveyor-belt banks |
| `seed_chooser.rs`, `tooltip.rs` | pre-level plant chooser, warnings, imitater picker |
| `cutscene.rs` | level intro timeline, first-ever intro cinematic, losing cutscene |
| `waves.rs` | zombie waves, spawning, progress meter, flag-wave messages |
| `level_end.rs` | award spawning, fade out, mowers→coins, zombies won, profile update |
| `lawn_mower.rs` | lawn mower, pool cleaner, roof cleaner |
| `grid_item.rs` | gravestones, craters, ladders; per-square looks (`GridCels`) |
| `pool.rs` | animated pool water (`PoolWater`, also used by the credits) |
| `fog.rs` | world 4's fog |
| `bungee.rs` | bungee targeting and grabbed plants |
| `crazy_dave.rs` | Crazy Dave: entering, talking, leaving; also used by the store (`board::figures`) |
| `tutorial.rs`, `message.rs` | tutorials, advice, on-screen messages |
| `whack_a_zombie.rs` | level 2-5 |
| `dev_scene.rs` | `PVZ_DEV_SCENE` set-ups for checking by eye |

### Tests

`crates/pvz/tests/*.rs` are compiled into the crate as `board::tests` (a `#[path]` module in `board/mod.rs`; `autotests = false`). `tests/mod.rs` has `TestBoard` / `test_board!`: a real level from the player's `main.pak`, intro skipped, with helpers to plant, dig, place zombies and run ticks. Without the game the tests print "skipped" and pass. Files: `plants`, `zombies`, `levels`, `world3`, `fog`, `roof`, `roof_zombies`, `boss`, `balloon`, `butter`, `almanac`, `options`, `credits`, `save`, `survival`, `minigame`.

Unit tests for the lower crates live next to their code (`#[cfg(test)]`).

## How a level tick flows

1. `FixedUpdate` → `update_board` → `Board::tick`.
2. Plants update and push `PlantAction`s; the board carries them out (shoot, make sun, explode...).
3. Zombies get a `ZombieEnv` (what they may know about the board) and report through `Fx` and requests (`BossRequest`, summon flags); the board applies them.
4. Projectiles, coins, mowers, waves, effects update; `EffectSystem::update` advances reanimations and particles.
5. `Update` → board draw systems fill `Graphics`; `graphics` turns the call list into entities in `PostUpdate`.

## Where new code goes

- A plant behaviour: `board/plant.rs` (+ `PlantAction` handling in `board/mod.rs`), definition data in `plant.rs`, a test in `tests/plants.rs`.
- A zombie type: `board/zombie.rs` (or a submodule like `zombie/boss.rs` if it is large), wire its reanim in `zombie.rs::reanim()`, add `FoleyType`s in `audio.rs` as needed, test in `tests/zombies.rs` or a new file listed in `tests/mod.rs`.
- A new screen: a new module with a plugin and an `AppState` variant in `main.rs`; draw through `graphics`, buttons through `widgets`, dialogs through `dialog`/`lawn_app`.
- Something engine-independent (a format, an algorithm from TodLib): `crates/sexy`, with unit tests there.
- A particle effect: add it to `ParticleEffect` in `particle.rs`, start it with `EffectSystem::add_particle`.
