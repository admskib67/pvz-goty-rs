# World 3 (levels 3-1 to 3-10): design

Part of the roadmap "finish the base game one world at a time" (world 3, world 4, world 5, upgrade plants, base-game audit; then Zomboss, Zen Garden, Tree of Wisdom, minigames, Zombatar). This spec covers world 3 only. Each world gets its own spec, plan and implementation cycle.

## Goal

Levels 3-1 to 3-10 play from the seed chooser to the award, the way the original does: pool lawn with working water, intros and Dave talks, zombie waves, pool cleaner, special levels, awards. "Working" means a `TestBoard` test per behaviour (the project's usual loop) plus a screenshot check of each new screen.

## Facts found while scoping (the roadmap needs these)

- The plants awarded in world 3 (lily pad, squash, threepeater, tangle kelp, jalapeno, spikeweed, torchwood, tall-nut) are all ported. The "remaining plants" (pumpkin, garlic, magnet-shroom, coffee bean, cactus, split pea, starfruit, ...) are awards of worlds 4 and 5, so they belong to those worlds' specs, not this one.
- Zombies by `starting_level`: snorkel 23, zamboni and bobsled 26, dolphin rider 28 are ported; jack-in-the-box starts at 31 (world 4), balloon 33, digger 36, pogo 38, yeti 40. World 3 needs no new zombie type.
- Some adventure levels are mini-game variants and are part of the base game: 3-5 is "Big Trouble Little Zombie" (`IsLittleTroubleLevel`, level 25), 4-5 is the scary potter / vasebreaker level (35), 4-10 is the stormy night (40), 5-5 is the bungee level (45), 3-10 is the conveyor mini boss (30), 5-10 the boss. This reorders part of the roadmap: each world's spec includes its special level.

## Scope (in order; each item is a slice with tests)

1. **Waves and level rules for 21-30.** Check `gZombieAllowedLevels` tables for ducky tube, snorkel, dolphin, zamboni and bobsled, the flag wave and big-wave rules on pool lawns, water-row picking (`row_can_have_zombie_type`, already partly there), and the end-of-level rules the code marks TODO (`board/zombie.rs`: pool lawns end differently). Test: a seeded 3-x level spawns only allowed types and swimmers only in water rows.
2. **Pool water.** Port `PoolEffect` (the original's animated, caustic-lit water, drawn in place of the static `IMAGE_POOL`) and plants bobbing on water (`board/plant.rs` TODO), the planting splash (`PlantingPool` particle) and the water sounds (`board/cursor.rs` TODO). Test: bob offset is a pure function of time and tested; the water is checked by screenshot.
3. **Pool cleaner.** `LawnMower` variant for water rows (`PoolCleaner`), its roll-in during the intro, its trigger and run, lawn mowers that reach water (`board/lawn_mower.rs` TODO), and the store's pool cleaner purchase swapping it in (`purchases[STORE_ITEM_POOL_CLEANER]`, already recorded by the store).
4. **3-1 to 3-10 intros and Dave talks.** The 3-1 talk (messages 501+), the pool intro pan and street zombies, the pool-cleaner roll-in ordering, `crazy_dave_dialog_start` for world 3 (`board/cutscene.rs` TODO), the tutorial hints that fire on world 3 levels (`AdviceType`).
5. **Special levels 3-5 and 3-10.** 3-5, Big Trouble Little Zombie (rules from `Challenge.cpp`: tiny zombies and plants, fixed seeds, its own waves). 3-10, the conveyor mini boss with pool plants (`board/conveyor.rs` TODO), the belt's list of ported plants widening to include them.
6. **Awards and level end for 3-x.** The award at each level (note at 3-10, tall-nut etc.), money bag, mower bonus coins on lawns without a mower in the pool, "zombies reach the house" rules in water rows. Test: finishing a level updates the profile level and award.
7. **World 3 audit.** Play 3-1 to 3-10 with a script of `TestBoard` runs (each level reaches its last wave and wins with a simple defence) and fix what breaks.

## Not in this spec

World 4 and 5 content, Zomboss, Zen Garden, minigame menu modes, the seed chooser's pool/roof warnings (world 4 spec), the store's cleaner effects beyond swapping the mower.

## Method

For each slice: read the matching decompilation functions (shallow clone in a temp folder, `grep -a`), reimplement in idiomatic Rust with a credit comment, add `TestBoard` tests (`test_board!("3-1")` etc.), run `cargo test --workspace` and `cargo clippy --workspace --all-targets`, screenshot where visual (`PVZ_AUTO_START=adventure ... -- --lvl 3-1`, frame ~400-500), update `agents.md` and `docs/todo.md`. Never commit.

## Corrections found during implementation

- Pool cleaner: per `LawnMower::LawnMowerInitialize` a pool row gets a pool cleaner only if the player bought it in the store; otherwise it gets an ordinary lawn mower that sinks in the water. (Earlier text saying pool rows have no mower, or that the cleaner is free, was wrong.)
- Walk into the house on the pool lawn: y stays 290; only x shifts (-80, -10 for football).
