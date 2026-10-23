# dev

Prebuilt native libraries the game links statically, so players install nothing and building the game needs no C++ compiler.

```
dev/
├── build-libopenmpt/        tool that builds libopenmpt for the current machine
├── libopenmpt-darwin/<target>/
├── libopenmpt-linux/<target>/
└── libopenmpt-win/<target>/
```

Each `<target>` folder (e.g. `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`) holds the static libraries, libopenmpt's `LICENSE` (it must ship with the game) and a `VERSION` note.

## Building libopenmpt for a new OS or CPU

On the machine you want to support, run:

```bash
cargo run -p build-libopenmpt
```

It downloads the pinned libopenmpt source release, checks its SHA-256, compiles it (about a minute) and writes the libraries to the folder for that machine. Commit the new files.

This needs a C++ compiler on that machine, once:
- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Windows:** Visual Studio Build Tools with "Desktop development with C++" (Rust on Windows already needs these).
- **Linux:** gcc or clang, e.g. `sudo apt install build-essential`.

If the game is built for a target that has no libraries here yet, the build stops with a message saying to run the command above.

## Updating libopenmpt

Change `VERSION`, `URL` and `SHA256` in `build-libopenmpt/src/main.rs`, rerun the tool on every OS, and commit all the updated folders together.
