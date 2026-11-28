//! Typed recipe filters from the modding book.
use mod_api::{
    Namespace, RecipeAction, RecipeFilter, RecipeItem, RecipePattern, RecipeSelector, SelectorError,
};

// ANCHOR: filters
/// The filters a pack declares, already validated.
pub fn filters() -> Result<Vec<RecipeFilter>, SelectorError> {
    Ok(vec![
        // Every recipe that makes iron ingots in one namespace.
        RecipeFilter::new(
            RecipeSelector::new()
                .with_namespace(Namespace::parse("examplemod")?)
                .with_output(RecipeItem::parse("minecraft:iron_ingot")?),
            RecipeAction::Remove,
        ),
        // One recipe type that accepts an ore tag, instead of a list of ids.
        RecipeFilter::new(
            RecipeSelector::new()
                .with_kind(RecipePattern::parse("examplemod:crushing")?)
                .with_input(RecipeItem::parse("#c:ores/iron")?),
            RecipeAction::Remove,
        ),
        // Swap one input without removing the recipe.
        RecipeFilter::new(
            RecipeSelector::new().with_input(RecipeItem::parse("#c:ores/iron")?),
            RecipeAction::ReplaceInput {
                from: RecipeItem::parse("minecraft:iron_ore")?,
                to: RecipeItem::parse("minecraft:raw_iron")?,
            },
        ),
    ])
}
// ANCHOR_END: filters
