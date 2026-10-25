# Roof: design

Date: 2026-10-25. Branch: `core`.

## Goal

World 5 (levels 5-1 to 5-9) plays on its sloped roof: everything is planted in flower pots, shots follow the slope, and the lobbing plants (cabbage-pult, kernel-pult, melon-pult) work.

## Out of scope

Roof zombies (bungee, ladder, catapult, gargantuar, imp), the boss level 5-10, umbrella leaf, upgrades (winter melon, cob cannon), the roof cleaner store item, rake, Crazy Dave's 5-x talks, the seed chooser's roof warnings.

## Rules for the work

Per ruslan831 decomp: `Board::CanPlantAt`, `GetPosYBasedOnRow`, `Projectile::UpdateMotion`/`UpdateLobMotion`/`DoSplashDamage`, `Plant::Fire`, `Plant::PlantInitialize` (shooting counters). Plants stay decoupled from the board. Test first.

## Design

- **Planting:** on the roof a plant needs a flower pot on its square (`PlantingReason::NeedsPot` already exists). A flower pot goes on any roof square with nothing in it and nowhere else. Spikeweed and spikerock can't go on the roof. Zombies eat the plant, then the pot (the pot is an "under plant", like the lily pad). A plant in a pot stands 5 px higher and its shots leave 5 px higher.
- **No mowers:** roof levels have no lawn mowers without the (unported) roof cleaner.
- **Slope:** zombies and plants already use `pos_y_based_on_row`/`grid_to_pixel_y` with the roof slope. Projectiles remember the row's height when they start and follow its change as they move: lobbed shots shift `pos_y` and `pos_z` by the change, straight shots only their shadow.
- **Lobbed projectiles:** `ProjectileType::{Cabbage, Kernel, Melon, Butter}` with `Lobbed` motion: the plant gives them `vel_x = range_x / 120`, `vel_z = range_y / 120 - 7`, `acc_z = 0.115`; each tick `vel_z += acc_z`, position adds the velocities; after age 20 and while falling they test for a hit once `pos_z` passes the type's minimum collision height, or the ground at 80. A hit or the ground ends them. Melons splash the rows beside for a third of their damage. Kernel shots are sometimes butter: 40 damage and a stun (`buttered_counter`, 400 ticks).
- **Lobbing plants:** a shooter whose `Fire` computes the aim from the target zombie (`target_lead_x(50)`, the zombie's top) and launches the arc; `shooting_counter` 32 (cabbage), 30 (kernel), 36 (melon); damage range 13 (reaches submerged snorkels).
- **Levels 41-49** pick the roof background (already so); the 5-x board needs no new intro.

## Slices

1. Roof planting rules and flower pots; no roof mowers.
2. Projectile slope following.
3. Lobbed projectiles and the three lobbers (+ butter).
4. 5-x plumbing check, docs.
