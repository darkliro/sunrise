//! Multiblock patterns: one shape, validated against a level.
//!
//! A pattern is a stack of layers of single-character cells. A [`Legend`] says what each character
//! accepts: any block, air, one block, one exact state or a block tag, and may name a role such as
//! `controller` or `hatch`. [`Pattern::matches`] walks the cells through a [`PatternWorld`] and
//! answers a [`PatternMatch`] only when every cell matches, which is the shape check a multiblock's
//! block entity runs when it forms.
//!
//! Matching is read-only and allocates nothing. It never loads a chunk: a position the world cannot
//! answer ends the match. Re-run it when a neighbouring block changes or on the block entity's own
//! cadence, not every tick.

use crate::{BlockKey, BlockState, Host, HostBlockStates, LevelRef, TagKey, Tags};
use std::fmt;

/// How a pattern is turned about the vertical axis before it is matched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PatternRotation {
    /// The layers as written: `x` grows east and `z` grows south.
    #[default]
    None,
    /// A quarter turn clockwise seen from above.
    Clockwise,
    /// Half a turn.
    Half,
    /// A quarter turn counter-clockwise seen from above.
    CounterClockwise,
}

/// A pattern's `rotation` and whether it is mirrored first, across its north-south axis.
///
/// The mirror applies to the pattern as written, then the rotation: this is the order a build
/// tool uses when it flips a plan and then turns it. `PatternPlacement::default()` matches the pattern as
/// written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PatternPlacement {
    /// The turn applied after the mirror.
    pub rotation: PatternRotation,
    /// Whether the pattern is mirrored across its north-south axis before turning.
    pub mirrored: bool,
}

/// What one pattern cell accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CellMatcher {
    /// Any state, including air.
    Any,
    /// Air of any kind, as the world reports it.
    Air,
    /// Any state of this block, whatever its properties.
    Block(BlockKey),
    /// Exactly this state, properties included.
    State(BlockState),
    /// Any block in this tag (`BlockState.is(TagKey)`), in the current snapshot.
    Tag(TagKey<BlockKey>),
}

impl CellMatcher {
    fn accepts(&self, state: BlockState, world: &dyn PatternWorld) -> bool {
        match self {
            Self::Any => true,
            Self::Air => world.is_air(state),
            Self::Block(block) => state.block() == *block,
            Self::State(wanted) => state == *wanted,
            Self::Tag(tag) => state.is_in_tag(world.tags(), *tag) == Some(true),
        }
    }
}

/// The characters a pattern's layers use, and what each one accepts.
///
/// Every character in a layer must have an entry: an unknown character is a [`PatternError`], never
/// an empty cell. Building a legend consumes it and returns it, so entries chain. A second entry
/// for the same character replaces the first.
#[derive(Clone, Debug, Default)]
pub struct Legend {
    entries: Vec<(char, CellMatcher, Option<&'static str>)>,
}

impl Legend {
    /// An empty legend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accept `matcher` where `symbol` appears, with no role.
    pub fn cell(mut self, symbol: char, matcher: CellMatcher) -> Self {
        self.insert(symbol, matcher, None);
        self
    }

    /// Accept `matcher` where `symbol` appears, and give those cells `role` so a formed
    /// [`PatternMatch`] can name their world positions, such as `hatch` or `controller`.
    pub fn role(mut self, symbol: char, matcher: CellMatcher, role: &'static str) -> Self {
        self.insert(symbol, matcher, Some(role));
        self
    }

    fn insert(&mut self, symbol: char, matcher: CellMatcher, role: Option<&'static str>) {
        match self.entries.iter_mut().find(|entry| entry.0 == symbol) {
            Some(entry) => {
                entry.1 = matcher;
                entry.2 = role;
            }
            None => self.entries.push((symbol, matcher, role)),
        }
    }

    fn get(&self, symbol: char) -> Option<&(char, CellMatcher, Option<&'static str>)> {
        self.entries.iter().find(|entry| entry.0 == symbol)
    }
}

/// A pattern that cannot be read from its layers.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatternError {
    /// No layers, or a layer with no rows.
    Empty,
    /// A layer's row has a different width from its first row.
    UnevenRow {
        /// The layer's index, from the bottom.
        layer: usize,
        /// The row's index within the layer.
        row: usize,
    },
    /// A layer has a different row count from the first layer.
    UnevenLayer {
        /// The layer's index, from the bottom.
        layer: usize,
    },
    /// A character with no legend entry.
    UnknownSymbol {
        /// The layer's index, from the bottom.
        layer: usize,
        /// The row's index within the layer.
        row: usize,
        /// The character's index within the row.
        column: usize,
        /// The character itself.
        symbol: char,
    },
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("the pattern has no layers or no rows"),
            Self::UnevenRow { layer, row } => {
                write!(
                    f,
                    "layer {layer} row {row} is not as wide as the layer's first row"
                )
            }
            Self::UnevenLayer { layer } => {
                write!(f, "layer {layer} does not have the first layer's row count")
            }
            Self::UnknownSymbol {
                layer,
                row,
                column,
                symbol,
            } => write!(
                f,
                "layer {layer} row {row} column {column} uses {symbol:?}, which the legend does not name"
            ),
        }
    }
}

impl std::error::Error for PatternError {}

/// A block read a pattern needs: the state at a position, whether it is air, and the tag snapshot.
///
/// A game side implements this through [`LevelPatternWorld`], which forwards to a [`LevelRef`] and
/// its [`Host`]; a test implements it over a map.
pub trait PatternWorld {
    /// The state at `pos`, or `None` when the position is unloaded, out of height, or unavailable.
    fn block_state(&self, pos: [i32; 3]) -> Option<BlockState>;
    /// Whether `state` is air, for [`CellMatcher::Air`].
    fn is_air(&self, state: BlockState) -> bool;
    /// The tag snapshot [`CellMatcher::Tag`] reads.
    fn tags(&self) -> &dyn Tags;
}

/// A [`PatternWorld`] over a game level: a [`LevelRef`] handle and the [`Host`] that reads it.
pub struct LevelPatternWorld<'a> {
    level: LevelRef,
    host: &'a dyn Host,
}

impl<'a> LevelPatternWorld<'a> {
    /// Read `level` through `host`, as `level.block_state(host, pos)` does.
    pub fn new(level: LevelRef, host: &'a dyn Host) -> Self {
        Self { level, host }
    }
}

impl PatternWorld for LevelPatternWorld<'_> {
    fn block_state(&self, pos: [i32; 3]) -> Option<BlockState> {
        self.level.block_state(self.host, pos)
    }

    fn is_air(&self, state: BlockState) -> bool {
        self.host.block_states().is_air(state).unwrap_or(false)
    }

    fn tags(&self) -> &dyn Tags {
        self.host.tags()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Cell {
    local: [i32; 3],
    matcher: CellMatcher,
    role: Option<&'static str>,
}
/// A multiblock shape, read once from its layers and matched many times.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    size: [i32; 3],
    cells: Vec<Cell>,
}

impl Pattern {
    /// Read a pattern from its layers, bottom first.
    ///
    /// A layer is its rows from north to south, and a row is its cells from west to east, so the
    /// local position of `layers[y][z]`'s `x`th character is `[x, y, z]`. Every layer must have the
    /// same row count and every row the same width. `legend` must name every character used.
    pub fn new(layers: &[&[&str]], legend: &Legend) -> Result<Self, PatternError> {
        let Some(first_layer) = layers.first() else {
            return Err(PatternError::Empty);
        };
        let Some(first_row) = first_layer.first() else {
            return Err(PatternError::Empty);
        };
        let width = first_row.chars().count();
        if width == 0 {
            return Err(PatternError::Empty);
        }
        let depth = first_layer.len();
        let mut cells = Vec::new();
        for (y, layer) in layers.iter().enumerate() {
            if layer.len() != depth {
                return Err(PatternError::UnevenLayer { layer: y });
            }
            for (z, row) in layer.iter().enumerate() {
                let symbols: Vec<char> = row.chars().collect();
                if symbols.len() != width {
                    return Err(PatternError::UnevenRow { layer: y, row: z });
                }
                for (x, symbol) in symbols.into_iter().enumerate() {
                    let Some((_, matcher, role)) = legend.get(symbol) else {
                        return Err(PatternError::UnknownSymbol {
                            layer: y,
                            row: z,
                            column: x,
                            symbol,
                        });
                    };
                    cells.push(Cell {
                        local: [x as i32, y as i32, z as i32],
                        matcher: matcher.clone(),
                        role: *role,
                    });
                }
            }
        }
        Ok(Self {
            size: [width as i32, layers.len() as i32, depth as i32],
            cells,
        })
    }

    /// The pattern's size as written: `[width x, height y, depth z]`.
    pub fn size(&self) -> [i32; 3] {
        self.size
    }

    /// The footprint's size after `placement`: x and z swap for a quarter turn.
    pub fn footprint(&self, placement: PatternPlacement) -> [i32; 3] {
        match placement.rotation {
            PatternRotation::None | PatternRotation::Half => self.size,
            PatternRotation::Clockwise | PatternRotation::CounterClockwise => {
                [self.size[2], self.size[1], self.size[0]]
            }
        }
    }

    /// Turn one local position into its offset from the pattern's origin after `placement`.
    fn placed_delta(&self, local: [i32; 3], placement: PatternPlacement) -> [i32; 3] {
        let [width, _, depth] = self.size;
        let x = if placement.mirrored {
            width - 1 - local[0]
        } else {
            local[0]
        };
        let z = local[2];
        let (x, z) = match placement.rotation {
            PatternRotation::None => (x, z),
            PatternRotation::Clockwise => (depth - 1 - z, x),
            PatternRotation::Half => (width - 1 - x, depth - 1 - z),
            PatternRotation::CounterClockwise => (z, width - 1 - x),
        };
        [x, local[1], z]
    }

    /// Match the whole pattern with its minimum corner at `origin`.
    ///
    /// `origin` is the minimum corner of the pattern's footprint after `placement`, so a mirrored
    /// or turned pattern still starts at the position the caller names. `None` if any cell's state
    /// is unavailable or does not match.
    pub fn matches(
        &self,
        world: &dyn PatternWorld,
        origin: [i32; 3],
        placement: PatternPlacement,
    ) -> Option<PatternMatch<'_>> {
        for cell in &self.cells {
            let delta = self.placed_delta(cell.local, placement);
            let pos = [
                origin[0] + delta[0],
                origin[1] + delta[1],
                origin[2] + delta[2],
            ];
            let state = world.block_state(pos)?;
            if !cell.matcher.accepts(state, world) {
                return None;
            }
        }
        Some(PatternMatch {
            pattern: self,
            origin,
            placement,
        })
    }

    /// [`Self::matches`] over a game level: the handle and the host that reads it.
    pub fn matches_level(
        &self,
        level: LevelRef,
        host: &dyn Host,
        origin: [i32; 3],
        placement: PatternPlacement,
    ) -> Option<PatternMatch<'_>> {
        self.matches(&LevelPatternWorld::new(level, host), origin, placement)
    }
}

/// A pattern that matched: its origin, its placement and the world positions of its roles.
#[derive(Clone, Copy, Debug)]
pub struct PatternMatch<'a> {
    pattern: &'a Pattern,
    origin: [i32; 3],
    placement: PatternPlacement,
}

impl<'a> PatternMatch<'a> {
    /// The world position of the pattern's minimum corner.
    pub fn origin(&self) -> [i32; 3] {
        self.origin
    }

    /// The placement the pattern matched with.
    pub fn placement(&self) -> PatternPlacement {
        self.placement
    }

    /// The world position of the cell written at local `[x, y, z]`.
    pub fn position(&self, local: [i32; 3]) -> [i32; 3] {
        let delta = self.pattern.placed_delta(local, self.placement);
        [
            self.origin[0] + delta[0],
            self.origin[1] + delta[1],
            self.origin[2] + delta[2],
        ]
    }

    /// The world positions of the cells given `role` in the legend, in layer order.
    pub fn positions<'b>(&'b self, role: &'b str) -> impl Iterator<Item = [i32; 3]> + 'b {
        self.pattern
            .cells
            .iter()
            .filter(move |cell| cell.role == Some(role))
            .map(move |cell| self.position(cell.local))
    }

    /// How many cells the legend gives `role`.
    pub fn count(&self, role: &str) -> usize {
        self.pattern
            .cells
            .iter()
            .filter(|cell| cell.role == Some(role))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STONE: BlockKey = BlockKey(7);
    const GLASS: BlockKey = BlockKey(8);

    #[derive(Default)]
    struct NoTags;
    impl Tags for NoTags {}

    struct TestWorld {
        blocks: Vec<([i32; 3], BlockState)>,
        air: BlockState,
        tags: NoTags,
    }

    impl TestWorld {
        fn new(air: BlockState) -> Self {
            Self {
                blocks: Vec::new(),
                air,
                tags: NoTags,
            }
        }

        fn set(&mut self, pos: [i32; 3], block: BlockKey) {
            let state = crate::block::block_state::block_state_from_ordinal(block, 0);
            if let Some((_, stored)) = self.blocks.iter_mut().find(|(at, _)| *at == pos) {
                *stored = state;
            } else {
                self.blocks.push((pos, state));
            }
        }
    }

    impl PatternWorld for TestWorld {
        fn block_state(&self, pos: [i32; 3]) -> Option<BlockState> {
            Some(
                self.blocks
                    .iter()
                    .find(|(at, _)| *at == pos)
                    .map_or(self.air, |(_, state)| *state),
            )
        }

        fn is_air(&self, state: BlockState) -> bool {
            state == self.air
        }

        fn tags(&self) -> &dyn Tags {
            &self.tags
        }
    }

    fn air() -> BlockState {
        crate::block::block_state::block_state_from_ordinal(BlockKey(0), 0)
    }

    fn casing() -> Pattern {
        let legend = Legend::new()
            .role(
                'H',
                CellMatcher::State(crate::block::block_state::block_state_from_ordinal(
                    GLASS, 0,
                )),
                "hatch",
            )
            .cell('.', CellMatcher::Any)
            .cell(' ', CellMatcher::Air);
        Pattern::new(&[&[" H ", "...", " H "]], &legend).unwrap()
    }

    #[test]
    fn reads_layers_and_names_roles_by_world_position() {
        let pattern = casing();
        assert_eq!(pattern.size(), [3, 1, 3]);
        let mut world = TestWorld::new(air());
        world.set([11, 64, 20], GLASS);
        world.set([11, 64, 22], GLASS);
        let found = pattern
            .matches(&world, [10, 64, 20], PatternPlacement::default())
            .unwrap();
        assert_eq!(found.origin(), [10, 64, 20]);
        assert_eq!(found.count("hatch"), 2);
        let hatches: Vec<_> = found.positions("hatch").collect();
        assert_eq!(hatches, [[11, 64, 20], [11, 64, 22]]);
        assert_eq!(found.position([0, 0, 0]), [10, 64, 20]);
    }

    #[test]
    fn turns_and_mirrors_about_the_origin() {
        let pattern = casing();
        assert_eq!(
            pattern.footprint(PatternPlacement {
                rotation: PatternRotation::Clockwise,
                mirrored: true,
            }),
            [3, 1, 3]
        );
        let mut world = TestWorld::new(air());
        // Mirror x -> 2 - x, then clockwise (x, z) -> (2 - z, x).
        world.set([2, 0, 1], GLASS);
        world.set([0, 0, 1], GLASS);
        let found = pattern
            .matches(
                &world,
                [0, 0, 0],
                PatternPlacement {
                    rotation: PatternRotation::Clockwise,
                    mirrored: true,
                },
            )
            .unwrap();
        assert_eq!(found.position([0, 0, 0]), [2, 0, 2]);
        assert_eq!(found.position([1, 0, 1]), [1, 0, 1]);
        let hatches: Vec<_> = found.positions("hatch").collect();
        assert_eq!(hatches, [[2, 0, 1], [0, 0, 1]]);
    }

    #[test]
    fn refuses_a_missing_or_wrong_cell() {
        let pattern = casing();
        let mut world = TestWorld::new(air());
        world.set([1, 0, 0], GLASS);
        world.set([1, 0, 2], GLASS);
        world.set([1, 0, 0], STONE);
        assert!(
            pattern
                .matches(&world, [0, 0, 0], PatternPlacement::default())
                .is_none(),
            "a wrong hatch state must refuse the shape"
        );
        let mut broken = TestWorld::new(air());
        broken.set([1, 0, 0], GLASS);
        assert!(
            pattern
                .matches(&broken, [0, 0, 0], PatternPlacement::default())
                .is_none()
        );
    }

    #[test]
    fn reports_uneven_rows_and_unknown_symbols() {
        let legend = Legend::new().cell('x', CellMatcher::Any);
        assert_eq!(
            Pattern::new(&[&["xx", "x"]], &legend),
            Err(PatternError::UnevenRow { layer: 0, row: 1 })
        );
        assert_eq!(
            Pattern::new(&[&["x"], &["xx"]], &legend),
            Err(PatternError::UnevenRow { layer: 1, row: 0 })
        );
        assert_eq!(
            Pattern::new(&[&["x?"]], &legend),
            Err(PatternError::UnknownSymbol {
                layer: 0,
                row: 0,
                column: 1,
                symbol: '?'
            })
        );
        assert_eq!(Pattern::new(&[], &legend), Err(PatternError::Empty));
    }
}
