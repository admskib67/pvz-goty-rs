# agents.md

Guidelines for LLMs and coding agents working on this project.

## Project

A Rust reconstruction of Plants vs. Zombies: Game of the Year Edition, built on Bevy. It ships no game data: players supply their own legally owned copy, and the app reads its `main.pak` at runtime (see `readme.md`).

- Targets: Windows and macOS arm64 for now. Linux (Wayland) is deferred: its libopenmpt libraries exist (built under WSL) but nothing is tested there. Development happens on macOS; the user also builds and runs on Windows 11.
- Current phase: engine and architecture foundations. Gameplay comes later.

## Branches

- `main` is the shared, stable line; the user merges the other branches into it.
- `core` is where the engine and game move forward (new systems and screens), developed on macOS. It started from `mac` once `mac` and `windows` were verified compatible.
- `mac` is the earlier macOS development line, now continued by `core`.
- `windows` is where a separate agent on Windows makes Windows-specific fixes. Keep platform fixes and core features in separate commits so the branches merge cleanly, and keep this file current on whichever branch you work on.

## Git

- **Never commit or push.** The user writes every commit and does all pushes and branch publishing themselves. Leave changes uncommitted in the working tree and say what changed. Don't amend, reset, stash, rebase or otherwise rewrite their history. Only create, switch or merge branches when the user asks, and leave merges uncommitted for them to commit.
- **Line endings are LF everywhere.** `.gitattributes` (`* text=auto eol=lf`) stores and checks out every text file with Unix line endings, Windows included. Write LF; never convert files to CRLF.

## Files agents must not edit

- **`readme.md` is maintained by the user only.** Don't edit, rewrite, rename or delete it, even for small fixes or to add credits. If something in it looks wrong or out of date, tell the user instead.

## Reference project

This repository will be used as reference for an existing decompilation:
- https://github.com/ruslan831/PlantsVsZombies-decompilation

To read it, shallow-clone it into a scratch or temp folder outside this repo (`git clone --depth 1 https://github.com/ruslan831/PlantsVsZombies-decompilation`); never add it to the repo. Many of its source files are Latin-1/GBK with Chinese comments, so plain `grep` may skip them as binary: use `grep -a` (or ripgrep's `-a`).

It is other people's work, made with a lot of effort. Use it respectfully:
- **Reimplement, don't transcribe.** Read it to understand behaviour (formulas, timings, state machines, edge cases), then write idiomatic Rust. Don't paste or mechanically translate large blocks of its code.
- **Respect its license.** As of 2026-10-22 it is MIT-licensed. Re-check before relying on it, and follow the license (including its attribution notice).
- **Give credit.** Where logic was worked out from the reference, add a short comment naming the function, e.g. `// Behaviour per ruslan831 decomp, Zombie::UpdateZombieWalking`.
- **Don't misrepresent.** Never present its findings as this project's own work or imply its authors endorse this project.

## Game data

Game assets never go in the repo (`.gitignore` excludes `*.pak` and `/extracted`).

- `main.pak` is a PopCap pak. The whole file is XOR'd with `0xF7`; then come magic `0xBAC04AC0`, version 0, a directory of `(flags, name, u32 size, u64 filetime)` records ending at flag `0x80`, and the concatenated file data.
- The user has two copies, and the two paks differ:
  - **Mac** (Steam, v1.0.41, 32-bit, can't run on modern macOS): `~/Library/Application Support/Steam/steamapps/common/Plants Vs Zombies/Plants vs. Zombies.app/Contents/Resources/main.pak`. 2,898 files. Reanims (`reanim/*.reanim`) and particles (`particles/*.xml`) are plain XML; every sound is `.ogg`. Everything ported so far is tested against this pak.
  - **Windows** (Steam GOTY): `C:\Program Files (x86)\Steam\steamapps\common\Plants Vs Zombies\main.pak`, an install folder rather than a `.app`. 3,198 files. Some sounds aren't `.ogg` (`SOUND_DIAMOND` wasn't found until `.au` was added to the extensions tried). Reanims and particles are expected to be compiled binaries only, at `compiled/<xml path>.compiled` (e.g. `compiled/reanim/LoadBar_sprout.reanim.compiled`) — verify with `extract` before relying on exact names.
- Compiled definitions, per ruslan831 decomp `Sexy.TodLib/Definition.cpp` (`DefinitionReadCompiledFile`, `DefinitionUncompressCompiledBuffer`, `DefinitionGetCompiledFilePathFromXMLFilePath`): a header with cookie `0xDEADFED4` and the uncompressed size, then zlib data. Uncompressed, it's a schema hash followed by the definition's structs as laid out in memory by the 32-bit game (pointers are 4 bytes and get replaced on load, strings and arrays follow inline), described by the `DefMap`/`DefField` tables in `Definition.cpp`. The original prefers the compiled file when it's in the pak, otherwise it parses the XML.
- `properties/resources.xml` names files without extensions; resolve them with `Pak::find_with_extensions`.
- Sound effects are `.ogg` on Mac; Windows also has others. The original's `DSoundManager::LoadSound` tries `.wav`, `.mp3`, `.ogg`, `.au` in that order, and `pvz::resources` matches it. Music is `.mo3`, a tracker module that only libopenmpt (or the original's BASS) can play; it must be played live with order jumps and per-channel volumes, so it can't be pre-rendered. It plays through libopenmpt in a custom `bevy_audio` source (see `pvz::audio`).

## Commands

```bash
cargo build -p pvz                           # first Bevy build takes >10 min; run it in the background
cargo run -p pvz
cargo run -p pvz -- --clean                  # wipe all saved app data first (back to the setup screen)
cargo test --workspace
cargo test -p pak directories                # single test, filtered by name
cargo clippy --workspace --all-targets
cargo run -p pak --example extract -- <main.pak> [out_dir]  # list or extract a pak
cargo run -p openmpt --example render -- <module> <out.wav> [seconds] [order]  # render a tracker module
cargo run -p build-libopenmpt                # rebuild prebuilt libopenmpt for this machine (see dev/README.md)
cargo run -p sexy --release --example check_resources -- <main.pak>  # load every manifest image, font, reanim and particle
```

App environment variables:
- `PVZ_GAME_PATH=<dir|.app|main.pak>` overrides the saved install and skips the setup screen.
- `PVZ_AUTO_START=1` continues past the title screen as soon as loading finishes; `PVZ_AUTO_START=adventure` goes straight on into the current player's next adventure level (useful with `PVZ_SCREENSHOT` to check the board).
- `PVZ_SCREENSHOT=<file.png>` saves a screenshot after ~120 frames, then exits; `PVZ_SCREENSHOT_FRAME=<n>` picks another frame. Use it to check screens visually; computer-use tools can't drive the bare `target/debug/pvz` binary.

On Windows use PowerShell: set environment variables with `$env:PVZ_GAME_PATH = "..."` before `cargo run -p pvz`.

The chosen install and the last player (`cur_user`) are saved in `config.toml` in the platform config dir (`~/Library/Application Support/pvz-goty-rs/` on macOS). Player profiles go in `userdata/` in the platform data dir (the same folder on macOS). `--clean` deletes the whole app data folder (config, profiles, and later saves and caches) before starting.

The dev profile builds our crates at `opt-level = 1` and dependencies at `3`, because Bevy is unusably slow unoptimised.

## Platform status

- **macOS arm64:** everything runs: setup screen, title screen with sprouts, main menu, dialogs and player profiles (new user, who are you?, rename, delete, locked-mode messages, quit confirmation), and an adventure level: the intro, planting from the seed bank, sun, level 1's tutorial, and the plain zombies (normal, flag, cone, bucket) walking in in waves, eating, and being shot by peashooters. Lawn mowers mow zombies down; a zombie reaching the house plays the "zombies ate your brains" cutscene and the Game Over dialog (Try Again / Main Menu); the last zombie of a level drops a seed packet that ends the level and moves the player to the next one.
- **Windows 11 (x86_64, MSVC from Visual Studio 2026, AMD GPU on Vulkan):** builds and links, including libopenmpt (`dev/libopenmpt-win/x86_64-pc-windows-msvc/`, plus `rpcrt4` for `UuidCreate`). The setup screen works and resource loading completes after the `.au` fix. Compiled `.reanim` files load (`ReanimDefinition::load` prefers `compiled/…`, falls back to XML); `check_resources` loads every resource and reanim from the Windows pak with 0 failures. Not yet run visually on Windows: the title screen and main menu. Compiled particles (`compiled/particles/*.xml.compiled`) have a reader (`ParticleDefinition::parse_compiled`) but it is untested against the Windows pak. Audio (from the `mac` branch) is untested there, and `.au` sounds stay silent until a decoder is added.
- **Linux:** deferred. `dev/libopenmpt-linux/x86_64-unknown-linux-gnu/` was built under WSL (real Linux libraries) but nothing has been run on Linux.

Windows gotchas already hit:
- `Path::canonicalize()` returns verbatim `\\?\C:\...` paths that `cl.exe` (and some other tools) can't open; build paths from `CARGO_MANIFEST_DIR` instead.
- Paths inside the pak use `\`; `pak` normalises them, but anything building file paths by hand should use `/` and let `pak`/`GameFiles` handle it.

Testing notes:
- On macOS, `PVZ_SCREENSHOT` captures sometimes come out fully black after the app has run for a few seconds (it happened even with all new drawing disabled, so it's the capture, not the game). Don't spend time debugging it; ask the user to check screens by eye instead.

## Architecture

`docs/architecture.md` is the short, human-facing overview of the project structure and the reasons for it. When a change alters the structure (a new crate, a new layer, a different approach to logic, drawing or loading), update it too, keeping it brief. The details below are for agents.

A Cargo workspace (`crates/*`) on Bevy 0.20. Bevy's API changes between releases, so check the vendored source in `~/.cargo/registry/src/*/bevy*-0.20.0/` (including `bevy-0.20.0/examples/`) rather than relying on memory of older versions.

- **`pak`** is the pak reader, with no Bevy dependency. It decrypts the whole file into memory and indexes entries by normalised path (lowercase, `/`-separated), matching how SexyFramework resolves paths.
- **`sexy`** holds engine-independent ports of SexyAppFramework/TodLib, unit-tested without Bevy:
  - `files`: `GameFiles` reads from the pak first, then loose files beside it (`properties/partner_logo.jpg` ships outside the pak).
  - `resources`: the `resources.xml` manifest (groups, `SetDefaults` path/id prefixes, image attributes).
  - `image`: decoding plus PopCap's alpha masks. A mask is `_name` or `name_`, its blue channel becomes alpha, and a mask with no base image becomes a solid `alphacolor` image; also `alphaimage` and per-cel `alphagrid`. Extensions are tried in the original order: tga, jpg, png, gif (the pak has both `titlescreen.jpg` and `.png`).
  - `tod`: `TodAnimateCurve` and friends. Most screen timing is written as these curves over count-down tick counters.
  - `font`: PopCap image fonts. A `data/*.txt` descriptor (a small `Define`/`LayerSet…` script, Latin-1 encoded) describes one or more layers, each a glyph sheet with per-character widths, rects, offsets and kerning. Multi-layer fonts (e.g. outline + main) draw every layer per character. `Font::layout` reproduces `DrawStringEx` (baseline-relative, widest layer wins the advance); only the default point size at scale 1 is supported. `Font::word_wrap` reproduces `WriteWordWrapped`'s line breaking and height (colour codes aren't supported).
  - `reanim` + `matrix`: reanimations (`reanim/*.reanim`, top-level XML fragments). Tracks hold one keyframe per frame; empty keyframes inherit earlier values; `anim_*` tracks only mark frame ranges. `Reanimation` ports playback (100 Hz `update`, loop types, interpolation, blending, `play`/`set_frames_for_layer`), render groups, image overrides and attachment overlay matrices (used to make one reanimation follow another's track). `Matrix3` mirrors `SexyMatrix3` (`a * b` applies `b` first). Track image ids not in the manifest are found via `image_search_paths` (`IMAGE_REANIM_X` → `reanim/X`, …).
  - `particle` (`TodParticle`): `ParticleDefinition::load` reads `particles/*.xml` (or the compiled form). `FloatTrack` parses the tracks' text syntax (nodes `n` or `[low high]`, optional `,time` percent, curve names between nodes; numbers are scanf-like, so `.3.8` is two numbers). `ParticleSystem` runs emitters and their particles (fields, cross-fades, overrides) and `draw_list()` returns what to draw, so the Bevy side only draws. `definition` holds the shared compiled-file reader (`decompress`, `CompiledReader`), used by reanims and particles.
  - `rand`: `MtRand`, the original's Mersenne Twister (31-bit output) with `range_int`/`range_f32` (`RandRangeInt`/`RandRangeFloat`). The game's instance is the `GameRng` resource.
  - `data_array`: `DataArray<T>`, a generational slot map standing in for the original's `DataArray`; screens keep `Id`s to objects such as reanimations.
  - `strings`: `LawnStrings.txt` (Latin-1, `[NAME]` then text); `translate("[NAME]")` mirrors `TodStringTranslate`, including `<Missing NAME>`.
- **`openmpt`** is a small safe wrapper over libopenmpt's C API (load from memory, render stereo, order/row jumps, per-channel volume). Its `build.rs` links prebuilt static libraries from `dev/libopenmpt-<os>/<target>/`, which are committed; it never compiles C++. Those libraries come from `dev/build-libopenmpt` (downloads the pinned release, checks its SHA-256, compiles with `cc`), run once per OS by the user. If a target's libraries are missing, ask the user to run it on that machine rather than adding a fallback. libopenmpt is BSD-3-Clause; its `LICENSE` sits beside each library and must ship with the game.
- **`pvz`** is the Bevy application.
  - `AppState` runs `Boot → Setup → Title → GameSelector ⇄ Board`. `Boot` opens `$PVZ_GAME_PATH` or the saved install, and falls back to `Setup` with `SetupStatus.message` explaining why.
  - `pak_source` registers the `pak://` asset source. Bevy builds asset sources when `AssetPlugin` is added, before the pak location is known, so `PakSourcePlugin` must be added **before** `DefaultPlugins`. Its reader shares a `GamePak` slot (`Arc<RwLock<Option<GameFiles>>>`) that is filled after setup; until then reads return `NotFound`.
  - `install` finds `main.pak` from whatever the player picks (the pak, a Windows/Linux install folder, a `.app` bundle, or the folder containing it), and persists the config. There is no auto-detection: the player always picks the folder through the OS dialog.
  - `setup` is the first-run screen (dark theme, Browse and Quit only): `bevy_ui` with `bevy_ui_widgets` `Button` and `Activate` observers, plus `rfd::AsyncFileDialog`. Picking and pak validation run on `AsyncComputeTaskPool` and are polled with `check_ready`.
  - The camera shows an 800×600 logical space (`ScalingMode::AutoMin`), the original game's resolution.
  - Game logic runs in `FixedUpdate` at 100 Hz (`TICKS_PER_SECOND`), like the original's `Update()`; drawing runs every frame in `Update`.
  - `graphics` is an immediate-mode `Graphics` resource modelled on SexyFramework's: screens call `fill_rect`, `draw_rect`, `draw_image`, `draw_image_box` (3×3-sliced), `draw_string`, `write_word_wrapped`, `clip_rect`, `translate`, `push_state`/`pop_state` and `draw_image_matrix` in 800×600 top-left, y-down coordinates, so ported `Draw()` code stays close to the reference. `translate` lets a widget draw in its own coordinates, like a widget's `Graphics` in the original. The call list is cleared in `First` and turned into reused entities in `PostUpdate`, depth by call order: axis-aligned draws become sprites; `draw_image_matrix`/`draw_image_matrix_src` (which may skew) become quad meshes with vertex colors and our own `QuadMaterial` (`quad_material.wesl`), pooled per texture.
  - `resources` turns manifest groups into Bevy assets: `GameResources::image("IMAGE_...")`, `font("FONT_...")` and `translate("[NAME]")`; sounds go to the `SoundSystem`. `Init` loads synchronously; `GameResources::start_loading_thread` loads `LoaderBar` and `LawnStrings.txt`, then `LoadingImages`, `LoadingFonts`, the drum and hi-hat music modules (`MusicInit`), then `LoadingSounds`, on a background task with the original's weighted progress.
  - `audio` ports `Music` and `TodFoley`:
    - Music: one endless `MusicStream` (a custom `Decodable`) renders up to three libopenmpt modules side by side, the original's main/drums/hi-hats instances, each with channels masked per tune (`SetupMusicFileForTune`). Because they're rendered in one loop they stay sample-locked, so the original's BPM resync isn't needed. `Music::make_sure_music_is_playing(MusicTune)`, `stop_all_music`, `fade_out`; it ticks in `FixedUpdate`. The title screen loads the main module (`MusicTitleScreenInit`). Not yet: drum/hi-hat bursts during levels, pause, the credits song.
    - Sounds: the `Sounds` system param has `play_sample("SOUND_X")` and `play_foley(FoleyType)` (random variation and pitch, the 10-tick repeat guard, max 8 instances, one-at-a-time and don't-repeat flags). `FoleyType` lists only the types used so far; add entries from the reference's `gLawnFoleyParamArray` as needed. Systems that play sounds and need randomness use `Sounds::rng()` (the shared `GameRng`) instead of a separate `GameRng` param. Only Ogg and WAV are decoded; other formats (the Windows pak's `.au`) are skipped with a warning, because rodio would panic on them.
    - `UpdateCount` counts 100 Hz ticks (`mUpdateCount`).
  - `reanim` maps every `ReanimationType` to its file (the full `gLawnReanimationArray`), loads definitions and their images on demand (`GameResources::reanim_definition`), and draws tracks (`draw_reanimation`, a port of `DrawTrack`; text and "fullscreen" tracks are still TODO). `EffectSystem` holds running reanimations and particle systems in `DataArray`s, like the original's effect system; a reanimation's `render_order` places it among the board's objects, and `is_attachment` ones are updated and drawn by their owner instead. `attach_to_another` ties a reanimation to another's track (a peashooter's head on its stem, a blink on the head's face): `update_reanimation` then moves it along (`GetAttachmentOverlayMatrix`) and `draw` draws it right after that track, as the original's attachment holder does. `kill` ends a reanimation and its attachments.
  - `particle` (pvz) has `ParticleEffect` (the original's `ParticleEffect` enum, with each one's file) and `draw_particle_system`, which draws a `ParticleSystem`'s `draw_list()` as transformed quads. `EffectSystem::add_particle(x, y, render_order, effect, …)` starts one. The board draws non-attachment particle systems among its render items by `render_order`, and queues its own requests (`ParticleRequest`, flushed at the end of each tick/frame) where the effect system isn't at hand: the level 1 tutorial arrow (`SeedPacketPick`, kept in `tutorial_arrow`), packet sparkles (`SeedPacketFlash`, from `flash_if_ready`). Planting dirt (`Planting`) and sod roll dirt (`SodRoll`) are started directly. Not yet: additive blending (`draw_particle_system` draws everything normally), `PlantingPool` and the other effects. `draw_reanimator_frame` replaces `ReanimatorCache`: it draws a still frame directly instead of caching an image (seed packet plants, idle mowers); the definition must be loaded beforehand.
  - `zombie`: `ZombieType` (all 33) with `gZombieDefs`, `gZombieWaves` and the allowed-level tables for the zombies ported so far.
  - `plant`: `SeedType` (all 53, in the original's order) with `PlantDefinition`s (`gPlantDefs`: reanimation, cost, recharge, launch rate), plus which plants adventure has handed out (`award_seed_for_level`, `seeds_available`).
  - `board`: the level (`AppState::Board`). `Board` holds the level's state (`InitLevel`, `PickBackground` with on-demand background groups, plant rows and grid squares, grid-to-pixel helpers). Its systems mirror `Board::Update` (100 Hz) and `DrawGameObjects`, which sorts everything by render order (`make_render_order`, `render_layer`) and draws it translated by the board's position, which the intro pans. Submodules: `plant` (`Plant`: body and head reanimations, blinking, shadow, render order; no abilities yet), `coin` (`Coin`, sun only: falling, fading, collecting), `cursor` (`CursorObject`/`CursorPreview`, `Board::mouse_down`, picking up packets, `can_plant_at` for plain grass, planting and sun spending), `tutorial` (`TutorialState` for level 1, `AdviceType` hints shown once, `display_advice`/`clear_advice`), `cutscene` (the level intro timeline: pans, the house-name banner, seed bank slide, sod roll with its looping digging sound, mowers rolling in, "Ready, Set, Plant!", then play and the level's music), `seed_bank` (`SeedBank`, `SeedPacket` with recharging, `draw_seed_packet` and `draw_seed_type`), `message` (`MessageWidget`, static styles only) and `lawn_mower`. The Menu button opens the "Leave Game?" dialog (the options dialog isn't ported). `zombie` (`Zombie`: the plain zombies' walking, eating, damage, head/arm/helmet dropping and death; `ZombieEnv` and `Fx` carry what a zombie needs from the board), `projectile` (`Projectile`: the pea), `waves` (`WaveState`: `PickZombieWaves`, the countdowns, `UpdateZombieSpawning`, row picking with `pick_from_smooth_array`). Plants act through `PlantAction`s (`Shoot`, `MakeSun`) that the board carries out, so a plant never needs the board; peashooters, snow peas (their peas chill zombies), repeaters, sunflowers, wall-nuts (they crack as they're eaten), cherry bombs, potato mines and chompers (bite, chew for 40 s, swallow) work; the plant's explosion goes through `PlantAction::CherryBomb`/`PotatoMine` and `Board::kill_all_zombies_in_radius`, and `shake_board` jolts the screen. Zombies can be chilled (half speed) and burned (charred). Only zombie types in `ZombieType::is_implemented` are picked for waves. `waves` also keeps the progress meter (`update_progress_meter`, `draw_progress_meter`). Zombies waiting on the street during the intro (`place_street_zombies`, from_wave `ZOMBIE_WAVE_CUTSCENE`, idle animations, removed when play starts) are placed by guess: the reference lacks the routine. `lawn_mower` (rolling in, triggering, mowing; plain lawn mowers only) and `level_end` (`TrySpawnLevelAward`, `FadeOutLevel`, `ZombiesWon`, `UpdatePlayerProfileForFinishingLevel`). Coins can be the level's `FinalSeedPacket`. The profile's level moves on when the level ends, so testing a win changes the saved player. Level 2's sunflower lesson and the later "plant more sunflowers" reminder (levels 3-7) work, and flag waves show the red "huge wave" warning (plain text, not the original's animated letters) before the wave, with the final wave banner. The shovel (bank at the right of the seed bank, `CursorType::Shovel`, digs the plant on a square) works, and level 1-4's award is the shovel coin; highlighting the plant under the shovel isn't drawn. Not yet: the other zombies and plants' abilities, money coins and mower bonus coins, the other level awards (shovel, almanac, trophies, notes), the award screen between levels (the next level starts directly), the original's Main Menu button on the Game Over dialog, the pool/roof/boss endings, the huge wave drum burst, the seed chooser, Crazy Dave, street zombies, the hit flash and plant eaten flash (both need additive drawing), saving, and the first-ever-adventure intro (`GAMEMODE_INTRO`).
  - `widgets`: `GameButton` (`NewLawnButton`: normal/over/down images, offsets, tint, optional polygon hit area) and `update_buttons`, which routes the mouse through buttons front to back like `WidgetManager` and reports enter/press/click; `StoneButton` (`LawnStoneButton`/`DrawStoneButton`), `EditWidget` (`EditWidget` + `LawnEditWidget`: cursor, selection, scrolling, mouse and keys; no clipboard or undo) and `ListWidget` (no scrollbar). Widgets inside a dialog use coordinates relative to the dialog.
  - `dialog`: `LawnDialog` (sized in whole tiles of the stone frame by `CalcSize`, buttons laid out by `Resize`, drawn by `Draw`) with a `DialogKind` for the extra widgets of `NewUserDialog` and `UserDialog`, and the `Dialogs` resource, the open dialogs back to front (`DoDialog`, `AddDialog`, `KillDialog`, `GetDialog`). All dialogs are modal: only the topmost gets the mouse and keyboard (its text box if it has one), and it can be dragged. Clicks become `DialogEvent`s; dialogs don't act on them themselves.
  - `lawn_app`: the `LawnApp` side of dialogs. `lawn_message_box`, `confirm_quit`, `do_user_dialog`, `do_create_user_dialog`, and the `Finish…Dialog` logic in `handle_dialog_events` (the original's `ButtonDepress(2000/3000 + id)`). The original's `LawnMessageBox` blocks until answered; here every dialog is asynchronous and reports through events. Also picks the player at startup (`CurUser`, else any profile), remembers the current player in `config.toml`, sends `PlayerChanged` when it changes, and saves the player's details on exit.
  - `UiSet` (in `main.rs`) orders each frame's `Update`: `DialogInput → DialogActions → ScreenInput → ScreenDraw → DialogDraw`, so dialogs take input before the screen behind them and draw on top. While `Dialogs::is_modal()`, screens treat the mouse as absent.
  - `player`: `PlayerInfo` (one player's progress, every field of the original) and `ProfileMgr` (the roster, unique names ignoring case, and the current player), saved in the original's binary formats: `users.dat` (version 14: names, use order, ids) and `user<id>.dat` (version 12: the details, with zen garden plants kept as raw 88-byte structs). A `userdata` folder from the original game should load as-is (untested: no original saves were available). Also `GameRng`.
  - `game_selector` ports `GameSelector` (main menu): `SelectorScreen.reanim` with clouds, leaves, clickable flowers, buttons that track reanim tracks, locked modes greyed out, the name sign, level numbers, the zombie hand on Adventure, the title theme and its sounds. With no player it shows the new-user dialog before the sign drops in (`SELECTOR_NEW_USER`, `SELECTOR_SHOW_SIGN`). Locked modes show their message, Quit asks first, and the sign's "If this is not you" opens the user dialog; the other buttons lead nowhere yet.
  - `input` keeps `GameCursor`, the mouse in 800×600 game coordinates.
  - `title` ports `TitleScreen` (PopCap logo, logo drop, grass bar with rolling sod cap, "LOADING..."/"CLICK TO START!" link with hover color, sprouts and zombie head popping out of the bar, title music and its sounds).

Planned direction:
- Game simulation as plain Rust structs on a fixed timestep, following the original's structure and names as documented by the reference (`Board`, `DataArray<T>` as a generational slot map with ID handles, type enums with `match` rather than trait objects), so code can be compared against it.
- `bevy_ui` for screens outside the original game (setup, settings, debug). The original's screens use bitmap fonts and pixel-exact drawing; they go through `graphics` and `Graphics::draw_string`.
