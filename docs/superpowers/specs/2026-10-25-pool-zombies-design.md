# Pool zombies: design

Date: 2026-10-25. Branch: `core`.

## Goal

Make the pool world (levels 3-1 to 3-10, then the fog levels that share the pool) send and fight the zombies that belong there, so the pool plants already ported (lily pad, tangle kelp, sea-shroom) have something to do. Every behaviour is read from the reference and covered by a `TestBoard` test.

Zombies in scope: ducky tube (plain, cone and bucket variants), snorkel, dolphin rider, Zamboni, bobsled team. Pool lane plumbing comes first because they all need it.

## Out of scope

Pool cleaner, animated water, jack-in-the-box, balloon, digger, pogo, yeti, roof zombies, the pool/fog Crazy Dave talks, the 3-10 conveyor weights, bungee/ladder/catapult/gargantuar/imp/boss.

## Rules for the work

- Behaviour is read from the ruslan831 decomp (`Zombie::UpdateZombiePool`, `CheckForPool`, `PoolSplash`, `ZombieTypeCanGoInPool`, `UpdateZombieSnorkel`, `UpdateZombieDolphinRider`, `UpdateZamboni`, `UpdateZombieBobsled`, `BobsledCrash`, `ZamboniDeath`) and reimplemented, with a credit comment naming the function (`agents.md`).
- Zombies stay decoupled from the board: they see `ZombieEnv` and report through the existing request/`Fx` mechanisms.
- One vertical slice per zombie, test first. Shared pieces are added by the slice that needs them first.
- No commits. `readme.md` untouched.

## Design

**Height and water state.** Add to `Zombie` the original's `ZombieHeight` (normal, into pool, out of pool, dragged under, falling) and `in_pool`, using the existing `altitude` field. `Zombie::update_zombie_pool` ports `UpdateZombiePool`: sinking to `-40` on entry (zombie draws lowered and clipped at the waterline), rising back on exit. `check_for_pool` ports `CheckForPool`: a zombie whose body squares are water (columns at `x+75` and `x+45` in a pool row, `x < 680`) splashes in; leaving the water it splashes out. `PoolSplash` plays the `Splash` reanimation, `PlantingPool` particle and foley. The ice-trap shortcut for swimmers is kept as in the original.

**Who can swim.** `ZombieType::can_go_in_pool` ports `ZombieTypeCanGoInPool` (normal, cone, bucket, flag, snorkel, dolphin, and later balloon and puzzle heads). Others keep walking dry-shod and can't be placed in pool rows.

**Waves.** Pool rows (rows 2 and 3, `PlantRowType::Pool`) are currently excluded from `pick_row_for_new_zombie` (`waves.rs:444`). Swimmers (ducky tube, snorkel, dolphin) are picked only for pool rows; land zombies keep to grass rows; the Zamboni and bobsled use the ice rows per the original's rules. Allowed-level tables for the new types are added to `zombie.rs` (`gZombieAllowedLevels`), and `is_implemented` gains each type as its slice lands.

**Ducky tube.** Reuses the normal zombie's body with the `Zombie_ducky_tube` overlay (`ReanimationType` already lists it). Swims at the ordinary walking speed in pool rows, drawn at the waterline; cone/bucket variants add their armour as now. Walks onto land and drops the tube like the original.

**Snorkel.** Phases `SnorkelWalking → IntoPool → WalkingInPool → UpToEat → EatingInPool → DownFromEat`. While swimming it is submerged and untargetable by shooters (`effected_by_damage` rejects it unless eating); it surfaces to bite. Leaves the pool at `x <= 25`.

**Dolphin rider.** Phases `Walking → IntoPool → Riding → InJump → WalkingInPool → WalkingWithoutDolphin`. Jumps the first plant in reach like the pole vaulter (shares the tall-nut block and `FindPlantTarget` vault logic with the vaulter), lands behind it and swims on without the dolphin. A tangle kelp target is left alone.

**Zamboni.** Heavy, slow, crushes plants it touches, leaves an ice trail (`ice_min_x`, `ice_timer` per row on the board) and dies with `ZamboniDeath`. **Bobsled team.** Four zombies on a sled, rides the ice trail, `BobsledCrash` when the sled is destroyed. Both depend on the ice-trail state added with the Zamboni; the bobsled slice reuses it.

**Drawing.** Swimmers draw lowered by `altitude` and clipped at the waterline with the existing clip-rect support (`Graphics::clip_rect`), as zombies rising from graves already are.

## Slices (in order)

1. Pool-lane plumbing: heights, `check_for_pool`, `update_zombie_pool`, `can_go_in_pool`, pool-row picking, tests with a plain zombie walking into the pool (sinks, slows nothing, comes out).
2. Ducky tube zombie (plus cone/bucket variants).
3. Snorkel.
4. Dolphin rider.
5. Zamboni, then bobsled.
6. Gating and docs: `is_implemented` and allowed-level tables per slice, then `agents.md`, `docs/architecture.md`, `docs/todo.md`.

## Testing

Tests go in `crates/pvz/tests/zombies.rs` on `test_board!("3-1")`, seeded RNG, skipped without the game pak. `cargo test --workspace` and `cargo clippy --workspace --all-targets` must pass after each slice.

## Open details

Exact timings, offsets, reanimation track names and sound ids are read from the reference when each slice is written; any difference found there overrides the numbers here. The Zamboni and bobsled details (crush rules, sled health, ice-trail drawing) are read when slice 5 starts.
