# Multiblocks and structure patterns

A multiblock is a block entity that behaves differently once the blocks around it form a shape: a
boiler that only heats when its casing, firebox and hatches are placed, a farm that needs its
structure intact. Sunrise has no separate multiblock framework. The mod API gives you the two pieces
the check needs, and `mod_api::Pattern` combines them:

- a **pattern** read once from layers of characters, with a legend saying what each character accepts;
- a **block read** from a `LevelRef` and its `Host`, through `Pattern::matches_level`.

The pattern validates the shape. It does not place, break or own the blocks.

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
(unloaded chunk, outside the build height, a stale view), or one an `i32` position cannot name, ends
the match with `None`: no partial formation.

`PatternPlacement` takes one of `StructureRotation`'s four turns and a mirror, which applies before
the turn:

```rust,ignore
let formed = casing.matches_level(level, host, origin, PatternPlacement {
    rotation: StructureRotation::Clockwise90,
    mirrored: true,
})?;
let front = formed.position([1, 0, 2]);          // Some(...) for a local cell in the world
let hatches: Vec<_> = formed.positions("hatch").collect();
let controllers = formed.count("controller");
```

`PatternMatch::position` maps any local position and answers `None` when an `i32` position cannot
name it; `positions(role)` and `count(role)` read the legend's roles. `origin()` and `placement()` are
there to store with the formed state.

`matches_level(level, host, ...)` is the whole game-facing read: it asks the level for each cell's
state and the host for air and the tag snapshot, the same reads the rest of the mod uses. There is
no capability lookup inside the pattern: after a match, read each hatch's `ITEMS`, `FLUIDS` or
`ENERGY` capability at the positions the roles give you, with the side that faces the machine.

`Pattern::matches` takes a `PatternWorld` instead, for a caller that has no `Host`: the same three
reads, which a unit test answers over a map.

## Cost and lifetime

- Matching walks the pattern's cells and allocates nothing. A `Pattern` is a value the block entity
  type keeps and reads.
- A pattern is read-only. Forming can require extra rules the shape cannot state (owner, tier,
  redstone, a minimum number of hatches): those are checked after `matches`, at the role positions.
- A match is a point-in-time answer: it says which cells were where, when the match ran.

## Limits

- There is no auto-build, no hatch tier arithmetic and no StructureLib conversion. A 1.7.10
  StructureLib string becomes layers plus a legend, written by hand.
- Tags are block tags, not item or fluid tags; a hatch that must also accept a specific block
  entity capability is checked after the shape match.
- The pattern cannot describe entities, fluids or block entity data. Those are separate reads at
  the positions the shape gives you.
