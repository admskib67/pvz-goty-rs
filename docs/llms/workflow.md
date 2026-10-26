# Workflow

How features have been built here, and the traps already hit. Commands are in `agents.md` ("Commands").

## Porting a feature

1. **Find it in the reference.** Shallow-clone ruslan831/PlantsVsZombies-decompilation into a temp folder outside the repo. Many files are Latin-1/GBK: use `grep -a`. Read the function(s) to learn the behaviour: timings, formulas, state machines.
2. **Find the data in the pak.** `cargo run -p pak --example extract -- <main.pak> <out_dir>` and read the `.reanim`/particle XML and `properties/resources.xml`. Track names, frame ranges and image ids come from there.
3. **Reimplement in idiomatic Rust**, following the original's names and structure so code can be compared (`Board`, `DataArray`, enums with `match`). Credit the reference in a comment: `// Behaviour per ruslan831 decomp, Plant::UpdateCactus`.
4. **Test it** with a `TestBoard` test in `crates/pvz/tests/` (plants/zombies), or a unit test in `sexy` for engine pieces. Run `cargo test -p pvz <filter>`.
5. **Check it by eye** when it is visual (below).
6. **Run** `cargo clippy --workspace --all-targets` and the tests.
7. **Update docs**: `agents.md` (what works, what's not ported), `docs/architecture.md` if the structure changed, and `docs/llms/status-and-todo.md`.
8. **Don't commit.** Report what changed.

## Checking screens by eye

Computer-use tools can't drive the bare binary; use the screenshot dev aid:

```bash
PVZ_AUTO_START=adventure PVZ_SCREENSHOT='/tmp/shot{}.png' PVZ_SCREENSHOT_FRAME=400,450,500 cargo run -p pvz -- --lvl 3-1
```

- `PVZ_SCREENSHOT_FRAME` takes several frames; `{}` in the file name becomes the frame number. The app exits shortly after the last one.
- Frames are wall-clock: loading takes about 300 frames, and timing varies run to run.
- `PVZ_AUTO_START=` `1` / `adventure` / `options` / `help` / `store` / `almanac` / `almanac-plants` / `almanac-zombies` / `credits`.
- `PVZ_DEV_SCENE=butter|boss|boss-stomp|boss-rv|boss-bungee|pool` sets up a scene (`board/dev_scene.rs`).
- `PVZ_CREDITS_JUMP=2:318` starts the credits part 2 at about frame 318.
- macOS captures sometimes come out fully black; that is the capture, not the game. Don't debug it; retake or ask the user to look.

## Gotchas

- **Bevy 0.20 APIs** differ from what you may remember: check `~/.cargo/registry/src/*/bevy*-0.20.0/` and its `examples/`.
- **First build takes over 10 minutes**: run it in the background.
- **Corrupted incremental cache** (linker "Undefined symbols" for our own functions, often after two cargo runs overlapped): `rm -rf target/debug/incremental/pvz-*` and rebuild.
- **`PakSourcePlugin` must come before `DefaultPlugins`.**
- **Pak paths** are case-insensitive and use `/` after normalising; never build Windows `\` paths by hand. Don't use `Path::canonicalize()` on Windows (verbatim `\\?\` paths).
- **Two paks differ**: Mac has XML reanims/particles and `.ogg` sounds; Windows has `compiled/*.compiled` and some `.au` sounds. Anything new must load from both (`check_resources`).
- **Reanimation attachments**: a parent with things attached via `attacher__` tracks has `NO_BASE_POSE` (-2), not -1 (which means "use the first frame"). An attacher track's alpha must fade what it holds.
- **Tests that win a level or collect money change the saved player profile.**
- **macOS has no `timeout` command**; BSD `sed -i` needs a suffix argument. Prefer the editing tools over shell edits.
