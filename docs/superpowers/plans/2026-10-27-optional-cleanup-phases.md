# Optional, Cleanup, and No-Action Phases

> **For agentic workers:** Use checkbox (`- [ ]`) tracking. The required parity
> work is phase 14 (native Windows visual/audio and Linux); these phases are
> follow-ups that should not block it.

**Goal:** Close the non-required gaps without pretending stale TODOs are work.
Each phase ends with `cargo test --workspace` and
`cargo clippy --workspace --all-targets` clean, plus a docs update.

**Constraints:** Never commit or push; LF; do not edit `readme.md`; reimplement
from the ruslan831 decomp rather than transcribing it; visual changes get a
screenshot eye-check; no UI-only widget without a consumer unless the phase
explicitly says it is an engine-completeness item.

---

## Phase 15 — Optional gameplay parity

### 15.1 Typed dance cheat

**Files:** `crates/pvz/src/board/mod.rs` (key buffer/state),
`crates/pvz/src/board/zombie.rs` (`anim_dance`), `crates/pvz/tests/zombies.rs`

- [x] **Verify the reference:** `Zombie::UpdateZombieDancing` / `anim_dance`
      and the typed cheat switch. The decomp has no cheat UI here; GOTY uses a
      typed-word buffer.
- [x] **Add a small typed-letter buffer** in `board_input` (only while a board
      is live), matching the original word and handling keyboard repeat.
- [x] **Dance mode:** walking zombies play `anim_dance` and do not advance
      while the cheat is on; toggling the word again restores normal walking.
- [x] **Test:** typing the word toggles `Board::dance_mode`; a normal zombie
      switches reanim track; turning it off resumes walking. Screenshot of a
      dancing zombie.
- [x] **Docs:** note the cheat in `status-and-todo.md`.

### 15.2 Real potted-plant store icon

**Files:** `crates/pvz/src/store.rs`, `crates/pvz/src/zen_garden.rs`

- [x] Extract the potted-plant drawing used by the Zen Garden into a reusable
      helper (`draw_potted_plant_icon`): flowerpot plus the seed's reanim at
      `0.7`, with the age/variation rules.
- [x] Use it for the three `PottedMarigold*` store icons instead of the
      marigold seed-packet stand-in.
- [x] Test: the store page draws without panic and the icon helper returns the
      expected reanim/scale; screenshot the Zen 1 page.

**Not a phase:** bonus-mower and chocolate *store pages*. The decomp's
`gStoreItemSpots` has no page entries for them; verify and remove the stale
status claim in phase 17 rather than inventing UI.

---

## Phase 16 — Optional widget completeness

**Files:** `crates/pvz/src/widgets.rs` (+ its tests)

- [x] Vertical sliders: port the remaining `SliderWidget` orientation math
      (track/thumb rects, drag mapping, keyboard step), unit-tested with
      synthetic rects. No screen uses them; do not wire one in.
- [x] `EditWidget` clipboard/password characters: decide with a repo search
      whether any current screen needs them. If none does, implement only the
      password mask/character filtering and unit-test it; leave OS clipboard
      unsupported and document why.
- [x] If either item still has no consumer after the work, keep it as an
      engine-completeness API with tests rather than dead code.

**Acceptance:** widget tests cover both orientations, drag bounds, and text
filtering; no change to existing screens.

---

## Phase 17 — Cleanup and stale-TODO audit

**Files:** `docs/llms/status-and-todo.md`, `agents.md`, source TODO comments

- [ ] Run `grep -rn -E "TODO|Not ported|not yet" crates --include='*.rs'`
      and classify each hit: **done/stale**, **real gap**, or **intentional**.
- [ ] Verify and remove stale entries that are already implemented:
      foley pause, almanac pool water, coin bank/fade/preview, level-1 sod
      flash, bonus-mower rollback, trophy/store/Zen items, and the award
      growth note if it is already covered.
- [ ] Update `status-and-todo.md`’s snapshot date, “What works” list, missing
      tables, and suggested order. Remove completed “Not yet” sentences from
      module headers (`store.rs`, `game_selector.rs`, `award.rs`, etc.).
- [ ] Add a one-line policy: a TODO must name the missing behavior and the
      reference function, not just “TODO”.

**Acceptance:** every remaining `TODO` is actionable; no doc claims a done
feature is missing. No behavior tests needed beyond compile/clippy.

---

## Phase 18 — No-action decisions

These are intentionally not implementation tasks unless the audit finds a
consumer or a content path.

- [ ] **Pool/roof grave rising:** confirm no level/test places a grave in
      water or on the roof; keep the decomp branch documented in a comment as
      unused, or delete the dead helper. Do not invent a level to use it.
- [ ] **Clipboard/password characters:** document as unsupported (no OS
      clipboard, no password screen). Remove the “not ported” TODO if the
      decision is no-action.
- [ ] **Achievements:** already out of scope in `agents.md` (Steamworks).
- [ ] **Stinky sleeping/waking + chocolate-to-Stinky:** current Stinky is
      always awake; decide whether this is acceptable Zen simplification or a
      future optional phase. Write the decision in the Zen section of the
      status doc.
- [ ] **Vertical sliders:** if phase 16 chooses not to implement them, mark
      them engine-only/unused instead of “not ported”.

**Acceptance:** each no-action item has an explicit rationale in the docs;
nothing remains listed as missing without a decision.

---

## Phase 19 — Platform verification (required, not optional)

This is the existing phase 14: native Windows MSVC run (title/menu/board,
audio, compiled assets) and Linux/Wayland run. The macOS-side Windows-pak
resource check and `.au` decoder are done. Do not start optional phases at the
cost of leaving this unverified.
