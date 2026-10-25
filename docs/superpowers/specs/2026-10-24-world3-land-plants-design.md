# World 3 land plants and pole vaulter gaps: design

Date: 2026-10-24. Branch: `core`.

## Goal

Make the plants that work on plain grass in adventure world 3 (squash, threepeater, jalapeno, spikeweed, torchwood, tall-nut) fully playable, and close the pole vaulter's gaps (tall-nut block, footstep/landing dust). Each behaviour matches the original and is covered by a `TestBoard` test (`crates/pvz/tests/`).

## Out of scope

Pool and water, lily pad, tangle kelp, imitater, world 4+ plants (cactus, blover, starfruit, magnet, pumpkin, split pea, plantern, sea-shroom), the spikerock upgrade, new levels. The seed chooser, award flow and level data do not change except for the final gating step.

## Rules for the work

- Behaviour is read from the reference decompilation (`Plant::UpdateSquash`, `FindSquashTarget`, `DoSquashDamage`, `UpdateSpikeweed`, `SpikeweedAttack`, `UpdateTorchwood`, `Zombie` pole-vault code) and reimplemented in idiomatic Rust, with a credit comment naming the function.
- Plants stay decoupled from the board: they emit `PlantAction`s that `Board` carries out.
- One vertical slice per plant, test first. Shared pieces are added only by the slice that needs them.
- No commits (see `agents.md`). `readme.md` is untouched.

## Slices (in order)

1. **Tall-nut and pole vaulter.** Tall-nut already plants and is eaten like a wall-nut. A vaulting zombie that finds a tall-nut in its way is blocked (bonk sound, `TallNutBlock` particle, loses its pole) and walks on instead of jumping, per the reference's `Zombie` vault code. Add the vaulter's footstep and landing dust. Tests: vaulter jumps a wall-nut but not a tall-nut.
2. **Squash.** States: idle, look (80 ticks, looks left or right at the target), jump, land, die. `FindSquashTarget`: nearest zombie in its row, with a head, within about 70 px (110 if eating) of its attack rect; vaulters before the vault and walkers qualify, vaulting, snorkel and dolphin phases don't. On landing `DoSquashDamage` deals 1800 damage to zombies overlapping the attack rect (football gets 20 px of extra reach). New `PlantAction::Squash`; landing shake and particle. Tests: kills a normal zombie in range, ignores another row, ignores a zombie too far.
3. **Jalapeno.** Fuse like cherry bomb, then burns every zombie in the row. New `PlantAction::Jalapeno` with the flame-row effect, reusing the cherry bomb burn path and charred death. Tests: row burned, other rows untouched.
4. **Spikeweed.** Ground plant that zombies walk over and eat. Attacks when a zombie is in its row and in reach: `anim_attack`, then `DoRowAreaDamage(20)` at countdown 75 (flags 33), 100-tick cycle, back to idle. Uses the existing fume-shroom `RowAreaDamage` action. Planting and cursor rules already exist. Tests: damages a zombie on it repeatedly, ignores one outside its square.
5. **Torchwood.** `UpdateTorchwood`: a pea overlapping its attack rect by at least 10 px in its row becomes a fire pea (`ProjectileType::Fire`, double damage, splash to neighbours, `ConvertToFireball`); a snow pea becomes a normal pea. Tests: pea converts, snow pea thaws, other-row pea unchanged.
6. **Threepeater.** One pea in each of three rows (own, above, below, where they exist). Fires when a zombie is in any of the three rows. Extends the shooter code. Tests: peas appear in all three rows; edge rows only fire the rows that exist.
7. **Gating and docs.** Review which plants are offered by `seed_chooser`/`award_seed_for_level` so an unfinished plant is never handed out; update `agents.md` and `docs/architecture.md`.

## Testing

Each plant's tests live in `crates/pvz/tests/plants.rs` (or `zombies.rs` for the vaulter), use `test_board!` and seeded RNG, and print "skipped" without the game pak. `cargo test --workspace` and `cargo clippy --workspace --all-targets` must pass at the end.

## Open details

Exact timings, damage flags, particle names and reanimation track names are read from the reference when each slice is written; any difference found there overrides the numbers above.
