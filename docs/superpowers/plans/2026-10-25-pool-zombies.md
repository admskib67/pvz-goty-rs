# Pool zombies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The pool levels send and fight ducky tube, snorkel, dolphin rider, Zamboni and bobsled zombies.

**Architecture:** Extend `Zombie` with the original's height/water state, ported from the decomp; waves pick pool rows for swimmers; each zombie is a test-first vertical slice behind `ZombieType::is_implemented`.

**Tech Stack:** Rust, Bevy 0.20, the `TestBoard` harness in `crates/pvz/tests`.

**Spec:** `docs/superpowers/specs/2026-10-25-pool-zombies-design.md`

## Global Constraints

- Never commit or push (`agents.md`); leave changes in the working tree.
- Credit comment per ported function, e.g. `// Behaviour per ruslan831 decomp, Zombie::UpdateZombieSnorkel`. Reimplement, don't transcribe.
- LF line endings; `readme.md` untouched.
- Tests run with `cargo test -p pvz board::tests`; finish each task with `cargo clippy --workspace --all-targets` clean.
- The decomp is cloned in the session scratchpad; re-clone with `git clone --depth 1 https://github.com/ruslan831/PlantsVsZombies-decompilation` if gone.

## Review Focus

- A land zombie placed in a pool row at spawn (should sink on entering the first water square, not at x=0).
- A swimmer that is hypnotised, chilled or frozen by an ice-shroom while in the water.
- A snorkel killed by a puff or fume while submerged (shots should miss it until it surfaces).
- A dolphin rider whose first plant is a tall-nut or a tangle kelp.
- A Zamboni in a row with a lily pad plant (crush rules) and lawn mowers in water rows.

---

### Task 1: Pool-lane plumbing

**Files:**
- Modify: `crates/pvz/src/board/zombie.rs` (fields `height`, `in_pool`; `update_zombie_pool`, `check_for_pool`, `pool_splash`; drawing offset/clip)
- Modify: `crates/pvz/src/zombie.rs` (`ZombieType::can_go_in_pool`)
- Modify: `crates/pvz/src/board/waves.rs:~444` (`pick_row_for_new_zombie` pool rows)
- Test: `crates/pvz/tests/zombies.rs`

**Interfaces:**
- Produces: `ZombieType::can_go_in_pool(self) -> bool`; `Zombie::in_pool() -> bool` (`#[cfg(test)]` ok); `ZombieHeight` enum with `Normal, IntoPool, OutOfPool, DraggedUnder, Falling`.

- [ ] **Step 1: Write failing tests** `a_zombie_sinks_into_the_pool_and_climbs_out` (3-1, normal zombie in row 2 at x=700: `in_pool()` becomes true before x=640, altitude reaches -40, after crossing it becomes false and altitude 0) and `land_only_zombies_do_not_swim` (`ZombieType::Newspaper.can_go_in_pool()` false, `Normal` true).
- [ ] **Step 2: Run** `cargo test -p pvz board::tests::zombies::a_zombie_sinks` — expect FAIL (no `in_pool`).
- [ ] **Step 3: Implement** the fields and `update_zombie_pool` / `check_for_pool` per the decomp (columns at `x+75`, `x+45`, `x < 680`; entry depth `-40`), splash via the existing particle/reanim helpers.
- [ ] **Step 4: Run** the new tests and the whole `board::tests` — expect PASS.
- [ ] **Step 5: Waves** — test `swimmers_only_spawn_in_pool_rows` once Task 2 adds the type; here, assert `pick_row_for_new_zombie` still never gives land zombies a pool row.

### Task 2: Ducky tube zombie

**Files:** `crates/pvz/src/zombie.rs` (`reanim()`, allowed-level table), `board/zombie.rs` (tube overlay, variants), `board/waves.rs` (pool-row picking for swimmers), `tests/zombies.rs`.

**Interfaces:** Consumes `can_go_in_pool` and `check_for_pool` from Task 1. Produces `ZombieType::DuckyTube` in `is_implemented`.

- [ ] **Step 1:** failing tests `ducky_tube_zombie_swims_in_pool_rows_only`, `ducky_tube_armour_variants` (cone 370 / bucket 1100 helm health, 270 body).
- [ ] **Step 2:** run, expect FAIL.
- [ ] **Step 3:** map `DuckyTube` to its reanimation, add tube overlay and variant spawn per `gZombieAllowedLevels`, pick pool rows for it in `waves.rs`.
- [ ] **Step 4:** run, expect PASS; `levels_only_send_their_own_zombies` still passes.

### Task 3: Snorkel

**Files:** `board/zombie.rs` (`ZombiePhase::Snorkel*`, `update_zombie_snorkel`), `zombie.rs`, `tests/zombies.rs`.

- [ ] **Step 1:** failing tests `snorkel_dives_and_cannot_be_shot_while_swimming` (a peashooter's pea passes over it; fume-shroom still hurts it), `snorkel_surfaces_to_eat_a_lily_pad_plant`.
- [ ] **Step 2:** run, expect FAIL.
- [ ] **Step 3:** port phases `SnorkelWalking → IntoPool → WalkingInPool → UpToEat → EatingInPool → DownFromEat`; `effected_by_damage` rejects it while `WalkingInPool`; leaves the pool at `x <= 25`.
- [ ] **Step 4:** run, expect PASS.

### Task 4: Dolphin rider

**Files:** `board/zombie.rs` (`Dolphin*` phases, vault target shared with `PolePreVault` via a common `find_plant_target(attack_type)`), `zombie.rs`, `tests/zombies.rs`.

- [ ] **Step 1:** failing tests `dolphin_rider_jumps_the_first_plant`, `dolphin_rider_is_stopped_by_a_tall_nut`, `dolphin_rider_ignores_tangle_kelp`.
- [ ] **Step 2:** run, expect FAIL.
- [ ] **Step 3:** port `UpdateZombieDolphinRider` phases and attack rects.
- [ ] **Step 4:** run, expect PASS.

### Task 5: Zamboni and bobsled

**Files:** `board/zombie.rs`, `board/mod.rs` (per-row `ice_min_x`, `ice_timer`), `board/plant.rs` or `board/zombie.rs` (crush rule), `zombie.rs`, `tests/zombies.rs`. Read `UpdateZamboni`, `ZamboniDeath`, `UpdateZombieBobsled`, `BobsledCrash`, `BobsledDie` first.

- [ ] **Step 1:** failing tests `zamboni_crushes_plants_and_leaves_ice`, `zamboni_dies_with_its_driver`, `bobsled_team_rides_the_ice_and_crashes`.
- [ ] **Step 2:** run, expect FAIL.
- [ ] **Step 3:** port per the decomp; bobsled slice depends on the ice-trail state from the Zamboni half.
- [ ] **Step 4:** run, expect PASS.

### Task 6: Gating and docs

- [ ] Flip `is_implemented` for each type only as its task is green; update `agents.md` platform status and `docs/architecture.md`; tick `docs/todo.md` pool-zombie lines; run `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
