# Status and TODO

Snapshot of branch `core` on 2026-10-26, from the code (`TODO` comments, `is_implemented`, `GameMode`, `AppState`) and from `agents.md`. **Check an item against the code before you start on it**, and update this file when you finish or find one.

## What works (macOS, Mac pak)

- Boot, first-run setup, title screen, main menu, player profiles, every main-menu dialog, options (volumes, full screen), help note, almanac, store, credits (from the options' Credits button or after the boss).
- **Adventure 1-1 to 5-10 end to end**: day, night (graves), pool, fog, roof, and the final boss (Dr. Zomboss). Level intros, seed chooser with all of `OnStartButton`'s warnings, conveyor levels (1-5 bowling, 1-10/2-10/3-10 mini-bosses, 3-5 Little Trouble, 5-10 boss belt), whack-a-zombie (2-5), awards and award screens, trophy, credits note, money bag on replays, mowers → coins at level end, losing and Game Over.
- **Plants: all 49 adventure/store plants** (every `SeedType` except Zen Garden's `Sprout`), including upgrades (gatling, twin sunflower, gloom, cattail, winter melon, gold magnet, spikerock, cob cannon) and the imitater.
- **Zombies: 21 of 33 types**: normal, flag, cone, bucket, pole vaulter, newspaper, screen door, football, dancer, backup dancer, ducky tube, snorkel, Zamboni, bobsled, dolphin rider, balloon, bungee, ladder, catapult, gargantuar, imp, Dr. Zomboss.
- Crazy Dave talks on 1-5, 2-1, 2-2, 2-3+ (seed slots), 2-5, 3-1, 3-5, 4-1, 5-1, 5-10.
- **Saving a level in progress** (`board::save`, `tests/save.rs`): leaving through the menu, or quitting the app, saves an adventure level in progress (`TryToSaveGame`/`NeedSaveGame`, `userdata/game{id}_{mode}.dat` as TOML); Adventure offers the Continue dialog (resume, or erase and restart); winning, losing, restarting or starting fresh erases it; `--lvl` copies never save; deleting a profile deletes its saves. Not saved: reanimation/particle phases and peas in flight (rebuilt fresh on load).
- Windows: builds, setup screen and resource loading verified; everything else untested there.

## Missing: large pieces

| Area | Notes |
|------|-------|
| **Game modes** | `GameMode` has only `Adventure` and `Intro`. Missing: mini-games (~20), puzzle (Vasebreaker, I, Zombie), survival (day/night/pool/fog/roof, normal/hard/endless), their selector panels, locks and best scores. |
| **Zen Garden** | garden screens, potted plants and their growth/watering, Stinky, tools, marigold sales, "needs water" bubble on the menu. `PlayerInfo` already stores Zen plants as raw 88-byte structs. |
| **Tree of Wisdom** | growth, tree food, speech. The store records buying the tree but can't go there. |
| **Zombies (12 types)** | Jack-in-the-box, digger, pogo, yeti, red-eye gargantuar, the six Zombotany heads (PeaHead, WallnutHead, JalapenoHead, GatlingHead, SquashHead, TallnutHead). Wire each in `zombie.rs::reanim()`; the almanac already shows still frames of the first four. |
| **Achievements** | none. |
| **Windows/Linux verification** | Windows: title/menu/board never run visually; compiled particles untested; audio untested; `.au` sounds silent (needs a decoder, `audio.rs:703`). Linux: nothing run. |

## Missing: features inside existing screens

### Board and levels
- Presents, potted plants and chocolate drops from zombies.
- Huge-wave drum burst (`waves.rs:788`, `audio.rs:320`) and the original's animated wave/huge-wave lettering (`board/message.rs:33`; plain text now).
- Brains cutscene shake (`cutscene.rs:346`), fog and boss specifics in the intro timeline (`cutscene.rs:215`), the rake (`cutscene.rs:372`).
- Level 1 tutorial's flashing sod row (`board/mod.rs:2235`); whack-a-zombie's tutorial; the water-planting advice (`ADVICE_LILYPAD_ON_WATER` etc.).
- Crazy Dave's talks from level 11 on, except those listed above; 1-5's explanation and giant wall-nuts.
- Street zombie preview grid (`CutScene::PlaceStreetZombies`); placement is a guess now.
- The house door overlay on a lost level (`DrawHouseDoorTop`); the Main Menu button on the Game Over dialog (`board/mod.rs:1936`).
- Hand cursor over clickable things (`board/mod.rs:1862`), plant highlight under the shovel, cursor preview bob over water.
- Bonus mowers rolling back in (`lawn_mower.rs:397`).
- Pool and roof gravestone and crater variants (`grid_item.rs:303`, `grid_item.rs:341`); 4-5 at night (`board/mod.rs:619`).
- Seed packet visuals: explode-o-nut tint, left repeater (`seed_bank.rs:456`); packet starburst on pick-up (`cursor.rs:573`); award coin extras (`coin.rs:538`).
- Ice-shroom pausing the pool water's counter.
- Hypnotised zombies' shadow offsets differ from `Zombie::DrawShadow` (football about 52 px off; table in `board/zombie.rs` `draw_shadow`).
- Tiny zombies' particles not scaled (`OverrideParticleScale`).

### Plants
- Plant glow as a zombie nears (`board/plant.rs:922`), multi-head shake (`:1206`), several faces for threepeater/split pea/twin sunflower (`:2605`).
- Remaining `PlantInitialize` per-plant setup (`:624`); other shooters' fire timings (cactus 35..., `:2197`); spikeweed height by row/stage (`:355`).
- Instant kills only hurting gargantuars and the boss (`:966`).
- Magnet-shroom pulling ladders propped on plants (`:1321`) and loot from unported zombies; gold magnet waiting for another (`:1371`); cob flashing while it waits (`:1443`); a pumpkin over a kernel-pult invalidating a cannon spot (`cursor.rs:328`); coffee bean's wake-up squash; attached particle (`:1794`).
- Lobbers' upgrades; gargantuar's ice-trap size and smashing mind-controlled targets.

### Zombies
- Dance mode `anim_dance` (`board/zombie.rs:1519`); loot by zombie value in places (`:2327`); rising out of pool/roof graves (`:2897`).
- Boss: a replayed boss level should skip Dave after a loss; pots' and fireballs' smoke trails should go when he dies.

### Menus and screens
- Main menu: trophy and its sparkle (`game_selector.rs:374`, `:828`), adventure-complete message (`:625`), Zen "needs water" bubble (`:892`), hand clipping (`:895`), the other mode buttons.
- Options: pausing music and MUTE_ON_PAUSE foley (`audio.rs:782`), volume changes for sounds already playing.
- Store: Zen items (potted marigolds, `store.rs:306`, `:862`), going to the tree (`:746`), "coming soon" rule (`:188`), bonus mower and chocolate pages, award-screen intro talks, night background, opening from the seed chooser.
- Seed chooser: almanac and store buttons.
- Almanac: pool water behind water plants (`almanac.rs:508`), live portraits of unported zombies (`:623`).
- Credits: the shaky text (`credits.rs:386`).
- Title: resource error dialog (`title.rs:118`); loading progress weights for reanims/particles (`resources.rs:364`).
- Widgets: vertical sliders, clipboard/undo/password in `EditWidget`.

### Engine
- Reanimation track shake (`mShakeOverride`) and the remaining attacher/attachment cases in `sexy::reanim` (`reanim.rs:530`).
- Particle `DieIfOverloaded` (`sexy/src/particle.rs:807`).
- Music drum/hi-hat bursts during levels; per-mode music (Zen Garden, mini-games).

## Suggested order

1. The remaining zombies (jack-in-the-box, digger, pogo, yeti, red-eye gargantuar), since modes need them.
2. Presentation gaps on the board (animated wave text, drum burst, hand cursor, glow, faces).
3. Survival and mini-games (reuse `Board` with a `GameMode` per mode), then puzzle.
4. Zen Garden and Tree of Wisdom, then the store's Zen items.
5. Achievements; Windows visual pass and `.au` decoder; Linux.

## Code TODO index

Run this for the live list:

```bash
grep -rn -E "TODO|Not ported|not ported" crates --include='*.rs'
```

Most entries above carry their `file:line` from the 2026-10-26 snapshot; lines drift as code changes.
