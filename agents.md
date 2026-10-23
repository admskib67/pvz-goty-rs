# agents.md

Guidelines for LLMs and coding agents working on this project.

## Project

A Rust reconstruction of Plants vs. Zombies: Game of the Year Edition, built on Bevy. It ships no game data: players supply their own legally owned copy, and the app reads its `main.pak` at runtime (see `readme.md`).

- Targets: Windows, Linux (Wayland), macOS arm64. Development and testing currently happen on macOS only.
- Current phase: engine and architecture foundations. Gameplay comes later.

## Files agents must not edit

- **`readme.md` is maintained by the user only.** Don't edit, rewrite, rename or delete it, even for small fixes or to add credits. If something in it looks wrong or out of date, tell the user instead.

## Reference project

This repository will be used as reference for an existing decompilation:
- https://github.com/ruslan831/PlantsVsZombies-decompilation

It is other people's work, made with a lot of effort. Use it respectfully:
- **Reimplement, don't transcribe.** Read it to understand behaviour (formulas, timings, state machines, edge cases), then write idiomatic Rust. Don't paste or mechanically translate large blocks of its code.
- **Respect its license.** As of 2026-10-22 it is MIT-licensed. Re-check before relying on it, and follow the license (including its attribution notice).
- **Give credit.** Where logic was worked out from the reference, add a short comment naming the function, e.g. `// Behaviour per ruslan831 decomp, Zombie::UpdateZombieWalking`.
- **Don't misrepresent.** Never present its findings as this project's own work or imply its authors endorse this project.

## Game data

Game assets never go in the repo (`.gitignore` excludes `*.pak` and `/extracted`).

- `main.pak` is a PopCap pak. The whole file is XOR'd with `0xF7`; then come magic `0xBAC04AC0`, version 0, a directory of `(flags, name, u32 size, u64 filetime)` records ending at flag `0x80`, and the concatenated file data.
- The developer's copy is the Steam **Mac** build v1.0.41 (32-bit, can't run on modern macOS): `~/Library/Application Support/Steam/steamapps/common/Plants Vs Zombies/Plants vs. Zombies.app/Contents/Resources/main.pak`. Its reanims (`.reanim`) and particles are plain XML. The Windows GOTY pak stores them as `.compiled` binaries, so Windows installs will need a separate parser.
- `properties/resources.xml` names files without extensions; resolve them with `Pak::find_with_extensions`.
- Sound effects are `.ogg`. Music is `.mo3`, a tracker format with per-channel volume control that Symphonia can't decode; the plan is libopenmpt behind a custom `bevy_audio` source.

## Commands

```bash
cargo build -p pvz                           # first Bevy build takes >10 min; run it in the background
cargo run -p pvz
cargo run -p pvz -- --clean                  # wipe all saved app data first (back to the setup screen)
cargo test --workspace
cargo test -p pak directories                # single test, filtered by name
cargo clippy --workspace --all-targets
cargo run -p pak --example extract -- <main.pak> [out_dir]  # list or extract a pak
cargo run -p sexy --release --example check_resources -- <main.pak>  # load every manifest image and font
```

App environment variables:
- `PVZ_GAME_PATH=<dir|.app|main.pak>` overrides the saved install and skips the setup screen.
- `PVZ_SCREENSHOT=<file.png>` saves a screenshot after ~120 frames, then exits; `PVZ_SCREENSHOT_FRAME=<n>` picks another frame. Use it to check screens visually; computer-use tools can't drive the bare `target/debug/pvz` binary.

The chosen install is saved in `config.toml` in the platform config dir (`~/Library/Application Support/pvz-goty-rs/` on macOS). `--clean` deletes the whole app data folder (config, plus future saves and caches) before starting.

The dev profile builds our crates at `opt-level = 1` and dependencies at `3`, because Bevy is unusably slow unoptimised.

## Architecture

`docs/architecture.md` is the short, human-facing overview of the project structure and the reasons for it. When a change alters the structure (a new crate, a new layer, a different approach to logic, drawing or loading), update it too, keeping it brief. The details below are for agents.

A Cargo workspace (`crates/*`) on Bevy 0.20. Bevy's API changes between releases, so check the vendored source in `~/.cargo/registry/src/*/bevy*-0.20.0/` (including `bevy-0.20.0/examples/`) rather than relying on memory of older versions.

- **`pak`** is the pak reader, with no Bevy dependency. It decrypts the whole file into memory and indexes entries by normalised path (lowercase, `/`-separated), matching how SexyFramework resolves paths.
- **`sexy`** holds engine-independent ports of SexyAppFramework/TodLib, unit-tested without Bevy:
  - `files`: `GameFiles` reads from the pak first, then loose files beside it (`properties/partner_logo.jpg` ships outside the pak).
  - `resources`: the `resources.xml` manifest (groups, `SetDefaults` path/id prefixes, image attributes).
  - `image`: decoding plus PopCap's alpha masks. A mask is `_name` or `name_`, its blue channel becomes alpha, and a mask with no base image becomes a solid `alphacolor` image; also `alphaimage` and per-cel `alphagrid`. Extensions are tried in the original order: tga, jpg, png, gif (the pak has both `titlescreen.jpg` and `.png`).
  - `tod`: `TodAnimateCurve` and friends. Most screen timing is written as these curves over count-down tick counters.
  - `font`: PopCap image fonts. A `data/*.txt` descriptor (a small `Define`/`LayerSet…` script, Latin-1 encoded) describes one or more layers, each a glyph sheet with per-character widths, rects, offsets and kerning. Multi-layer fonts (e.g. outline + main) draw every layer per character. `Font::layout` reproduces `DrawStringEx` (baseline-relative, widest layer wins the advance); only the default point size at scale 1 is supported.
  - `strings`: `LawnStrings.txt` (Latin-1, `[NAME]` then text); `translate("[NAME]")` mirrors `TodStringTranslate`, including `<Missing NAME>`.
- **`pvz`** is the Bevy application.
  - `AppState` runs `Boot → Setup → Title`. `Boot` opens `$PVZ_GAME_PATH` or the saved install, and falls back to `Setup` with `SetupStatus.message` explaining why.
  - `pak_source` registers the `pak://` asset source. Bevy builds asset sources when `AssetPlugin` is added, before the pak location is known, so `PakSourcePlugin` must be added **before** `DefaultPlugins`. Its reader shares a `GamePak` slot (`Arc<RwLock<Option<GameFiles>>>`) that is filled after setup; until then reads return `NotFound`.
  - `install` finds `main.pak` from whatever the player picks (the pak, a Windows/Linux install folder, a `.app` bundle, or the folder containing it), and persists the config. There is no auto-detection: the player always picks the folder through the OS dialog.
  - `setup` is the first-run screen (dark theme, Browse and Quit only): `bevy_ui` with `bevy_ui_widgets` `Button` and `Activate` observers, plus `rfd::AsyncFileDialog`. Picking and pak validation run on `AsyncComputeTaskPool` and are polled with `check_ready`.
  - The camera shows an 800×600 logical space (`ScalingMode::AutoMin`), the original game's resolution.
  - Game logic runs in `FixedUpdate` at 100 Hz (`TICKS_PER_SECOND`), like the original's `Update()`; drawing runs every frame in `Update`.
  - `graphics` is an immediate-mode `Graphics` resource modelled on SexyFramework's: screens call `fill_rect`, `draw_image`, `draw_string`, `clip_rect`, `push_state`/`pop_state` and `draw_image_matrix` in 800×600 top-left, y-down coordinates, so ported `Draw()` code stays close to the reference. The call list is cleared in `First` and turned into a reused pool of Bevy sprites in `PostUpdate`, depth by call order. Sprites can't skew, so reanimation will need a custom mesh path later.
  - `resources` turns manifest groups into Bevy assets: `GameResources::image("IMAGE_...")`, `font("FONT_...")` and `translate("[NAME]")`. `Init` loads synchronously; `GameResources::start_loading_thread` loads `LoaderBar` and `LawnStrings.txt`, then `LoadingImages`/`LoadingFonts`/`LoadingSounds` on a background task with the original's weighted progress. Sounds are only checked for presence until audio exists.
  - `input` keeps `GameCursor`, the mouse in 800×600 game coordinates.
  - `title` ports `TitleScreen` (PopCap logo, logo drop, grass bar with rolling sod cap, "LOADING..."/"CLICK TO START!" link with hover color). Still missing: sprouts on the bar, sounds, and what happens after the click (the game selector).

Planned direction:
- Game simulation as plain Rust structs on a fixed timestep, following the original's structure and names as documented by the reference (`Board`, `DataArray<T>` as a generational slot map with ID handles, type enums with `match` rather than trait objects), so code can be compared against it.
- `bevy_ui` for screens outside the original game (setup, settings, debug). The original's screens use bitmap fonts and pixel-exact drawing; they go through `graphics` and `Graphics::draw_string`.
