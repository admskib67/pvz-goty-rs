# Status and TODO

Snapshot of branch `core` on 2026-10-26, from the code (`TODO` comments, `GameMode`, `AppState`) and from `agents.md`. **Check an item against the code before you start on it**, and update this file when you finish or find one.

## What works (macOS, Mac pak)

- Boot, first-run setup, title screen, main menu, player profiles, every main-menu dialog, options (volumes, full screen), help note, almanac, store, credits (from the options' Credits button or after the boss).
- **Adventure 1-1 to 5-10 end to end**: day, night (graves), pool, fog, roof, and the final boss (Dr. Zomboss). Level intros, seed chooser with all of `OnStartButton`'s warnings, conveyor levels (1-5 bowling, 1-10/2-10/3-10 mini-bosses, 3-5 Little Trouble, 5-10 boss belt), whack-a-zombie (2-5), awards and award screens, trophy, credits note, money bag on replays, mowers → coins at level end, losing and Game Over.
- **Plants: all 49 adventure/store plants** (every `SeedType` except Zen Garden's `Sprout`), including upgrades (gatling, twin sunflower, gloom, cattail, winter melon, gold magnet, spikerock, cob cannon) and the imitater.
- **Zombies: all 33 types**: normal, flag, cone, bucket, pole vaulter, newspaper, screen door, football, dancer, backup dancer, ducky tube, snorkel, Zamboni, bobsled, dolphin rider, balloon, bungee, ladder, catapult, gargantuar, imp, Dr. Zomboss, plus jack-in-the-box (runs, pops, explodes), digger (tunnels, rises, loses its axe to magnets), pogo (bounces, vaults, tall-nut breaks it), yeti (flees), red-eyed gargantuar (6000 HP), and the six Zombotany heads (pea/gatling shoot backwards, jalapeno burns its row, squash jumps, wall/tall-nuts are helmets).
- Crazy Dave talks on 1-5, 2-1, 2-2, 2-3+ (seed slots), 2-5, 3-1, 3-5, 4-1, 5-1, 5-10.
- **Tree of Wisdom** (`tree_of_wisdom`): fed by tree food (the store grants five with the tree), grows through `anim_grow1..50`, babbles `[TREE_OF_WISDOM_N]` advice in its speech bubble, and is reachable from the store and the garden's Next button.
- **Zen Garden** (`zen_garden`): all three gardens (main greenhouse, mushroom garden, aquarium) with their grids and backgrounds; potted plants from the profile, two starter marigolds when empty (or Dave's tutorial at level 45); watering→fertilizer growth (sprout→small→medium→full); water/fertilizer/bug-spray/phonograph needs with bubbles; chocolate's doubled production; coins and the coin bank; selling at the original prices; Stinky wandering to coins; the wheelbarrow for moving plants between gardens; the Zen shop page; the menu's "needs water" waterdrop; and saving back to `userdata`. Timers are scaled down (a need every 30 s, coins every 60 s).
- **Menus pass**: the sunflower trophy with its sparkle on the main menu, the first-time adventure-complete message, the store's night background, live SFX volume changes (the sounds already playing follow the slider) and foley muting while the board is paused, the seed chooser's Almanac button, the almanac's animated pool water, a resource-error screen instead of an instant exit, and Ctrl+Z in the name box.
- **Plant pass**: the chomper bites gargantuars and the boss for 40 instead of swallowing them (and misses bouncing pogos); the spikeweeds' height follows the row and stage; the magnet-shroom pulls ladders off plants and the gold magnets take turns; the loaded cob cannon's cob flashes; a plant being woken by a coffee bean squashes and bounces back; a plant flashes while an upgrade for it (or a cob cannon over its kernel-pult) is held over it; garlic has its 400 health; the left repeater fires its second pea; the gargantuars' ice trap is drawn larger and a hypnotised gargantuar smashes the zombie in front of it; and the missing planting advice keys (`LILYPAD_ON_WATER`, `TANGLEKELP_ON_WATER`, `SEASHROOM_ON_WATER`, `POTATO_MINE_ON_LILY`, `PLANTING_NEEDS_GROUND`, `PLANTING_NEED_SLEEPING`, `CANT_PLANT_THERE` on pool craters).
- **Content pass**: the mode-unlocking presents (3-2's mini-games, 4-6's puzzle), the pool/roof/night crater art, 4-5 at night (stormy night is 4-10, not 4-5), ice-shrooms freezing the pool water, red-tinted explode-o-nut and mirrored left-repeater seed art, the award's starburst and pickup particles, the zombies' full shadow-offset table (a backward-facing football was 52 px off) with the pool's water shadow, tiny zombies' particle scaling, and the boss level skipping Dave when it's retried after a loss.
- **Presentation pass**: the huge-wave banner's letter animations and the music's drum burst, the hand cursor (hidden for the hammer), the shovel's plant highlight, the cursor preview's water bob, the selector hand's clip, the rake (store item: swings at the first zombie), bonus mowers rolling back in, the Game Over dialog's top-right Main Menu button and the door overlay on a lost level, the level-1 sod-row flash, the whack-a-zombie packet tutorial, the fog roll-in and 3-5/4-5 talks, and the doom-shroom's shaking heads.
- **Survival** (`board::survival`, `challenge.rs`, `tests/survival.rs`): the challenge screen's survival page (from the main menu once adventure is done) with the five lawns × normal/hard/endless, trophy-based unlocking (the first three lawns are free, each trophy opens one more, endless waits for ten) and the `0/10` tally. A run is five stages on normal and hard (ten and twenty waves per stage, stage points scaling with `mSurvivalStage`) and endless never ends; clearing a stage shows the "more zombies" advice and starts the next stage on the same lawn with the plants kept; the last stage drops the silver-sunflower trophy and returns to the challenge screen; losing an endless run records its flags (`userdata/challenge{id}.dat`). The stage tables follow `InitZombieWavesSurvival` (normals and cones on stage one, pails from hard; random kinds later, normal stops at the snorkel, no bungees outside endless/roof, no Zamboni on the night lawn). Survival runs are not saved or resumed.
- **Mini-games** (`board/minigame.rs`, `challenge.rs`, `tests/minigame.rs`): the challenge screen's Mini-games page, opened from the main menu (the page's own `0/20` trophy tally and 20 tiles), with all twenty challenge-page mini-games playable: War and Peas 1 and 2 (zombotany-only tables, no ordinary or flag zombies), Wall-nut Bowling 1 and 2 (the adventure bowling lawn, 20/30 waves), Big Trouble Little Zombie (its conveyor), Bobsled Bonanza (the lawn starts frozen; bobsleds and Zambonis), Whack a Zombie (twelve waves), Pogo Party (pogo-only roof waves), Column Like You See 'Em (the roof's pot belt, mirror planting down a column, its set-piece waves), Invisighoul (the conveyor and hidden zombies on a fog lawn that shows no fog) Dr. Zomboss's Revenge (the boss fight on its boss belt, without the adventure intro), Zombies on Speed (the whole board updates twice a tick), Seeing Stars (the starfruit ghost picture: wrong plants are refused on its cells, the picture drops the award when complete, and the chooser warns when starfruit isn't picked) It's Raining Seeds (no seed bank: usable seed packets fall from the sky with the rain and are planted straight from the lawn for free) and Portal Combat (two pairs of square and round portals that move zombies, straight shots and running mowers between lanes, favour the portal rows when spawning, and relocate every 6000 ticks; its point particles and portal foley aren't ported) and Last Stand (5000 sun, no falling sun, five onslaughts of ten waves that start only on the Start/CONTINUE ONSLAUGHT button, ten suns at each stage's end) Slot Machine (three reels in the seed bank; 25 sun a pull spins them to plants, suns or diamonds; two of a kind pay one, three pay the jackpot; 2000 sun wins) and Beghouled (an 8×5 match-3 board of plants: drag adjacent pieces to make runs of three, matches pay sun and open the Repeater/Fume-shroom/Tall-nut upgrade shop plus Shuffle and Crater buttons, eaten pieces leave craters, 75 matches wins; falls and refills are instant in this port and the flashing match hint isn't drawn) Beghouled Twist (the same board, where clicking a 2×2 square rotates its four pieces clockwise when that makes a match; a static target overlay marks a valid square) and Zombiquarium (a fish tank of swimming snorkel zombies: 5 sun drops a brain, fish eat it for health and spit sun every 1000–1500 ticks, the Snorkel packet buys fish, the Trophy spends 1000 sun to win, and an empty tank loses; the wave overlay and tutorial arrows aren't ported). Each game's lawn, wave count, allowed zombies, point multiplier and first-wave delay come from `InitZombieWaves`/`StartLevel`/`PickBackground`; wins earn the tile's trophy and return to the page. Every tile on the challenge page is playable now; the remaining pages are puzzle's. Because unimplemented games can't be won, unlocking follows the ported games in page order (first three free, then one per trophy). Mini-game runs aren't saved or resumed.
- **Puzzle page** (`board/scary_potter.rs`, `board/izombie.rs`, `tests/minigame.rs`, `challenge.rs`): all 18 puzzle trophies plus both endless modes.
  - Vasebreaker (9 + endless): pots per the decomp tables, broken for usable seed packets, zombies or 1–3 suns; leaf hints, night lawn, no seed bank/mowers/waves, endless repopulates and keeps its best streak.
  - I, Zombie (9 + endless): 5 brains, pre-placed plants per level, zombie packets (cached zombie art and decomp costs) placed on the player's side of the line (bungees on the other), brain eating with the 50-sun loss rule, endless reuses level 9's layout with its streak.
  - Vasebreaker has the hammer's `anim_pot_open` swing, and planterns reveal nearby pots' contents through the lid; endless I, Zombie rolls its five formation families, scaling with the streak and using the five-column line.
- **Saving a level in progress** (`board::save`, `tests/save.rs`): leaving through the menu, or quitting the app, saves an adventure or survival run (`TryToSaveGame`/`NeedSaveGame`, `userdata/game{id}_{mode}.dat` as TOML); survival also checkpoints one tick before each between-stage repick. Adventure offers the Continue dialog from its button, and each survival tile offers its own (resume, or erase and start over); winning, losing, restarting or starting fresh erases the run; `--lvl` copies never save; deleting a profile deletes its saves.
- Windows: builds, setup screen and resource loading verified; everything else untested there.

## Missing: large pieces

| Area | Notes |
|------|-------|
| **Game modes** | `GameMode` has `Adventure`, `Intro`, `Survival`, `MiniGame` and `Puzzle`; adventure, all 20 mini-games, and the puzzle page (Vasebreaker + I, Zombie, 18 trophies) are playable. Survival has its between-stage seed repick (music fades out and back in), per-mode saved runs, and the per-tile Continue dialog. |
| **Zen Garden** | done: three gardens, needs/tools, chocolate, Stinky, wheelbarrow, sales, shop, menu bubble, Dave's tutorial and potted-plant/chocolate drops from zombies. |
| **Tree of Wisdom** | done: growth, tree food, speech (shop and garden both lead there). |
| **Achievements** | Out of scope: GOTY's achievements are Steamworks platform features, not game content. The pak's strings/icons are unused by the port. |
| **Windows/Linux verification** | Windows: title/menu/board never run visually; compiled particles untested; audio untested; `.au` sounds silent (needs a decoder, `audio.rs:703`). Linux: nothing run. |

## Missing: features inside existing screens

### Board and levels

### Plants
- Attached particles (`AddAttachedParticle`): the coffee bean's and grave buster's effects are pinned to the plant in the original; particles can't follow a reanimation track yet, so they're spawned in place (`board/plant.rs`).

### Zombies
- `anim_dance`: the original's typed cheat-code dance mode (`mDanceMode`), not a game mode; the cheat isn't ported.
- Boss: the pots' and fireballs' smoke trails (particles attached to reanimation tracks, `ApplyBossSmokeParticles`).
- Rising out of pool/roof graves: graves only stand on grass or high ground, so adventure content never grows one on water; the decomp's pool branch is unused here.

### Menus and screens
- Main menu: Zen "needs water" bubble (`game_selector.rs:892`), the other mode buttons, the trophy's tooltip.
- Store: Zen items and their "coming soon" rule (`store.rs:306`, `:862`, `:188`), going to the tree (`:746`), bonus mower and chocolate pages, award-screen intro talks, opening from the seed chooser (our store is a screen that returns to the main menu, so it can't open over a level yet).
- Seed chooser: its Store button (same reason as above). The Almanac button works.
- Widgets: vertical sliders (no screen uses them), clipboard and password characters in `EditWidget` (no OS clipboard here; nothing asks for a password).

### Engine
- Per-mode music (Zen Garden, mini-games).

## Suggested order

1. Presentation polish: attached particles (coffee bean, grave buster) and boss smoke trails.
2. Windows visual pass and `.au` decoder; Linux.

## Code TODO index

Run this for the live list:

```bash
grep -rn -E "TODO|Not ported|not ported" crates --include='*.rs'
```

Most entries above carry their `file:line` from the 2026-10-26 snapshot; lines drift as code changes.
