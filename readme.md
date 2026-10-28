# pvz-goty-rs

Vibe-coded reconstruction of Plants vs. Zombies: Game of the Year Edition.

- A valid copy of the game is required
- Right now all the levels and minigames should be playable
- This readme is also available in:

| [English](readme.md) | [Español](readme_spanish.md) | [Português](readme_portuguese.md) | [Русский](readme_russian.md) | [中文](readme_chinese.md) | [Français](readme_french.md) | [Deutsch](readme_german.md) |
| --- | --- | --- | --- | --- | --- | --- |

![main menu image](./github/main%20menu.png)

## How to run

- If a precompiled version exists for your OS just use that
- If not, you can build the game like a rust program running:

```bash
cargo build --release -p pvz
```

The outputs will be on:

- macOS/Linux: `target/release/pvz`
- Windows: `target\release\pvz.exe`

Note: If a build error occurs regarding libopenmpt, consult the libopenmpt section of the readme.

## Credits

- PopCap Games for creating the Plants vs Zombies game.
- ruslan831 for mantaing a decompilation of pvz on: https://github.com/ruslan831/PlantsVsZombies-decompilation.
- The libopenmpt dev team for producing the libopenmpt library: https://lib.openmpt.org/libopenmpt/.
- The people behind the bevy game engine: https://github.com/bevyengine/bevy

## Dev commands
For testing some specific command and tools have been developed, here are some of them:

```bash
cargo run -p pvz -- --clean          # wipe saved app data and profiles
cargo run -p pvz -- --lvl 3-1        # play a chosen adventure level
cargo run -p pvz -- --intro          # replay the first adventure intro
```

## Producing a mac version with an icon

Use the following script to package the game for mac:

```bash
./scripts/package-macos.sh
```

The app will be written to:

```text
target/release/Plants vs Zombies.app
```

## libopenmpt

Due to how some parts of the game audio are handled, using this library is a hard requirement.
Precompiled versions of for windows, mac and wsl ubuntu come bundled with the the project

In case the bundled libopenmpt librairies arent working for your os or architecutre you can build one with:

```bash
cargo run -p build-libopenmpt
```