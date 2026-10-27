# pvz-goty-rs

<img src="./github/pvz-icon.png" width="96" align="right" alt="Plants vs. Zombies icon">

Vibe-coded reconstruction of *Plants vs. Zombies: Game of the Year Edition* in Rust and Bevy.

- A valid copy of the game is required at runtime; no `main.pak` or extracted game data ships with this repository.
- The app reads the player's `main.pak` from their installation.

![main menu image](./github/main%20menu.png)

## How to run

```bash
cargo run -p pvz
```

On first run the game asks for the folder containing a valid `main.pak`. You can also set it up front:

```bash
# macOS
PVZ_GAME_PATH="$HOME/Library/Application Support/Steam/steamapps/common/Plants Vs Zombies" cargo run -p pvz

# Windows PowerShell
$env:PVZ_GAME_PATH = "C:\Program Files (x86)\Steam\steamapps\common\Plants Vs Zombies"
cargo run -p pvz
```

Useful flags:

```bash
cargo run -p pvz -- --clean          # wipe saved app data and profiles
cargo run -p pvz -- --lvl 3-1        # play a chosen adventure level
cargo run -p pvz -- --intro          # replay the first adventure intro
```

## How to build

### Requirements

- A recent stable Rust toolchain with Cargo (`rustup` is recommended).
- A legal Plants vs. Zombies GOTY installation with `main.pak`.
- macOS: Xcode Command Line Tools for the linker.
- Windows: Visual Studio Build Tools with the MSVC C++ toolchain and Windows SDK.
- Linux: a C toolchain and the usual windowing/audio development packages.

The libopenmpt static libraries for macOS, Windows and Linux are committed under
`dev/libopenmpt-*`, so a normal build does not need a C++ compiler. If your target's
libraries are missing, run:

```bash
cargo run -p build-libopenmpt
```

### Debug build

```bash
cargo build -p pvz
cargo run -p pvz
```

The first build is slow because Bevy and the asset/audio dependencies are compiled.
The debug profile keeps this crate optimized at `opt-level = 1`.

### Release build

```bash
cargo build --release -p pvz
```

Outputs:

- macOS/Linux: `target/release/pvz`
- Windows: `target\release\pvz.exe`

### macOS `.app` with the game icon

Use the packaging script to create a double-clickable app bundle with
`github/pvz.icns` as its Finder/Dock icon:

```bash
./scripts/package-macos.sh
# or: ./scripts/package-macos.sh debug
```

The bundle is written to:

```text
target/release/Plants vs. Zombies.app
```

The app still reads the player's `main.pak`; use the first-run picker or
`PVZ_GAME_PATH`. The game's `main.pak` assets are not copied into the bundle.

### Windows executable icon

`crates/pvz/build.rs` embeds `github/pvz.ico` into the Windows executable, so
Explorer, the taskbar and Alt-Tab use the Plants vs. Zombies icon. Build with
MSVC as usual:

```powershell
cargo build --release -p pvz
```

If the icon resource fails to compile, make sure `rc.exe` (Windows SDK) is on
the MSVC toolchain path.

### Checks

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

Tests for board simulation use the real game files; they are skipped with a
message when `main.pak` cannot be found. Set `PVZ_GAME_PATH` to run them against
your installation.

## Credits

- PopCap Games: creators of *Plants vs. Zombies*, including the launcher icon.
- ruslan831: maintainer of the reference decompilation at
  <https://github.com/ruslan831/PlantsVsZombies-decompilation>.
