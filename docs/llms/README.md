# docs/llms: start here

Orientation for LLM agents working on this repository. Read these first, then the root `agents.md`, which holds the binding rules and the detailed per-feature notes.

| File | What it gives you |
|------|-------------------|
| [project-structure.md](project-structure.md) | The crates, every module and what it owns, how a tick and a frame flow, where to put new code |
| [status-and-todo.md](status-and-todo.md) | What works, what is missing, and the current code TODO policy (snapshot 2026-10-27) |
| [workflow.md](workflow.md) | How to port a feature, test it, check it by eye, and the gotchas already hit |

Other docs:
- `../architecture.md`: the short, human-facing overview (keep it brief; update it when the structure changes).
- `../superpowers/`: design specs and plans written for past features (world 3, pool zombies, fog, roof). Useful as examples of how a feature was planned.

## The rules that matter most

These are from `agents.md`; it wins if anything here disagrees.

- **Never commit or push.** Leave changes uncommitted and say what changed.
- **Never edit `readme.md`.** Tell the user if it looks wrong.
- **Line endings are LF.**
- **No game pak/extracted data in the repo.** The game reads the player's own `main.pak` at runtime; only the launcher icons are bundled.
- **The reference decompilation** (ruslan831/PlantsVsZombies-decompilation, MIT) is read to understand behaviour, then reimplemented in idiomatic Rust. Never paste or mechanically translate it. Credit it in a comment where logic came from it, e.g. `// Behaviour per ruslan831 decomp, Zombie::UpdateZombieWalking`.
- **Keep `agents.md` (and `docs/architecture.md` for structural changes) current** when you change things. Update `status-and-todo.md` when you finish or discover a TODO.
