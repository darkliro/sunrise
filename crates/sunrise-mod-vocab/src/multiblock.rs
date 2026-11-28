//! Multiblock patterns: one shape, validated against a level.
//!
//! A pattern is a stack of layers of single-character cells. A [`Legend`] says what each character
//! accepts: any block, air, one block, one exact state or a block tag, and may name a role such as
//! `controller` or `hatch`. [`Pattern::matches_level`] walks the cells through a level and answers a
//! [`PatternMatch`] only when every cell matches, which is the shape check a multiblock's block
//! entity runs when it forms.
//!
//! Matching is read-only and allocates nothing. It never loads a chunk: a position the level cannot
//! answer, or one an `i32` position cannot name, ends the match instead of reading another cell.

use crate::{
    BlockKey, BlockState, Host, HostBlockStates, LevelRef, StructureRotation, TagKey, Tags,
};
use std::fmt;

/// A pattern's `rotation` and whether it is mirrored first, across its north-south axis.
///
/// The mirror applies to the pattern as written, then the rotation: this is the order a build tool
/// uses when it flips a plan and then turns it. `PatternPlacement::default()` matches the pattern as
/// written.
// exhaustive: the two fields are the whole placement, and its Wire form is the contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
pub struct PatternPlacement {
    /// The turn applied after the mirror.
    pub rotation: StructureRotation,
    /// Whether the pattern is mirrored across its north-south axis before turning.
    pub mirrored: bool,
}

impl Default for PatternPlacement {
    fn default() -> Self {
        Self {
            rotation: StructureRotation::None,
            mirrored: false,
        }
    }
}

/// What one pattern cell accepts.
// no wire: a matcher is built during load and stays in the mod's own memory; a tag matcher holds a host-resolved key.
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
// exhaustive: its fields are private, so mods cannot build or destructure it.
// no wire: a legend is built and consumed during load; it has no binary form.
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
// no wire: reports a local constructor failure, never a value that crosses a boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatternError {
    /// No layers, or a first layer with no rows or a row with no cells.
    Empty,
    /// A layer's row has a different width from the pattern's first row.
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
    /// The pattern is wider, taller or deeper than an `i32` position can name.
    TooLarge,
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str(
                "the pattern has no layers, or its first layer has no rows or a row with no cells",
            ),
            Self::UnevenRow { layer, row } => {
                write!(
                    f,
                    "layer {layer} row {row} is not as wide as the pattern's first row"
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
            Self::TooLarge => f.write_str("the pattern is larger than an i32 position can name"),
        }
    }
}

impl std::error::Error for PatternError {}

/// A block read a pattern needs: the state at a position, whether it is air, and the tag snapshot.
///
/// [`Pattern::matches_level`] is the game-facing read and needs no adapter: it takes a [`LevelRef`]
/// and the [`Host`] that reads it. This trait is the same three reads named for a caller that has no
/// `Host`, such as a unit test over a map.
pub trait PatternWorld {
    /// The state at `pos`, or `None` when the position is unloaded, out of height, or unavailable.
    fn block_state(&self, pos: [i32; 3]) -> Option<BlockState>;
    /// Whether `state` is air, for [`CellMatcher::Air`].
    fn is_air(&self, state: BlockState) -> bool;
    /// The tag snapshot [`CellMatcher::Tag`] reads.
    fn tags(&self) -> &dyn Tags;
}

/// A [`PatternWorld`] over a game level, for [`Pattern::matches_level`].
struct LevelWorld<'a> {
    level: LevelRef,
    host: &'a dyn Host,
}

impl PatternWorld for LevelWorld<'_> {
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
// exhaustive: its fields are private, so mods cannot build or destructure it.
// no wire: a pattern is read from the mod's own layers during load; it has no binary form.
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
    /// same row count and every row the same width, and the size must fit an `i32` position.
    /// `legend` must name every character used.
    pub fn new(layers: &[&[&str]], legend: &Legend) -> Result<Self, PatternError> {
        let Some(first_layer) = layers.first() else {
            return Err(PatternError::Empty);
        };
        let Some(first_row) = first_layer.first() else {
            return Err(PatternError::Empty);
        };
        let width = i32::try_from(first_row.chars().count()).map_err(|_| PatternError::TooLarge)?;
        if width == 0 {
            return Err(PatternError::Empty);
        }
        let depth = i32::try_from(first_layer.len()).map_err(|_| PatternError::TooLarge)?;
        let height = i32::try_from(layers.len()).map_err(|_| PatternError::TooLarge)?;
        let mut cells = Vec::new();
        for (y, layer) in layers.iter().enumerate() {
            if layer.len() != first_layer.len() {
                return Err(PatternError::UnevenLayer { layer: y });
            }
            for (z, row) in layer.iter().enumerate() {
                let symbols: Vec<char> = row.chars().collect();
                if symbols.len() != width as usize {
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
                    // Every index is below the width, height and depth checked above, so the
                    // conversion cannot wrap.
                    cells.push(Cell {
                        local: [x as i32, y as i32, z as i32],
                        matcher: matcher.clone(),
                        role: *role,
                    });
                }
            }
        }
        Ok(Self {
            size: [width, height, depth],
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
            StructureRotation::None | StructureRotation::Clockwise180 => self.size,
            StructureRotation::Clockwise90 | StructureRotation::Counterclockwise90 => {
                [self.size[2], self.size[1], self.size[0]]
            }
        }
    }

    /// Turn one local position into its offset from the pattern's origin after `placement`.
    ///
    /// `None` when the local position is so far from the pattern that its offset is not an `i32`.
    fn placed_delta(&self, local: [i32; 3], placement: PatternPlacement) -> Option<[i32; 3]> {
        let [width, _, depth] = self.size;
        let last_x = width - 1;
        let last_z = depth - 1;
        let x = if placement.mirrored {
            last_x.checked_sub(local[0])?
        } else {
            local[0]
        };
        let z = local[2];
        let (x, z) = match placement.rotation {
            StructureRotation::None => (x, z),
            StructureRotation::Clockwise90 => (last_z.checked_sub(z)?, x),
            StructureRotation::Clockwise180 => (last_x.checked_sub(x)?, last_z.checked_sub(z)?),
            StructureRotation::Counterclockwise90 => (z, last_x.checked_sub(x)?),
        };
        Some([x, local[1], z])
    }

    /// The world position of `local`, or `None` when an `i32` cannot name it.
    fn world_position(
        &self,
        origin: [i32; 3],
        local: [i32; 3],
        placement: PatternPlacement,
    ) -> Option<[i32; 3]> {
        let delta = self.placed_delta(local, placement)?;
        Some([
            origin[0].checked_add(delta[0])?,
            origin[1].checked_add(delta[1])?,
            origin[2].checked_add(delta[2])?,
        ])
    }

    /// Match the whole pattern with its minimum corner at `origin`.
    ///
    /// `origin` is the minimum corner of the pattern's footprint after `placement`, so a mirrored
    /// or turned pattern still starts at the position the caller names. `None` if any cell's state
    /// is unavailable or does not match, or if a cell's position is not an `i32` position.
    pub fn matches(
        &self,
        world: &dyn PatternWorld,
        origin: [i32; 3],
        placement: PatternPlacement,
    ) -> Option<PatternMatch<'_>> {
        for cell in &self.cells {
            let pos = self.world_position(origin, cell.local, placement)?;
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

    /// [`Self::matches`] over a game level: the level handle and the host that reads it.
    pub fn matches_level(
        &self,
        level: LevelRef,
        host: &dyn Host,
        origin: [i32; 3],
        placement: PatternPlacement,
    ) -> Option<PatternMatch<'_>> {
        self.matches(&LevelWorld { level, host }, origin, placement)
    }
}

/// A pattern that matched: its origin, its placement and the world positions of its roles.
// exhaustive: its fields are private, so mods cannot build or destructure it.
// no wire: a match borrows the pattern and lives for one check.
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

    /// The world position of the cell written at local `[x, y, z]`, or `None` when an `i32`
    /// position cannot name it.
    pub fn position(&self, local: [i32; 3]) -> Option<[i32; 3]> {
        self.pattern
            .world_position(self.origin, local, self.placement)
    }

    /// The world positions of the cells given `role` in the legend, in layer order.
    pub fn positions<'b>(&'b self, role: &'b str) -> impl Iterator<Item = [i32; 3]> + 'b {
        self.pattern
            .cells
            .iter()
            .filter(move |cell| cell.role == Some(role))
            // `matches` checked every cell's position, so these cannot be unrepresentable.
            .map(move |cell| {
                self.position(cell.local)
                    .expect("matches checked every cell's position")
            })
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
    const ORE_TAG: TagKey<BlockKey> = TagKey::from_static("test:ore");

    fn state(block: BlockKey, ordinal: u16) -> BlockState {
        crate::block::block_state::block_state_from_ordinal(block, ordinal)
    }

    fn air() -> BlockState {
        state(BlockKey(0), 0)
    }

    #[derive(Default)]
    struct TestTags {
        members: Vec<(BlockKey, TagKey<BlockKey>)>,
    }

    impl Tags for TestTags {
        fn block_in_tag(&self, block: BlockKey, tag: TagKey<BlockKey>) -> Option<bool> {
            Some(self.members.contains(&(block, tag)))
        }
    }

    struct TestWorld {
        cells: Vec<([i32; 3], Option<BlockState>)>,
        tags: TestTags,
    }

    impl TestWorld {
        fn new() -> Self {
            Self {
                cells: Vec::new(),
                tags: TestTags::default(),
            }
        }

        fn set(&mut self, pos: [i32; 3], block: BlockKey, ordinal: u16) {
            self.put(pos, Some(state(block, ordinal)));
        }

        /// A cell the world cannot answer, as an unloaded chunk or an out-of-height position is.
        fn unreadable(&mut self, pos: [i32; 3]) {
            self.put(pos, None);
        }

        fn put(&mut self, pos: [i32; 3], state: Option<BlockState>) {
            match self.cells.iter_mut().find(|(at, _)| *at == pos) {
                Some((_, stored)) => *stored = state,
                None => self.cells.push((pos, state)),
            }
        }

        fn in_tag(&mut self, block: BlockKey, tag: TagKey<BlockKey>) {
            self.tags.members.push((block, tag));
        }
    }

    impl PatternWorld for TestWorld {
        fn block_state(&self, pos: [i32; 3]) -> Option<BlockState> {
            match self.cells.iter().find(|(at, _)| *at == pos) {
                Some((_, state)) => *state,
                None => Some(air()),
            }
        }

        fn is_air(&self, state: BlockState) -> bool {
            state == air()
        }

        fn tags(&self) -> &dyn Tags {
            &self.tags
        }
    }

    /// A 3x2x2 shape: a hatch at the west-north bottom corner and a controller at the east-south
    /// top corner, so every expected position names a different axis and layer.
    fn casing() -> Pattern {
        let legend = Legend::new()
            .role('H', CellMatcher::Block(GLASS), "hatch")
            .role('C', CellMatcher::Block(STONE), "controller")
            .cell('.', CellMatcher::Any);
        Pattern::new(&[&["H..", "..."], &["...", "..C"]], &legend).unwrap()
    }

    #[test]
    fn reads_layers_and_names_roles_by_world_position() {
        let pattern = casing();
        assert_eq!(pattern.size(), [3, 2, 2]);
        let mut world = TestWorld::new();
        world.set([10, 64, 40], GLASS, 0);
        world.set([12, 65, 41], STONE, 0);
        let found = pattern
            .matches(&world, [10, 64, 40], PatternPlacement::default())
            .unwrap();
        assert_eq!(found.origin(), [10, 64, 40]);
        assert_eq!(found.count("hatch"), 1);
        assert_eq!(found.count("controller"), 1);
        // Row 0 is north, so the hatch is the north edge of the bottom layer; the controller is
        // one layer up, on the south row and the east column.
        assert_eq!(found.positions("hatch").collect::<Vec<_>>(), [[10, 64, 40]]);
        assert_eq!(
            found.positions("controller").collect::<Vec<_>>(),
            [[12, 65, 41]]
        );
        assert_eq!(found.position([0, 0, 0]), Some([10, 64, 40]));
        assert_eq!(found.position([2, 1, 1]), Some([12, 65, 41]));
        assert_eq!(found.count("missing"), 0);
    }

    #[test]
    fn turns_and_mirrors_about_the_origin() {
        let legend = Legend::new()
            .role('a', CellMatcher::Any, "a")
            .role('b', CellMatcher::Any, "b")
            .cell('.', CellMatcher::Any);
        // 2 wide, 3 deep: turning swaps the axes, so the footprint says which turn was applied.
        let pattern = Pattern::new(&[&["a.", "..", ".b"]], &legend).unwrap();
        let placement = PatternPlacement {
            rotation: StructureRotation::Clockwise90,
            mirrored: true,
        };
        assert_eq!(pattern.footprint(placement), [3, 1, 2]);
        // Mirror x -> 1 - x, then clockwise (x, z) -> (2 - z, x).
        let mut world = TestWorld::new();
        world.set([2, 0, 1], GLASS, 0);
        world.set([0, 0, 0], GLASS, 0);
        let found = pattern.matches(&world, [0, 0, 0], placement).unwrap();
        assert_eq!(found.positions("a").collect::<Vec<_>>(), [[2, 0, 1]]);
        assert_eq!(found.positions("b").collect::<Vec<_>>(), [[0, 0, 0]]);
        assert_eq!(found.position([0, 0, 0]), Some([2, 0, 1]));
    }

    #[test]
    fn refuses_a_cell_the_world_cannot_read() {
        let pattern = casing();
        let mut unreadable = TestWorld::new();
        unreadable.set([10, 64, 40], GLASS, 0);
        unreadable.unreadable([12, 65, 41]);
        assert!(
            pattern
                .matches(&unreadable, [10, 64, 40], PatternPlacement::default())
                .is_none(),
            "an unreadable cell must end the match, not read as air"
        );

        // The same block in another state: `Block` takes every state, `State` only the one named.
        let legend = Legend::new()
            .cell('b', CellMatcher::Block(GLASS))
            .cell('s', CellMatcher::State(state(GLASS, 0)));
        let by_block = Pattern::new(&[&["b"]], &legend).unwrap();
        let by_state = Pattern::new(&[&["s"]], &legend).unwrap();
        let mut world = TestWorld::new();
        world.set([0, 0, 0], GLASS, 1);
        assert!(
            by_block
                .matches(&world, [0, 0, 0], PatternPlacement::default())
                .is_some()
        );
        assert!(
            by_state
                .matches(&world, [0, 0, 0], PatternPlacement::default())
                .is_none()
        );

        // Tag membership, against the world's snapshot.
        let tagged = Pattern::new(
            &[&["t"]],
            &Legend::new().cell('t', CellMatcher::Tag(ORE_TAG)),
        )
        .unwrap();
        let mut plain = TestWorld::new();
        plain.set([0, 0, 0], STONE, 0);
        assert!(
            tagged
                .matches(&plain, [0, 0, 0], PatternPlacement::default())
                .is_none()
        );
        plain.in_tag(STONE, ORE_TAG);
        assert!(
            tagged
                .matches(&plain, [0, 0, 0], PatternPlacement::default())
                .is_some()
        );
    }

    #[test]
    fn refuses_a_position_an_i32_cannot_name() {
        let legend = Legend::new().cell('.', CellMatcher::Any);
        let mut world = TestWorld::new();
        world.set([i32::MAX, 0, 0], STONE, 0);
        world.set([i32::MIN, 0, 0], STONE, 0);

        // A two-cell row at the largest origin: the second cell would be i32::MAX + 1.
        let pair = Pattern::new(&[&[".."]], &legend).unwrap();
        assert!(
            pair.matches(&world, [i32::MAX, 0, 0], PatternPlacement::default())
                .is_none()
        );
        // One cell fits there.
        let single = Pattern::new(&[&["."]], &legend).unwrap();
        let found = single
            .matches(
                &world,
                [i32::MAX, i32::MAX, i32::MAX],
                PatternPlacement::default(),
            )
            .unwrap();
        assert_eq!(found.position([0, 0, 0]), Some([i32::MAX; 3]));

        // A mirrored width-one pattern subtracts the local coordinate: the smallest one wraps.
        let mirrored = PatternPlacement {
            rotation: StructureRotation::None,
            mirrored: true,
        };
        let found = single.matches(&world, [0, 0, 0], mirrored).unwrap();
        assert_eq!(found.position([i32::MIN, 0, 0]), None);
        assert_eq!(found.position([0, 0, 0]), Some([0, 0, 0]));
    }

    #[test]
    fn reports_the_construction_failure_it_names() {
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
        for layers in [vec![], vec![vec![]], vec![vec![""]]] {
            let rows: Vec<&[&str]> = layers.iter().map(|rows| rows.as_slice()).collect();
            assert_eq!(Pattern::new(&rows, &legend), Err(PatternError::Empty));
        }
        assert_eq!(
            PatternError::Empty.to_string(),
            "the pattern has no layers, or its first layer has no rows or a row with no cells"
        );
        assert_eq!(
            PatternError::UnevenRow { layer: 1, row: 2 }.to_string(),
            "layer 1 row 2 is not as wide as the pattern's first row"
        );
    }
}
