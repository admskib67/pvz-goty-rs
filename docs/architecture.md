# Architecture

A Cargo workspace whose lower crates never depend on the game engine.

```
crates/
├── pak      reads main.pak
├── sexy     ports of PopCap's framework (no Bevy)
├── openmpt  tracker music playback (wraps libopenmpt)
└── pvz      the game, built on Bevy
dev/         prebuilt libopenmpt per OS, and the tool that builds it
```

## `pak`

Reads PopCap's `.pak` archive, where all the game data lives. It's the smallest, most stable piece, so it stands alone and is reused by tools such as the `extract` example.

## `sexy`

Plain-Rust versions of the parts of PopCap's SexyAppFramework and TodLib that PvZ is built on: the resource manifest, image loading with alpha masks, bitmap fonts, the string table, reanimations, particle systems, animation curves, the original's random number generator and its ID-based object pool. Keeping these out of Bevy means:
- they can be checked against the reference decompilation in isolation, with fast unit tests;
- tools can use them without starting a game window;
- a Bevy upgrade can't break them.

## `openmpt`

A small wrapper around libopenmpt, the only open-source library that plays PvZ's `.mo3` tracker music the way the game uses it (switching songs mid-file, fading drum channels). libopenmpt is C++, so it's linked from prebuilt static libraries in `dev/`, built once per OS (see `dev/README.md`). Players get a single executable with nothing to install, and building the game needs no C++ compiler.

## `pvz`

The application. Bevy supplies the window, input, rendering, audio and asset plumbing; the game itself follows the original's structure:
- **Game logic ticks at 100 Hz** in `FixedUpdate`, like the original, so timings ported from the reference behave the same.
- **Drawing goes through `graphics`**, an immediate-mode API shaped like the original's `Graphics` (800×600, top-left origin). Ported `Draw()` code reads almost line for line like the reference; the module turns it into Bevy sprites.
- **Resources load in groups** on a background task, as the original did, which drives the loading bar.
- **Music is played live** by libopenmpt through Bevy's audio, so the game can switch songs and fade the drum layers like the original; sound effects are ordinary Bevy audio.
- **Screens are Bevy states** (`Boot → Setup → Title → GameSelector ⇄ Board`, plus the award screen and Crazy Dave's store, which return to the menu or the next level). `bevy_ui` is used only for screens the original doesn't have, like the first-run install picker.
- **Dialogs are drawn and laid out like the original's** (stone frame, bitmap fonts), stacked over the current screen. A dialog only reports which button was clicked; the app-level code decides what that does, as `LawnApp` did. Big dialogs with a life of their own (the almanac) keep their state in their own module and hold a place in the dialog stack, so they stay modal and on top.
- **The board draws like the original**: every object (plants, zombies, mowers, effects, the UI) gets a render order from its layer and row, and the board draws them sorted, so things overlap the way they did in the game.
- **Settings live in `config.toml`** (the install, the last player, the options dialog's volumes and screen mode), where the original used the registry.
- **Player profiles use the original's save formats**, so an existing `userdata` folder from the game should load as-is.
- **The board is tested by simulation**: `Board::tick` is one game tick with nothing app-specific, so tests (`crates/pvz/tests`, compiled in as `board::tests`) set up a level, place plants and zombies, run ticks and check what happens, using the real animations and timings from the player's `main.pak`. Without the game they're skipped.

## Game data

No game assets are in the repository. The player picks their own install on first run, and everything is read from its `main.pak` at runtime.
