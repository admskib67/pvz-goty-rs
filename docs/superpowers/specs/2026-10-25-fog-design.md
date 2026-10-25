# Fog: design

Date: 2026-10-25. Branch: `core`.

## Goal

World 4 (levels 4-1 to 4-10, `BackgroundType::Fog`) plays with its fog: a drifting bank of fog covers the right of the lawn, hides what is in it, is pushed back by plantern light (and, a little, by torchwood), and is blown away for a while by the blover. Plantern and blover work.

## Out of scope

Balloon zombies (blover's flyers; `BlowAwayFliers` is a stub until they exist), Crazy Dave's 4-x talks, 4-5 (the stormy-night level: `StageHasFog` is false there), zombies not yet ported, the seed chooser's fog warning, fog in mini-games.

## Rules for the work

Behaviour is read from the ruslan831 decomp (`Board::UpdateFog`, `ClearFogAroundPlant`, `DrawFog`, `LeftFogColumn`, `UpdateGame`'s fog offset, `Plant::UpdateBlover`, `Plant::DoSpecial`, `BlowAwayFliers`, plantern init) and reimplemented, with credit comments. Plants stay decoupled from the board (`PlantAction`). Test first.

## Design

- **State on `Board`:** `grid_cel_fog[9][7]` (fog opacity 0-255 per square, one extra row), `fog_offset: f32`, `fog_blown_count_down: i32`. `left_fog_column()`: level 31 = 6, 32-36 = 5, 37-40 = 4. `stage_has_fog()` is the fog background except level 35.
- **Start:** `fog_blown_count_down = 200`, `fog_offset = 1065 - left * 80`, squares empty. During the intro the offset is pinned off to the right; once play starts the countdown runs and the fog slides in over 200 ticks.
- **Offset (every tick, before the playing check):** intro: curve over 200; `count_down < 2000`: ease out from the maximum back to 0 over 2000; otherwise (just blown) it moves to the maximum.
- **Fade (`update_fog`):** from the left fog column each square gains opacity (speed 3, or 1 while returning, 20 while being blown; max 200 in the leftmost column, 255 elsewhere); every plantern clears a diamond of radius 4 (a 7x5 shape missing its corners) and every torchwood radius 1, at speed 6 (2 returning, 40 blown), taking the fog's own offset into account.
- **Drawing:** one render item at `render_layer::FOG`; each square draws a fog cel of `IMAGE_FOG` at `x*80 + offset - 15`, `y*85 + 20`, tinted by position and time, alpha = the square's opacity. The ninth column also draws a second cel 80px to its right.
- **Plantern:** attaches `LanternShine` at layer FOG+1 (above the fog), plays `SOUND_PLANTERN`; passive.
- **Blover:** `do_special_countdown = 50`, plays `anim_blow` once then `anim_loop`; at 0 `PlantAction::BlowFog`: `fog_blown_count_down = 4000`, `SOUND_BLOVER`; it stays 200 ticks then dies.

## Slices

1. Fog state, update and drawing (+ tests on the fade/offset numbers).
2. Plantern and torchwood clearing.
3. Blover.
4. 4-x plumbing: levels pick the fog background with fog on, intro keeps fog away, seed gating.
5. Docs.
