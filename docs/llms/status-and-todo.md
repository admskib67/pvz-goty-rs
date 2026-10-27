# Status and TODO

Snapshot of branch `core` on 2026-10-26, from the code (`TODO` comments, `GameMode`, `AppState`) and from `agents.md`. **Check an item against the code before you start on it**, and update this file when you finish or find one.

## What works (macOS, Mac pak)

- Boot, first-run setup, title screen, main menu, player profiles, every main-menu dialog, options (volumes, full screen), help note, almanac, store, credits (from the options' Credits button or after the boss).
- **Adventure 1-1 to 5-10 end to end**: day, night (graves), pool, fog, roof, and the final boss (Dr. Zomboss). Level intros, seed chooser with all of `OnStartButton`'s warnings, conveyor levels (1-5 bowling, 1-10/2-10/3-10 mini-bosses, 3-5 Little Trouble, 5-10 boss belt), whack-a-zombie (2-5), awards and award screens, trophy, credits note, money bag on replays, mowers → coins at level end, losing and Game Over.
- **Plants: all 49 adventure/store plants** (every `SeedType` except Zen Garden's `Sprout`), including upgrades (gatling, twin sunflower, gloom, cattail, winter melon, gold magnet, spikerock, cob cannon) and the imitater.
- **Zombies: all 33 types**: normal, flag, cone, bucket, pole vaulter, newspaper, screen door, football, dancer, backup dancer, ducky tube, snorkel, Zamboni, bobsled, dolphin rider, balloon, bungee, ladder, catapult, gargantuar, imp, Dr. Zomboss, plus jack-in-the-box (runs, pops, explodes), digger (tunnels, rises, loses its axe to magnets), pogo (bounces, vaults, tall-nut breaks it), yeti (flees), red-eyed gargantuar (6000 HP), and the six Zombotany heads (pea/gatling shoot backwards, jalapeno burns its row, squash jumps, wall/tall-nuts are helmets).
- Crazy Dave talks on 1-5, 2-1, 2-2, 2-3+ (seed slots), 2-5, 3-1, 3-5, 4-1, 5-1, 5-10.
- **Content pass**: the mode-unlocking presents (3-2's mini-games, 4-6's puzzle), the pool/roof/night crater art, 4-5 at night (stormy night is 4-10, not 4-5), ice-shrooms freezing the pool water, red-tinted explode-o-nut and mirrored left-repeater seed art, the award's starburst and pickup particles, the zombies' full shadow-offset table (a backward-facing football was 52 px off) with the pool's water shadow, tiny zombies' particle scaling, and the boss level skipping Dave when it's retried after a loss.
- **Presentation pass**: the huge-wave banner's letter animations and the music's drum burst, the hand cursor (hidden for the hammer), the shovel's plant highlight, the cursor preview's water bob, the selector hand's clip, the rake (store item: swings at the first zombie), bonus mowers rolling back in, the Game Over dialog's top-right Main Menu button and the door overlay on a lost level, the level-1 sod-row flash, the whack-a-zombie packet tutorial, the fog roll-in and 3-5/4-5 talks, and the doom-shroom's shaking heads.
- **Saving a level in progress** (`board::save`, `tests/save.rs`): leaving through the menu, or quitting the app, saves an adventure level in progress (`TryToSaveGame`/`NeedSaveGame`, `userdata/game{id}_{mode}.dat` as TOML); Adventure offers the Continue dialog (resume, or erase and restart); winning, losing, restarting or starting fresh erases it; `--lvl` copies never save; deleting a profile deletes its saves.
- Windows: builds, setup screen and resource loading verified; everything else untested there.

## Missing: large pieces

| Area | Notes |
|------|-------|
| **Game modes** | `GameMode` has only `Adventure` and `Intro`. Missing: mini-games (~20), puzzle (Vasebreaker, I, Zombie), survival (day/night/pool/fog/roof, normal/hard/endless), their selector panels, locks and best scores. |
| **Zen Garden** | garden screens, potted plants and their growth/watering, Stinky, tools, marigold sales, "needs water" bubble on the menu. `PlayerInfo` already stores Zen plants as raw 88-byte structs. |
| **Tree of Wisdom** | growth, tree food, speech. The store records buying the tree but can't go there. |
| **Achievements** | none. |
| **Windows/Linux verification** | Windows: title/menu/board never run visually; compiled particles untested; audio untested; `.au` sounds silent (needs a decoder, `audio.rs:703`). Linux: nothing run. |

## Missing: features inside existing screens

### Board and levels
- Potted plants and chocolate drops from zombies (Zen Garden loot; the mini-games and puzzle presents drop and unlock their modes already).
- The water-planting advice variants (`ADVICE_LILYPAD_ON_WATER` etc.).

### Plants
- Remaining `PlantInitialize` per-plant setup (`:624`); other shooters' fire timings (cactus 35..., `:2197`); spikeweed height by row/stage (`:355`).
- Instant kills only hurting gargantuars and the boss (`:966`).
- Magnet-shroom pulling ladders propped on plants (`:1321`) and loot from unported zombies; gold magnet waiting for another (`:1371`); cob flashing while it waits (`:1443`); a pumpkin over a kernel-pult invalidating a cannon spot (`cursor.rs:328`); coffee bean's wake-up squash; attached particle (`:1794`).
- Lobbers' upgrades; gargantuar's ice-trap size and smashing mind-controlled targets.

### Zombies
- `anim_dance`: the original's typed cheat-code dance mode (`mDanceMode`), not a game mode; the cheat isn't ported.
- Boss: the pots' and fireballs' smoke trails (particles attached to reanimation tracks, `ApplyBossSmokeParticles`).
- Rising out of pool/roof graves: graves only stand on grass or high ground, so adventure content never grows one on water; the decomp's pool branch is unused here.

### Menus and screens
- Main menu: trophy and its sparkle (`game_selector.rs:374`, `:828`), adventure-complete message (`:625`), Zen "needs water" bubble (`:892`), the other mode buttons.
- Options: pausing music and MUTE_ON_PAUSE foley (`audio.rs:782`), volume changes for sounds already playing.
- Store: Zen items (potted marigolds, `store.rs:306`, `:862`), going to the tree (`:746`), "coming soon" rule (`:188`), bonus mower and chocolate pages, award-screen intro talks, night background, opening from the seed chooser.
- Seed chooser: almanac and store buttons.
- Almanac: pool water behind water plants (`almanac.rs:508`).
- Title: resource error dialog (`title.rs:118`); loading progress weights for reanims/particles (`resources.rs:364`).
- Widgets: vertical sliders, clipboard/undo/password in `EditWidget`.

### Engine
- Per-mode music (Zen Garden, mini-games).

## Suggested order

1. Survival and mini-games (reuse `Board` with a `GameMode` per mode), then puzzle.
2. Zen Garden and Tree of Wisdom, then the store's Zen items.
3. Achievements; Windows visual pass and `.au` decoder; Linux.

## Code TODO index

Run this for the live list:

```bash
grep -rn -E "TODO|Not ported|not ported" crates --include='*.rs'
```

Most entries above carry their `file:line` from the 2026-10-26 snapshot; lines drift as code changes.
