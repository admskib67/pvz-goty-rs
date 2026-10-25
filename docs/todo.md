# TODO: what is left of the game

Snapshot of branch `core` on 2026-10-25. Status comes from the code (`plant.rs`, `zombie.rs`, `board/`, `game_selector.rs`, the `TODO` comments) and from `agents.md`. Verify an item against the code before starting it. Each line is a candidate slice: follow the usual loop (read the decompilation, reimplement, add a `TestBoard` test, credit comment).

Legend: `[x]` done, `[~]` partly done, `[ ]` not started.

## 0. Where we are

- [x] Boot, setup, title, main menu, player profiles, dialogs, first-ever intro cinematic.
- [x] Adventure worlds 1 and 2 (day, night), levels 1-1 to 2-10, awards, Crazy Dave on 1-5 / 2-1 / 2-2 / 2-3+ (seed slots).
- [~] Zombies: 10 of 33 types (normal, flag, cone, bucket, screen door, newspaper, pole vaulter, football, dancer, backup dancer).
- [~] Plants: roughly 25 of 53 (see section 3).
- [~] Pool plants and land plants for world 3 (squash, jalapeno, spikeweed, torchwood, threepeater, tall-nut) are in. Lily pad, tangle kelp and sea-shroom have definitions and some code; the pool itself is not playable.

## 1. Adventure content still to build

### World 3: pool (levels 3-1 to 3-10, `BackgroundType::Pool`)
- [x] Pool terrain: water rows 3-4 (`has_pool`), plant legality per tile (lily pad, tangle kelp, sea-shroom only on water; land plants need a lily pad), grid cell water flag. Lily pad, tangle kelp and sea-shroom behave and are tested.
- [ ] Pool rendering: animated water (the intro cinematic uses a static `IMAGE_POOL`), pool shimmer, ripples, plants bobbing on water (`plant.rs` TODO), `PlantingPool` particle splash.
- [ ] Pool cleaner (lawn mower variant for water rows), its intro roll-in and trigger. Store item "Pool Cleaner" swaps it in.
- [ ] Level intro for 3-x: pan, pool-cleaner roll-in, zombies on the street, Crazy Dave talk (message 501+; `cutscene.rs` TODO "3-1's (501) and the later levels' talks").
- [ ] Zombie waves and allowed-level tables for world 3 (`gZombieAllowedLevels`, ducky tube spawn rules, water-row picking for swimmers).
- [ ] Level 3-5 (Wall-nut bowling again with pool rules), 3-10 (Zomboss-less mini boss belt), 3-x tutorials.

### World 4: fog (4-1 to 4-10, `BackgroundType::Fog`)
- [x] Night pool background with fog overlay, fog drawing and per-column fog coverage, fog fade as the level goes.
- [x] Fog interaction: Plantern clears fog in a radius, Blover blows it away for a time, fog hides zombies and plants beyond the line (balloon zombies still missing).
- [ ] Night pool grave-less rules, sun from sky off, mushrooms awake.
- [ ] Level intro for 4-x (fog descending, Crazy Dave talks), "fog" and "boss" TODOs in `cutscene.rs`.
- [ ] Seed chooser pool and fog warnings (not yet in the chooser).

### World 5: roof (5-1 to 5-10, `BackgroundType::Roof`)
- [x] Roof terrain: slope (columns 0-4 vs 5-8), flower pots required on every tile, tile-lifted drawing.
- [x] Flower pot planting (placing pots first, pot art under plants, plants 5 px higher in the pot).
- [x] Lobbed projectiles vs the slope (cabbage/kernel/melon hit rules); direct shooters keep a level flight, only their shadows follow the slope, as in the original.
- [ ] Roof level intro, rake, Crazy Dave talks, the "no sun from the sky" rule is gone on roof (sky sun returns).
- [ ] Level 5-5 conveyor / bowling variants and others per `gZombieWaves` tables.
- [ ] Craters for roof (`grid_item.rs` TODO) and pool craters.

### World 5 end: Dr. Zomboss (5-10, `BackgroundType::Boss`)
- [ ] Boss zombie: `Zombie` type `Boss`, 3 phases, head/arm animations, fireball / ice ball attacks, bungee drops, zombie summoning, rocket launch, hit boxes, defeat state.
- [ ] Boss reanimations, particles and sounds, boss level music, boss health bar / message text, boss intro (cutscene.rs TODO "the boss").
- [ ] Boss level seed chooser rules, conveyor belt, fixed roof layout, win condition.
- [ ] Final cinematic after the boss: win animation, Crazy Dave's taco / "the end" sequence, credits, trophy, return to the menu with adventure complete.
- [ ] Credits screen (`Credits`, reanim text tracks (`reanim.rs` TODO), song `ZombiesOnYourLawn.ogg` per `audio.rs` TODO), "adventure complete" message on the selector, golden sunflower trophy (`game_selector.rs` TODOs).

### Cross-world level plumbing
- [ ] Awards: levels 9, 14, 19, ... and 50 give a note, almanac, key, taco, watering can, etc. (`level_end.rs` TODO), the award screen's trophy, money bag and credits-note variants.
- [ ] Money bag to click and mower bonus coins at level end (`level_end.rs`).
- [ ] Presents, potted plants and chocolate drops from zombies.
- [ ] Huge-wave drum burst, wave text as the original's animated letters (not plain text).
- [ ] Level 3-x onward tutorial hints (`AdviceType` set is incomplete).
- [ ] Brains cutscene shake (`SetShakeOverride`, `cutscene.rs`), plant gloom/threepeater head shake.
- [ ] Mower tinting, upgrade graying (`seed_bank.rs`).
- [ ] Seed chooser extras: almanac / store / imitater buttons, pool / roof / flyer warnings.

## 2. Zombies (23 left)

Ported: normal, flag, cone, bucket, screen door, newspaper, pole vaulter, football, dancer, backup dancer.

Pool and fog:
- [x] Ducky tube zombie (normal / cone / bucket variants on water; any wading zombie wears the tube).
- [x] Snorkel zombie (swims submerged, surfaces to eat).
- [x] Zamboni (crushes plants, leaves ice trail).
- [x] Bobsled team (four zombies on a sled, slides on ice trail).
- [x] Dolphin rider (jumps pool plants).
- [ ] Jack-in-the-box (explodes, music-box sound).
- [ ] Balloon zombie (flies, popped by cactus / blover).
- [ ] Digger zombie (tunnels left, pops out behind plants).
- [ ] Pogo zombie (bounces over plants, blocked by tall-nut / pumpkin etc.).
- [ ] Yeti (rare appearance, runs off the screen).

Roof and boss:
- [ ] Bungee zombie (steals a plant, target marker, umbrella leaf defence).
- [ ] Ladder zombie (places a ladder so other zombies cross walls).
- [ ] Catapult zombie (basketballs).
- [ ] Gargantuar (smash, throws imp, giga-version with red eye `RedeyeGargantuar`).
- [ ] Imp.
- [ ] Dr. Zomboss (`Boss`).

Puzzle / "I, Zombie" heads (also used by Zen mini-games): [ ] PeaHead, WallnutHead, JalapenoHead, GatlingHead, SquashHead, TallnutHead.

Cross-cutting zombie work:
- [ ] Zombie ice trail, zombie-specific death animations, shields for pool variants.
- [ ] Water-aware movement / drawing (submerged clipping, splash on enter / exit).
- [ ] Balloon, bungee and flying targeting rules for plants.
- [ ] Remaining `gZombieAllowedLevels` tables for all 50 levels.
- [ ] Hypnotised versions of new zombies; zombies on roof (slope speed, fall).

## 3. Plants (about 28 left or incomplete)

Present (verify behaviour): peashooter, sunflower, cherry bomb, wall-nut, potato mine, snow pea, chomper, repeater, puff-shroom, sun-shroom, fume-shroom, grave buster, hypno-shroom, scaredy-shroom, ice-shroom, doom-shroom, squash, threepeater, jalapeno, spikeweed, torchwood, tall-nut, left repeater, explode-o-nut, giant wall-nut (partial).

Partly there (definition, little behaviour): lily pad, tangle kelp, sea-shroom, pumpkin, gloom-shroom, coffee bean (waking up sleeping mushrooms, `plant.rs` TODO).

Not started:
- [ ] Lily pad, tangle kelp, sea-shroom (finish), plantern (fog light), cactus (pops balloons, hits over high zombies), blover (blows fog and balloons), split pea (backward head), starfruit (five directions), pumpkin (shell over plant, repair), magnet-shroom (removes metal items), flower pot, cabbage-pult, kernel-pult (butter stun), coffee bean (finish), garlic (diverts zombies), umbrella leaf (bungee shield, lob protection), marigold (coins), melon-pult, gatling pea (upgrade of repeater), twin sunflower (upgrade), gloom-shroom (upgrade), cattail (pool homing shots), winter melon (upgrade), gold magnet (picks up coins), spikerock (upgrade), cob cannon (upgrade, aim and fire), imitater (clones any seed, grey packet), sprout.
- [ ] Upgrade plants: planting over the base plant, base plant required (`seed_bank.rs` TODO), glow when ready.
- [ ] Zombie-facing hit rules for new shooters (lobbed shots arc, ignore walls); `effected_by_damage` for new plants.
- [ ] Plant glow as zombies approach (`plant.rs` TODO), multi-head shake, twin / threepeater / split pea faces, 5-pixel lift in a flower pot.
- [ ] Each plant's Almanac entry data.

## 4. Game modes

All of these start from `AppState::GameSelector` and `GameMode` (only `Adventure` and `Intro` exist). Locked until the adventure is finished or the specific level is passed, per the original.

- [ ] Mini-games (~20, the GOTY list from `GameMode`): Zombiquarium, Beghouled, Beghouled Twist, Seeing Stars, Slot Machine, Invisi-ghoul, Bobsled Bonanza, Big Trouble Little Zombie, Wall-nut Bowling 2, Portal Combat, Column Like You See 'Em, Zombie Nimble Zombie Quick, Whack a Zombie (day and 2-5 version already exist), Last Stand, Pogo Party, Bungee Blitz, Air Raid, Ice Level, It Rains Seeds, Art Challenge Wall-nut / Sunflower, Zombotany and so on. For each: its rules, board layout, scoring, unlock.
- [ ] Pool mini-games (the same list's pool versions: Zombiquarium on pool, Big Trouble, Bobsled Bonanza, ...). Needs the pool.
- [ ] Night / fog pool mini-games (Invisi-ghoul, ...). Needs fog.
- [ ] Roof mini-games (Pogo Party, Air Raid, Bungee Blitz, ...). Needs the roof.
- [ ] Mini-game menu screen (the "Mini-games" panel on the selector, cleared padlocks, per-mini-game best score).
- [ ] Puzzle mode: Vasebreaker (9 stages + endless), I, Zombie (9 stages + endless). Vases, vase contents, "I, Zombie" brain scoring and zombie placement UI.
- [ ] Survival mode: Day, Night, Pool, Fog, Roof (normal and hard), plus endless variants. Choosing plants between rounds, "survival score".
- [ ] Zen Garden (see section 5).
- [ ] Trophies, the stats screen, and the "Ready to play" locks on the selector (`game_selector.rs` TODOs for trophy and sparkle).

## 5. Zen Garden, Tree of Wisdom, Store

- [ ] Zen Garden screen: the garden, the bonsai / mushroom / aquatic gardens, potted plants (planting, growth stages, wilting), the stinky snail and his chocolate, the watering can, fertilizer, bug spray, phonograph (zen music), glove (move plants), wheelbarrow, and "needs water" bubble on the menu (`game_selector.rs` TODO).
- [ ] Zen plant drops: presents, potted plant drops from zombies (`level_end` / `coin`), plant variants (colour variants), save of `zen_garden` plants in `PlayerInfo` (raw 88-byte structs already stored).
- [ ] Marigolds and their coin production, selling plants, plant rebinding on growth.
- [ ] Tree of Wisdom: growth stages (heights unlock), feeding with tree food, the tree's speech bubbles, tree height achievements, tree food purchase.
- [ ] Store ("Crazy Dave's Twiddydinkies"): all items (upgraded seed slots already used, plus pool cleaner, roof cleaner, rake, bug spray, fertilizer, chocolate, tree food, golden watering can, phonograph, glove, wheelbarrows, Stinky, aquarium, taco, ...), prices, item detail page, buy, closed / open states, store reachable from main menu and from the seed chooser.
- [ ] Crazy Dave's store dialogue ("Bonjour" greeting, purchase lines) and animations.

## 6. Main menu pieces

- [ ] Help: the Help dialog (`ButtonId::Help`, how-to-play text and illustrations from `properties/` and `LawnStrings`).
- [ ] Options dialog: music / sound volume sliders, 3D acceleration, full-screen, "always show seed packets" and such; mirrored in the pause menu (Menu button currently only offers "Leave Game?").
- [ ] Almanac: index with plants and zombies, detail pages with animated portrait, stats, descriptions; opens from menu, from the seed chooser and after unlock. Unlock rules per seed / zombie seen.
- [ ] Zombatar: zombie avatar editor (face, hair, eyes, mouth, clothing, accessories, background, colours), saved in the player profile, shown on the main menu sign / achievements, import image.
- [ ] Achievements (Steam GOTY list: Home Lawn Security, Nobel Peas Prize, Better Off Dead, ...): tracking, popup banner, achievements screen. Needs a per-achievement trigger audit.
- [ ] Quit confirmation and Windows-style "exit" behaviours already exist; add "Main Menu" button on the Game Over dialog (`level_end.rs` mention in `agents.md`).
- [ ] Pause menu and pause overlay (pause music, mute foley on pause, `audio.rs` TODO), "restart level" and "back to main" flows.
- [ ] Save and load a game in progress (`lawn_app.rs`, `Board::TryToSaveGame`, `player.rs` delete saved games TODO, resume from the selector).
- [ ] Adventure-complete message on the selector (`game_selector.rs` TODO).

## 7. Crazy Dave scenes (dialogue + animation)

Done: 1-5 (shovel lesson), 2-1 and 2-2 talks, 2-3+ seed-slot offers.

- [ ] 3-1 (message 501) and the other pool level talks (3-x), including "pool" explanation and the pool-cleaner reference.
- [ ] 3-5 (wall-nut bowling) talk and giant wall-nut stage (the current game lacks Dave's explanation and giant wall-nuts).
- [ ] Night pool (4-x) talks, fog explanation, plantern reference.
- [ ] Roof (5-x) talks: flower pots, catapults, the rake, final level talks before the boss.
- [ ] Post-boss talk and the final cinematic's Dave lines.
- [ ] Store talks, Zen Garden talks, Tree of Wisdom talks, mini-game talks.
- [ ] Dave's expressions / mouth animations for new lines, repeat prompts when the player returns, `{PLAYER_NAME}`-style variables for all of them.
- [ ] Levels 11+ talks not yet ported (`cutscene.rs` TODO).

## 8. Animations, particles and presentation

- [ ] Animated pool water, pool light, wave edge, drips when plants fall in.
- [ ] Fog texture and animation, fog reveal on plantern / blover.
- [ ] Roof tiles, pot art, slope shading, night glow.
- [ ] Reanimations for every new zombie and plant (wired through `ReanimationType`, already listed in full), render groups, shields, attachments (balloon string, ladder, bungee cord, catapult basket).
- [ ] Particles not yet implemented: `PlantingPool`, zombie splashes, ice trail, fire, sparks, fog puff, coin sparkle, trophy sparkle, plus the ones listed in `ParticleEffect` that no one draws yet.
- [ ] Text and "fullscreen" reanimation tracks (`reanim.rs` TODO) for the credits and boss intro.
- [ ] Animated huge-wave and final-wave banners, "zombies are coming" text.
- [ ] Plant highlight under the shovel, tooltips for new plants, seed packet visuals for upgrades.
- [ ] Final cinematic art and music cues (see section 1).

## 9. Audio

- [ ] Drum and hi-hat music bursts during levels (`audio.rs` TODO), per-world music for pool / fog / roof / boss / zen garden / minigames / credits / the store.
- [ ] Pause behaviour for sounds (MUTE_ON_PAUSE foley).
- [ ] `.au` decoder for the Windows pak (sounds like `SOUND_DIAMOND`).
- [ ] New foley types for each new plant and zombie (`FoleyType` lists only types used so far).
- [ ] Volume sliders hooked to the options dialog.

## 10. Platform, tooling, quality

- [ ] Windows: run the title screen and main menu visually, test compiled particles (`ParticleDefinition::parse_compiled`) against the Windows pak, audio on Windows.
- [ ] Linux (Wayland): deferred. libopenmpt is built; nothing tested.
- [ ] Windows pak is a different pak (compiled reanims / particles): make sure every new resource loads from both paks (`check_resources`).
- [ ] Tests: add a `TestBoard` test per new plant / zombie behaviour; extend `levels.rs` to cover pool, fog and roof setups.
- [ ] Resource-error dialog when loading fails (`title.rs` TODO), real loading-progress weights (`resources.rs` TODO).
- [ ] Update `agents.md` and `docs/architecture.md` as structures change (new `AppState`s for almanac, store, zen garden, credits, etc.).
- [ ] Packaging for distribution: ship libopenmpt `LICENSE`, no game data, attribution to the reference decomp where its logic was used.

## 11. Suggested order

1. Pool: terrain, lily pad / tangle kelp / sea-shroom, pool cleaner, water rendering, ducky tube / snorkel / dolphin / zamboni / bobsled, then 3-1..3-10 and Dave talks.
2. Remaining land plants and zombies that world 3 needs (jack-in-the-box, balloon, digger, pogo, magnet, cactus, cabbage...), awards 9 / 14 / 19.
3. Fog (plantern, blover) and world 4.
4. Roof (flower pot, lobbers, catapult, ladder, bungee, gargantuar, imp), world 5.
5. Dr. Zomboss, final cinematic, credits.
6. Main-menu features: options, help, almanac, store, then Zen Garden and the Tree.
7. Mini-games, puzzle, survival, their pool / fog / roof variants.
8. Zombatar, achievements, save / load, pause menu.
9. Windows verification, Linux, polish.
