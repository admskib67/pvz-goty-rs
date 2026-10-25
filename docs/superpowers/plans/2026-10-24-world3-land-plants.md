# World 3 land plants and pole vaulter gaps: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Squash, jalapeno, spikeweed, torchwood, threepeater and tall-nut behave like the original on plain grass, and the pole vaulter handles tall-nuts and leaves dust.

**Architecture:** Plants emit `PlantAction`s (`crates/pvz/src/board/plant.rs`) that `Board` carries out (`board/mod.rs`, match near line 1186), so a plant never touches the board. Each slice adds its ability to `Plant::update_abilities` plus any new `PlantAction`, and is driven by a `test_board!` test in `crates/pvz/tests/plants.rs`.

**Tech Stack:** Rust, Bevy 0.20, the workspace's `sexy` crate. Tests need the game's `main.pak` (they print "skipped" and pass without it).

**Spec:** `docs/superpowers/specs/2026-10-24-world3-land-plants-design.md`

## Global Constraints

- **Never commit or push** (`agents.md`): leave changes in the working tree; there are no commit steps. `readme.md` is untouched. Files use LF.
- Reference: clone `https://github.com/ruslan831/PlantsVsZombies-decompilation` (MIT) into the scratchpad (already at `<scratchpad>/PlantsVsZombies-decompilation`), never into the repo. Reimplement, don't transcribe. Every ported routine gets a comment like `// Behaviour per ruslan831 decomp, Plant::UpdateSquash`. Use `grep -a`.
- Verify per task with: `cargo test -p pvz board::tests::<name>`; at the end `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
- Out of scope: pool/water, lily pad, tangle kelp, imitater, world 4+ plants, spikerock.
- Timings, damage flags, particle and track names come from the reference when each task is written; the numbers below are the ones already confirmed.

## Review Focus

- Squash with no zombie in range or only in another row must not jump.
- A plant (spikeweed, squash) dug up or eaten mid-attack must not leave a dangling target or panic.
- Torchwood next to a dead/removed pea, or a pea overlapping two torchwoods in one row, must convert once and not double-convert.
- Threepeater on the top or bottom row must only fire rows that exist (no out-of-range row index).
- A vaulter blocked by a tall-nut must not repeat the block every tick or stay untouchable.
- Jalapeno must burn zombies in other states (frozen, dying, vaulting) without panicking and must not hurt zombies in other rows.

---

### Task 1: Tall-nut blocks the pole vaulter, plus vault dust

**Files:**
- Modify: `crates/pvz/src/board/zombie.rs` (`update_zombie_polevaulter`, ~line 1229; remove the `TODO: tall-nuts` comment)
- Test: `crates/pvz/tests/zombies.rs` (next to `pole_vaulter_jumps_the_first_plant`, line 158)

**Interfaces:**
- Consumes: `Zombie::find_plant_target(&self, plants) -> Option<Id<Plant>>`, `ParticleEffect::TallNutBlock`, `FoleyType` (add `Bonk` if missing).
- Produces: nothing for later tasks.

- [ ] **Step 1: Write failing tests** in `zombies.rs`: `pole_vaulter_cannot_jump_a_tall_nut` (plant `SeedType::Tallnut` at 3,2 and a `Polevaulter` at row 2, x 700; `run_until(1500, ...)` the vaulter reaches the nut; assert it never enters `ZombiePhase::PoleInVault`, ends up with `phase == ZombiePhase::Normal`, is still east of the nut's x, and then starts eating (`is_eating`)). Also `pole_vaulter_loses_its_pole_at_a_tall_nut` asserting `!has_object`... (use the zombie's pole flag, `has_object` as the existing code does).
- [ ] **Step 2: Run** `cargo test -p pvz board::tests::pole_vaulter_cannot_jump_a_tall_nut`. Expected: FAIL (it vaults).
- [ ] **Step 3: Implement** in `update_zombie_polevaulter`: when the jump (phase `PoleInVault`) reaches the point the reference checks (`Zombie::UpdateZombiePolevaulter`, the `SEED_TALLNUT` branch around Zombie.cpp:1698/1830), if the target plant is a tall-nut: play `FoleyType::Bonk`, add `ParticleEffect::TallNutBlock` at the plant, drop the pole, set `phase = Normal`, and restore the walking `attack_rect` and walk animation (as the existing `ended` branch does) without moving the zombie past the plant.
- [ ] **Step 4: Dust.** Add the vaulter's footstep dust while running and a landing dust at the end of the jump (reference: the `Dust`/`DustSquash` particles the vaulter spawns in `UpdateZombiePolevaulter`/`PlayZombieReanim` timed events); particle enum entries already exist in `particle.rs` (check names there). No test beyond "doesn't panic": run the existing vaulter tests.
- [ ] **Step 5: Run** `cargo test -p pvz board::tests::pole_vaulter` — all PASS, including the original wall-nut test.

### Task 2: Squash

**Files:**
- Modify: `crates/pvz/src/board/plant.rs` (new `PlantState::SquashLook/SquashJumpUp/SquashJumpDown/SquashLanded`... as the reference lists; `update_squash`; call from `update_abilities`; new `PlantAction::Squash { x, y, row, rect: IRect }`), `crates/pvz/src/board/mod.rs` (carry out the action), `crates/pvz/src/audio.rs` (`FoleyType::SquashHmm` if missing)
- Test: `crates/pvz/tests/plants.rs`

**Interfaces:**
- Consumes: `rect_overlap(IRect, IRect) -> i32`, `Zombie::zombie_rect()`, `Zombie::effected_by_damage(flags)`, `Zombie::has_head`, `Zombie::is_eating`, `Zombie::take_damage` (find the existing damage entry point used by `kill_all_zombies_in_radius`).
- Produces: `PlantAction::Squash` (Task 3 may mirror its shape for `Jalapeno`).

- [ ] **Step 1: Write failing tests**: `squash_flattens_a_zombie_in_front_of_it` (plant at 3,2; `Normal` zombie row 2 x 400; `run_until(400, zombie dead_or_dying)`; assert the squash plant is gone afterwards), `squash_ignores_other_rows_and_far_zombies` (zombie in row 1, and one at x 760 in row 2; `ticks(300)`; plant still `NotReady`, zombies unhurt).
- [ ] **Step 2: Run** both; expected FAIL.
- [ ] **Step 3: Implement** `Plant::find_squash_target(&self, zombies) -> Option<Id<Zombie>>` (same row, `has_head`, not in vault/jump phases, `-rect_overlap(attack, zombie_rect) <= 70`, or 110 if eating; keep the current target while valid) and `update_squash` (look state 80 ticks with `anim_lookleft/right` and `FoleyType::SquashHmm`; then jump to the target's lead x; on landing push `PlantAction::Squash` and die after the countdown). Board handler: for zombies in the row overlapping `rect` (football: overlap > -20, others > 0) apply 1800 damage with flags 18, add `ParticleEffect::DustSquash`, `shake_board`. Reference: `Plant::UpdateSquash`, `FindSquashTarget`, `DoSquashDamage`.
- [ ] **Step 4: Run** both tests; expected PASS. Run `cargo test -p pvz board::tests::every_plant_can_be_planted_and_dug_up`.

### Task 3: Jalapeno

**Files:**
- Modify: `crates/pvz/src/board/plant.rs` (`do_special` arm for `SeedType::Jalapeno`; start the countdown like ice/doom in `update_abilities`; `PlantAction::Jalapeno { x, y, row }`), `crates/pvz/src/board/mod.rs`
- Test: `crates/pvz/tests/plants.rs`

**Interfaces:**
- Consumes: the cherry bomb path (`Board::kill_all_zombies_in_radius` or the per-zombie burn it calls) for the charred death; `ParticleEffect` for the flame row (check `particle.rs`; `JalapenoFire` or similar).
- Produces: `PlantAction::Jalapeno`.

- [ ] **Step 1: Write failing tests**: `jalapeno_burns_its_whole_row` (zombies at x 300, 500, 700 in row 2 and one in row 1; plant at 1,2; `ticks(200)`; row-2 zombies dead_or_dying, row-1 zombie alive; plant gone), `jalapeno_burns_frozen_and_vaulting_zombies` (a chilled zombie and a `Polevaulter` in row 2 are still handled without panic; assert they die).
- [ ] **Step 2: Run**; expected FAIL.
- [ ] **Step 3: Implement** per `Plant::DoSpecial`/jalapeno branch and `Board::KillAllZombiesInRadius`-style row burn: foley, flame particle across the row, `shake_board`, die.
- [ ] **Step 4: Run** the new tests; expected PASS.

### Task 4: Spikeweed

**Files:**
- Modify: `crates/pvz/src/board/plant.rs` (`PlantState::SpikeweedAttacking`; `update_spikeweed`; targeting; zombies must walk over and eat it as the reference), `crates/pvz/src/board/zombie.rs` only if zombies currently skip it
- Test: `crates/pvz/tests/plants.rs`

**Interfaces:**
- Consumes: `PlantAction::RowAreaDamage { row, rect, damage, flags }` (already carried out in `board/mod.rs`), `Plant::find_target_zombie` (extend its attack-rect for spikeweed), `FoleyType::Throw`.
- Produces: nothing.

- [ ] **Step 1: Write failing tests**: `spikeweed_hurts_a_zombie_walking_over_it` (plant 3,2; zombie x 330 row 2; `run_until(300, body_health < 270)`; later `ticks(300)` and assert health dropped again, i.e. repeated hits), `spikeweed_ignores_zombies_in_other_rows` (row 1 zombie at same x stays at 270 over `ticks(300)`).
- [ ] **Step 2: Run**; expected FAIL.
- [ ] **Step 3: Implement** `update_spikeweed` per the reference: when idle and a zombie is in reach, play `anim_attack` (play-once-and-hold, blend 20, rate 18, `FoleyType::Throw`), state `SpikeweedAttacking`, countdown 100; at countdown 75 push `RowAreaDamage { damage: 20, flags: 33, rect: spikeweed's attack rect }`; at 0 return to idle (`play_idle_anim` equivalent used by other plants). Attack rect: the plant's own square.
- [ ] **Step 4: Run** tests; expected PASS. Confirm the plant can be eaten (zombie eats it and it dies) by extending the first test or adding `spikeweed_can_be_eaten`.

### Task 5: Torchwood and fire peas

**Files:**
- Modify: `crates/pvz/src/board/projectile.rs` (`ProjectileType::Fire`, damage 40, splash, `Projectile::convert_to_fireball(&mut self, col: i32)` and `convert_to_pea(&mut self, col: i32)`, track "already converted by column" to avoid double conversion; trail/draw image), `crates/pvz/src/board/plant.rs` (`update_torchwood` needs projectiles: either a new `PlantAction`-free pass in `Board::tick` before projectiles update, or pass `&mut DataArray<Projectile>`; prefer the board pass `Board::update_torchwoods` to keep plants board-free), `crates/pvz/src/board/mod.rs`
- Test: `crates/pvz/tests/plants.rs`

**Interfaces:**
- Consumes: `Projectile::rect()` (make `pub(super)`), `Plant::attack_rect()`/`rect()`, `rect_overlap`.
- Produces: `ProjectileType::Fire`, the two `convert_*` methods.

- [ ] **Step 1: Write failing tests**: `torchwood_turns_peas_into_fire_peas` (Peashooter at 0,2, Torchwood at 2,2, zombie at x 700 row 2; `run_until(300, any projectile.ty == ProjectileType::Fire)`; and the zombie loses 40 per hit at a hit), `torchwood_thaws_snow_peas` (Snowpea instead; projectile becomes `Pea`, zombie not chilled), `torchwood_ignores_other_rows` (torchwood row 1, peashooter row 2: no `Fire`), `two_torchwoods_convert_a_pea_once` (a second torchwood further along leaves it `Fire`).
- [ ] **Step 2: Run**; expected FAIL (no `Fire`).
- [ ] **Step 3: Implement** per `Plant::UpdateTorchwood` (overlap of torchwood attack rect and projectile rect >= 10, pea -> fire, snow pea -> pea) and the reference's `Projectile::ConvertToFireball`/`DoImpact` fire splash (neighbouring zombies in the same row get splash damage).
- [ ] **Step 4: Run**; expected PASS.

### Task 6: Threepeater

**Files:**
- Modify: `crates/pvz/src/board/plant.rs` (`attack_rect`, `find_target_zombie`/`find_target_and_fire`, `fire`: for `SeedType::Threepeater` consider rows `row-1..=row+1` clamped to existing rows; emit one `PlantAction::Shoot` per row, y adjusted by the row spacing)
- Test: `crates/pvz/tests/plants.rs`

**Interfaces:**
- Consumes: `PlantAction::Shoot { x, y, row, render_order, projectile, short_range }`; the board's row count for the level (`Board` field or `MAX_GRID_SIZE_Y`; the plant only needs the row bounds, pass them through `update_abilities` as it does `row_y`).
- Produces: nothing.

- [ ] **Step 1: Write failing tests**: `threepeater_fires_in_three_rows` (plant 2,2; zombie row 1 x 700; `run_until(300, !projectiles.is_empty())`; assert the projectile count in rows 1, 2, 3 each > 0 after the first volley), `threepeater_on_the_top_row_only_fires_two_rows` (plant row 0; assert no projectile has `row < 0` and rows 0 and 1 appear).
- [ ] **Step 2: Run**; expected FAIL.
- [ ] **Step 3: Implement** per `Plant::Fire` (threepeater branch) and `FindTargetZombie` row handling in the reference.
- [ ] **Step 4: Run**; expected PASS.

### Task 7: Gating and docs

**Files:**
- Modify: `crates/pvz/src/board/seed_chooser.rs` / `plant.rs` (the "not ported yet" gating: find where unported plants are excluded from the chooser/conveyor, e.g. the mini-boss belt's "only plants that are ported" and `seed_not_recommended`), `agents.md`, `docs/architecture.md`
- Test: `crates/pvz/tests/levels.rs` if gating is testable there

**Interfaces:** consumes all earlier tasks' plants.

- [ ] **Step 1:** Find how ported plants are decided (`grep -n "ported\|is_ported" crates/pvz/src`). Mark squash, jalapeno, spikeweed, torchwood, threepeater and tall-nut as ported; ensure plants still unported stay excluded.
- [ ] **Step 2:** Update `agents.md` "Platform status"/architecture text (new plants, new `PlantAction`s, `ProjectileType::Fire`) and add one line to `docs/architecture.md` if the structure changed.
- [ ] **Step 3: Verify** `cargo test --workspace` (all pass, game tests run with the Mac pak) and `cargo clippy --workspace --all-targets` (no new warnings). Report results; do not commit.
