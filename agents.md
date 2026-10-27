# Repository Guidelines

## Project Overview

Rust reconstruction of Plants vs. Zombies: Game of the Year Edition on Bevy 0.20. Ships no game data: the app reads the player's own `main.pak` at runtime. Start with `docs/llms/` (`README.md`, `project-structure.md`, `workflow.md`, `status-and-todo.md`); `docs/architecture.md` is the short human overview.

Binding rules: never commit or push (leave changes uncommitted, no amend/reset/rebase); never edit `readme.md` (user-owned); LF line endings everywhere (`* text=auto eol=lf`); game assets (`*.pak`, `/extracted`) never go in the repo.

Reference decomp (use respectfully): shallow-clone `https://github.com/ruslan831/PlantsVsZombies-decompilation` outside the repo, `grep -a` (Latin-1/GBK). Reimplement, don't transcribe; add `// Behaviour per ruslan831 decomp, <Fn>` credit; respect its MIT license.

## Architecture & Data Flow

```
main.pak --pak::Pak--> sexy parsers --GameFiles--> pvz resources/graphics/audio --> Bevy screens/board
libopenmpt <-- openmpt::Module <-- pvz audio mixer (main/drums/hi-hats sample-locked)
```

- `pak`: bottom layer, Bevy-free. XOR `0xF7` decrypt, magic `0xBAC04AC0`, in-memory index, `normalize_path()` (lowercase, `/`-separated).
- `sexy`: engine-independent SexyAppFramework/TodLib port, Bevy-free. `GameFiles` (pak-first, loose-file fallback), `resources` (manifest), `reanim` + `matrix`, `particle`, `font`, `strings`, `image`, `data_array` (generational `DataArray<T>`/`Id<T>`), `rand` (`MtRand`), `tod` (curves).
- `openmpt`: safe wrapper over libopenmpt C API (`Module::from_bytes`, `read_stereo`, order/row jumps). Links prebuilt statics from `dev/libopenmpt-<os>/<target>/`.
- `pvz`: Bevy game. `AppState`: `Boot → Setup → Title → GameSelector ⇄ Board`, plus `Award`/`Store`/`Challenges`/`Credits` screens. Logic at 100 Hz `FixedUpdate` (`TICKS_PER_SECOND`); drawing in `Update`. `Board::tick` is the pure simulation; `update_board` adds app shell (dialogs, profile, next screen).
- Rendering: immediate-mode `Graphics` resource (`fill_rect`, `draw_image`, `draw_string`, `translate`, `clip_rect`, `draw_image_matrix`) flushed in `PostUpdate` to sprites/mesh quads (`quad_material.wesl`); `EffectSystem` holds reanimations/particles in `DataArray`s. Screens are one `Plugin` each gated by `AppState`; modal `Dialogs` stack runs `DialogInput → DialogActions → ScreenInput → ScreenDraw → DialogDraw` (`UiSet`).
- Audio: one `MusicStream` renders up to 3 modules sample-locked; `Sounds` system param (`play_sample("SOUND_X")`, `play_foley(FoleyType)`). `GameRng` is the shared RNG; `UpdateCount` counts ticks.

## Key Directories

- `crates/pak/src/`: pak reader (`lib.rs`: `Pak`, `normalize_path`). Example `examples/extract.rs`.
- `crates/sexy/src/`: `files.rs`, `resources.rs`, `reanim.rs`, `particle.rs`, `font.rs`, `strings.rs`, `image.rs`, `data_array.rs`, `rand.rs`, `matrix.rs`, `tod.rs`, `definition.rs` (compiled-reader). Example `examples/check_resources.rs`.
- `crates/openmpt/src/`: `lib.rs` (`Module`); `build.rs` links prebuilts.
- `crates/pvz/src/`: `main.rs` (app/states), `board/` (simulation: `plant.rs`, `zombie/`, `projectile.rs`, `waves.rs`, `survival.rs`, `minigame.rs`, `coin.rs`, `cursor.rs`, `seed_bank.rs`, `seed_chooser.rs`, `cutscene.rs`, `grid_item.rs`, `level_end.rs`, `lawn_mower.rs`, `pool.rs`, `fog.rs`, `boss.rs`, `conveyor.rs`, `crazy_dave.rs`, `bungee.rs`, `whack_a_zombie.rs`, `save.rs`), `challenge.rs`, `graphics.rs`, `resources.rs`, `audio.rs`, `reanim.rs`, `particle.rs`, `dialog.rs`, `widgets/`, `lawn_app.rs`, `player.rs`, `game_selector.rs`, `title.rs`, `setup.rs`, `install.rs`, `store.rs`, `almanac.rs`, `credits.rs`, `options.rs`, `pak_source.rs`, `input.rs`, `devshot.rs`.
- `crates/pvz/tests/`: `TestBoard` suite compiled into the crate as `board::tests` (see Testing & QA; `survival.rs` and `minigame.rs` cover the challenge modes).
- `docs/llms/`, `docs/architecture.md`, `docs/todo.md`, `docs/superpowers/`: specs/plans archive.
- `dev/build-libopenmpt/`, `dev/libopenmpt-darwin|win|linux/<target>/`, `dev/README.md`: libopenmpt sources/prebuilts.
- `github/main menu.png`: reference screenshot.

## Development Commands

First Bevy build takes >10 min; run in background:

```bash
cargo build -p pvz
cargo run -p pvz
cargo run -p pvz -- --clean                 # wipe app data
cargo run -p pvz -- --lvl 2-1               # also --lvl-2-1 / --lvl=5-10 / --intro
cargo test --workspace
cargo test -p pak directories                # single test filter
cargo test -p pvz board::tests               # TestBoard suite (needs game)
cargo clippy --workspace --all-targets       # required clean after changes
cargo run -p pak --example extract -- <main.pak> [out_dir]
cargo run -p sexy --release --example check_resources -- <main.pak>
cargo run -p openmpt --example render -- <module> <out.wav> [seconds] [order]
cargo run -p build-libopenmpt                # rebuild prebuilt libopenmpt for this machine
```

Eye-check screens (computer-use can't drive `target/debug/pvz`):

```bash
PVZ_GAME_PATH=<dir|.app|main.pak> PVZ_AUTO_START=adventure \
PVZ_SCREENSHOT=/tmp/shot{}.png PVZ_SCREENSHOT_FRAME=400,450,500 \
cargo run -p pvz -- --lvl 3-1
```

Variants: `PVZ_AUTO_START=1|adventure|options|help|store|almanac|almanac-plants|almanac-zombies|credits|survival|minigames|survival-day|minigame-war|minigame-bowling|minigame-pogo|minigame-bobsled|minigame-column|minigame-invisighoul|minigame-boss|minigame-speed|minigame-stars|minigame-rain|minigame-portal|minigame-laststand|minigame-slot`, `PVZ_DEV_SCENE=butter|boss|boss-stomp|boss-rv|boss-bungee|pool`, `PVZ_CREDITS_JUMP=2:88`, `PVZ_SCREENSHOT_FRAME=<n>`. macOS black captures are a capture flake — retake, don't debug the game.

## Code Conventions & Common Patterns

- Formatting: `snake_case` modules mirroring original concepts (`seed_chooser`, `lawn_mower`); types mirror decomp names (`Board::TryToSaveGame`, `LawnApp::ButtonDepress`); `SCREAMING_SNAKE` constants (`GAME_WIDTH=800`, `GAME_HEIGHT=600`, `TICKS_PER_SECOND=100`, `MUSIC_VOLUME`). No `rustfmt.toml`/`clippy.toml`; `#[allow(...)]` only per-site for intentionally complete ported tables.
- State/DI: Bevy ECS only — no bespoke DI. Everything is `Resource` + `Plugin`; screens gate with `in_state(AppState::X)`, `resource_exists::<Board>()`, `Option<Res<T>>` for not-yet-loaded (e.g. `board/mod.rs:889-899`). Cross-screen events are messages: `PlayerChanged`, `DialogEvent`, `CoinEvent`. Shared audio via `#[derive(SystemParam)] Sounds` (`audio.rs:709-717`); use `Sounds::rng()` (shared `GameRng`), not a separate RNG param.
- Async: only two places. Asset reads are `async` but synchronous underneath (`pak_source.rs`). Real background work is `AsyncComputeTaskPool::spawn` + poll (`setup.rs` `PickTask` + `check_ready`; `resources.rs` `LoadingThread` + `receive_loaded_groups`). `PakSourcePlugin` must be added before `DefaultPlugins`.
- Error handling: library crates use `thiserror` enums, never `anyhow`/`unwrap` in lib paths (`PakError::{Io,BadMagic,Truncated,...}`, `ManifestError`, `ReanimError`, `CompiledError`). Loaders return `Result`/`Option`; missing assets degrade (`translate` → `"<Missing NAME>"`). App code logs and surfaces strings in UI (`SetupStatus.message`).
- Porting: keep `Board`/`DataArray`/enums-with-`match` structure and original names so code compares against the reference; cite source (`// Behaviour per ruslan831 decomp, Zombie::UpdateZombieWalking`).
- LSP first: use `xd://lsp` for definitions/references/hover before text-search refactors; `xd://ast_edit` for structural edits. Never `sed`/`perl`/`python` for single-file edits — use `edit`.

## Important Files

- Entry: `crates/pvz/src/main.rs` (`AppState`, `UiSet`, `main()`, `boot()`, `level_flag()`, flags `--clean/--lvl/--intro`).
- Config/save: `crates/pvz/src/install.rs` (`PVZ_GAME_PATH`, `config.toml` via `directories::ProjectDirs`, `userdata/`, `--clean` wipe); `crates/pvz/src/player.rs` (`PlayerInfo`/`ProfileMgr`, `users.dat`/`user<id>.dat` binary compat); `crates/pvz/src/board/save.rs` (`PVZ_SAVE` snapshots).
- Loading: `crates/pvz/src/pak_source.rs` (`pak://` source, `GamePak` slot), `crates/pvz/src/resources.rs` (weighted `LoadingImages/Fonts/Sounds` groups), `crates/sexy/src/resources.rs` (`resources.xml`, `SetDefaults` path/id prefixes, `Pak::find_with_extensions` with `tga,jpg,png,gif` then `.ogg/.wav/.au` for sounds).
- Simulation: `crates/pvz/src/board/mod.rs` (`BoardPlugin`, `Board`, `can_plant_at`, `add_plant`, `TickOutcome`), `crates/pvz/tests/mod.rs` (`test_board!`, `TestBoard`).
- Drawing/audio: `crates/pvz/src/graphics.rs` (`Graphics`), `crates/pvz/src/reanim.rs` (`ReanimationType` → file table, `draw_reanimation`, `NO_BASE_POSE`), `crates/pvz/src/audio.rs` (`Music`, `MusicTune`, `FoleyType`, `gLawnFoleyParamArray`), `crates/pvz/src/credits.rs` (credits movie).
- Dialogs/UI: `crates/pvz/src/dialog.rs` (`LawnDialog`, `DialogKind`, `Dialogs`), `crates/pvz/src/lawn_app.rs` (`ButtonDepress` dispatch), `crates/pvz/src/widgets/`, `crates/pvz/src/options.rs` (`Settings`), `crates/pvz/src/game_selector.rs`, `crates/pvz/src/title.rs`, `crates/pvz/src/setup.rs`.
- Content tables: `crates/pvz/src/plant.rs` (`SeedType`, `gPlantDefs`), `crates/pvz/src/zombie.rs` (`ZombieType`, `gZombieDefs`, `gZombieWaves`); every zombie type has a reanim, so waves pick from all of them.
- Sources of truth for behavior: `docs/llms/status-and-todo.md` (current port status; `grep -rn -E "TODO|Not ported" crates --include='*.rs'` for live index), `docs/architecture.md` (update when structure changes), `docs/llms/workflow.md` (port loop + gotchas).

## Runtime/Tooling Preferences

- Toolchain: Cargo only (workspace `resolver="3"`, `edition="2024"`, `Cargo.lock` v4 committed); no pinned toolchain, no `.cargo/` config. `[profile.dev] opt-level=1`, deps at `3` (Bevy unusable unoptimized). Check vendored Bevy 0.20 sources in `~/.cargo/registry/src/*/bevy*-0.20.0/` (incl. examples) instead of relying on memory of other Bevy versions.
- Runtime: player's `main.pak` required. Mac Steam: `~/Library/Application Support/Steam/steamapps/common/Plants Vs Zombies/Plants vs. Zombies.app/Contents/Resources/main.pak` (2,898 files, XML reanims, all `.ogg`); Windows GOTY: `C:\Program Files (x86)\Steam\steamapps\common\Plants Vs Zombies\main.pak` (3,198 files, `compiled/**/*.compiled` preferred, sounds try `.wav,.mp3,.ogg,.au`). Pak paths use `/`; `pak` normalizes. New resources must load from both paks (`check_resources` must be 0 failures).
- Mac pak gaps (verified by byte-scan 2026-10-26): no `HARDHAT2`, pogo stick-damage, `GARGANTUAR_HEAD_REDEYE`, `WALLNUT/TALLNUT_CRACKED`, `HEADLIGHT`, or `Zombie_surprise` particle images. `set_image_override` falls back to the base image when the id is missing, so write decomp-faithful overrides unconditionally — they show fully on the Windows pak and degrade gracefully on Mac. Don't branch per pak.
- libopenmpt: prebuilts committed (`dev/libopenmpt-<os>/<target>/` + `LICENSE` which must ship). Normal builds need no C++ compiler; if a target's libs are missing, ask the user to run `cargo run -p build-libopenmpt` on that machine (needs Xcode CLT / VS Build Tools + Desktop C++ / `build-essential`). Never add a fallback decoder silently; `.au` without a decoder stays silent with a warning.
- Platforms: macOS arm64 (primary, everything runs); Windows 11 x86_64 MSVC (builds/links, setup + `check_resources` pass; title/menu/audio untested visually). Linux/Wayland deferred (Bevy `wayland` feature on, never run). Windows gotchas: never `Path::canonicalize()` (verbatim `\\?\` breaks `cl.exe`) — build paths from `CARGO_MANIFEST_DIR`; WSL-built Linux libs are untested.
- Env/flags (see `main.rs`, `install.rs:9`, `title.rs:95`, `game_selector.rs:717-736`, `board/dev_scene.rs`, `devshot.rs`, `credits.rs:251`): `PVZ_GAME_PATH`, `PVZ_AUTO_START`, `PVZ_SCREENSHOT`/`PVZ_SCREENSHOT_FRAME`, `PVZ_DEV_SCENE`, `PVZ_CREDITS_JUMP`, `--clean`, `--lvl[-X-Y]`, `--intro`. Saved state in platform config dir (`~/Library/Application Support/pvz-goty-rs/` on macOS), profiles in `userdata/`.

## Testing & QA

- Frameworks: no separate harness. `#[cfg(test)] mod tests` units in `src/` for pure logic (lower crates stay Bevy-free so they're fast/hermetic), plus one `TestBoard` simulation suite: `crates/pvz/tests/` is not integration tests — `crates/pvz/Cargo.toml` sets `autotests=false` and `board/mod.rs:54-56` re-includes it via `#[path="../../tests/mod.rs"] mod tests;`.
- `TestBoard` holds the shared `Shared` mutex guard for its whole life: never hold two live boards in one test (a second `TestBoard::new` deadlocks). Split multi-setup tests, or scope the first in a block so it drops.
- `TestBoard::new("2-1")` (`test_board!` macro) is that level with intro skipped, waves held back unless `.with_waves()`; helpers `plant`, `plant_from_bank`, `dig`, `zombie`, `tick`/`ticks`/`run_until`. Runs real `Board::tick`; sounds silent, RNG seeded (`GameRng(2009)`), so runs repeat. Needs `main.pak` (`PVZ_GAME_PATH` else saved install); without it tests print `skipped` and pass — green but vacuous.
- Expectations: add a `TestBoard` test per new plant/zombie behavior (getters only tests need `#[cfg(test)]`); no coverage gate, no `nextest`. QA bar: per-feature test + `cargo test --workspace` + `cargo clippy --workspace --all-targets` clean + `check_resources` on both paks + screenshot eye-check for anything visual. Note: winning/money-collecting tests mutate the saved player profile.
- Useful filters: `cargo test -p pvz board::tests::<name>`, `cargo test -p pak directories`, `cargo test -p pvz <filter>`. Never `rerun` checks to confirm user-reported failures — treat them as ground truth.
