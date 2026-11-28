# Multiblocks and structure patterns

A multiblock is a block entity that behaves differently once the blocks around it form a shape: a
boiler that only heats when its casing, firebox and hatches are placed, a farm that needs its
structure intact. Sunrise has no separate multiblock framework. The mod API gives you the two pieces
the check needs, and `mod_api::Pattern` combines them:

- a **pattern** read once from layers of characters, with a legend saying what each character accepts;
- a **block read** through `PatternWorld`, which a game level answers through `LevelPatternWorld`.

The pattern validates the shape. It does not place, break or own the blocks: your block entity does
that, and re-runs the check when the world around it changes.

## Write the pattern

Layers are stacked bottom first. Each layer is its rows from north to south, and each row its cells
from west to east, so the cell written at row `z`, column `x` of layer `y` has local position
`[x, y, z]`. Every character must be named in the legend; an unknown one is refused instead of
leaving a hole.

{{#include ../../examples/src/multiblock.rs:pattern}}

`Legend::cell` gives a character a matcher with no role. `Legend::role` also names a role, so a
formed pattern can report where those cells are. The matchers are:

| Matcher | Accepts |
| --- | --- |
| `CellMatcher::Any` | Anything, air included. |
| `CellMatcher::Air` | Air, as the level reports it (`BlockStates::is_air`). |
| `CellMatcher::Block(key)` | Any state of that block, whatever its properties. |
| `CellMatcher::State(state)` | Exactly that state, properties included. |
| `CellMatcher::Tag(tag)` | Any block in the tag, in the current tag snapshot. |

Use a tag for a hatch tier set: a block-tag file under `data/<ns>/tags/block/` in a content mod, or a
`TagKey<BlockKey>` requested in `Hooks::register` with `TagsRegistrar::tag_key(...).typed()`. The tag
is read on each match, so a data reload changes what the pattern accepts without rebuilding it.

## Match it

{{#include ../../examples/src/multiblock.rs:match}}

`Pattern::matches` answers a `PatternMatch` only when every cell's state is available and accepted.
`origin` is the minimum corner of the pattern's **footprint after placement**, so building the same
shape mirrored or turned keeps the position the caller names. A position the level cannot answer
(unloaded chunk, outside the build height, a stale view) ends the match with `None`: no partial
formation.

Use `PatternPlacement` for the four quarter turns and the mirror:

```rust,ignore
let formed = casing.matches(&world, origin, PatternPlacement {
    rotation: PatternRotation::Clockwise,
    mirrored: true,
})?;
let front = formed.position([1, 0, 2]);          // a local cell, in the world
let hatches: Vec<_> = formed.positions("hatch").collect();
let controllers = formed.count("controller");
```

`PatternMatch::position` maps any local position; `positions(role)` and `count(role)` read the
legend's roles. `origin()` and `placement()` are there to store with the formed state.

On a server hook or a block entity tick, pass the level handle and its host to `matches_level`:

```rust,ignore
let formed = casing.matches_level(level, host, origin, PatternPlacement::default());
```

`LevelPatternWorld` reads block states through the level handle and its host, and forwards `is_air` and
the tag snapshot to the host, so the pattern uses the same level reads as the rest of the mod. There is no capability lookup inside the pattern: after a
match, read each hatch's `ITEMS`, `FLUIDS` or `ENERGY` capability at the positions the roles give
you, with the side that faces the machine.

## Cost and lifetime

- Matching walks the pattern's cells and allocates nothing. Keep the `Pattern` in the block entity
  type or a static, not in the tick path, and rebuild it only when its legend changes.
- Do not match every tick. Match when a neighbouring block changes, when the block entity is
  placed or loaded, and on a slower retry cadence while unformed.
- A pattern is read-only. Forming can require extra rules the shape cannot state (owner, tier,
  redstone, a minimum number of hatches): check those after `matches`, using the role positions.
- A match is a point-in-time answer. A later block change can break the shape; re-run the check
  rather than trusting a stored `PatternMatch`.

## Limits

- There is no auto-build, no hatch tier arithmetic and no StructureLib conversion. A 1.7.10
  StructureLib string becomes layers plus a legend, written by hand.
- Tags are block tags, not item or fluid tags; a hatch that must also accept a specific block
  entity capability is checked after the shape match.
- The pattern cannot describe entities, fluids or block entity data. Those are separate reads at
  the positions the shape gives you.
