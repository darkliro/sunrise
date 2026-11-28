//! The multiblock pattern the book's how-to page includes. A mod's real pattern is data from its
//! block entity; this one keeps the page compiled.

use mod_api::{BlockKey, CellMatcher, Legend, Pattern, PatternPlacement, PatternWorld, TagKey};

// ANCHOR: pattern
/// A 3x3x3 steam casing: `steel` everywhere, a controller in the middle of the top layer and two
/// hatch slots in the bottom layer, on the north and south faces.
pub fn steam_casing(steel: BlockKey, controller: BlockKey, hatches: TagKey<BlockKey>) -> Pattern {
    let legend = Legend::new()
        .cell('s', CellMatcher::Block(steel))
        .role('C', CellMatcher::Block(controller), "controller")
        .role('H', CellMatcher::Tag(hatches), "hatch");
    let layers: &[&[&str]] = &[
        &["sHs", "sss", "sHs"],
        &["sss", "sss", "sss"],
        &["sss", "sCs", "sss"],
    ];
    Pattern::new(layers, &legend)
        .expect("the casing layers are rectangular and use only legend symbols")
}
// ANCHOR_END: pattern

// ANCHOR: match
/// Form the casing with its minimum corner at `origin`; on success, give the world positions of its
/// hatch slots so the caller can read each one's capability.
pub fn hatch_positions(
    casing: &Pattern,
    world: &dyn PatternWorld,
    origin: [i32; 3],
) -> Option<Vec<[i32; 3]>> {
    let formed = casing.matches(world, origin, PatternPlacement::default())?;
    Some(formed.positions("hatch").collect())
}
// ANCHOR_END: match

/// The same check on a game side, proving a `ServerHost` and its `LevelRef` reach the level read.
pub fn forms_in(
    host: &dyn mod_api::ServerHost,
    level: mod_api::LevelRef,
    casing: &Pattern,
    origin: [i32; 3],
) -> bool {
    casing
        .matches_level(level, host, origin, PatternPlacement::default())
        .is_some()
}
