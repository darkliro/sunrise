//! The typed form of a `data/<namespace>/recipe_filter/*.json` entry.
//!
//! A filter matches recipes by all of its present criteria and then removes
//! them or swaps one input or output. A selector is built from validated
//! values, so a malformed reference is refused where it is read instead of
//! widening the filter. Matching and application stay with the engine, which
//! reads the recipe file once; these values describe what to match.

use crate::Identifier;
use std::fmt;

/// Why a recipe selector is not a resource reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum SelectorError {
    /// The selector names nothing.
    Empty,
    /// A recipe id, type, input or output must be written `namespace:path`.
    MissingColon,
    /// The namespace holds a character a resource namespace cannot hold.
    InvalidNamespace,
    /// The path holds a character a resource path cannot hold, or the text
    /// has more than one namespace separator.
    InvalidPath,
}

impl fmt::Display for SelectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "empty recipe selector",
            Self::MissingColon => "recipe selector needs a namespace:path reference",
            Self::InvalidNamespace => "invalid recipe selector namespace",
            Self::InvalidPath => "invalid recipe selector path",
        })
    }
}

impl std::error::Error for SelectorError {}

/// Split a fully qualified `namespace:path` reference, refusing a missing,
/// empty or repeated namespace separator before the identifier rules run.
fn qualified(text: &str) -> Result<Identifier, SelectorError> {
    if text.is_empty() {
        return Err(SelectorError::Empty);
    }
    let Some((namespace, path)) = text.split_once(':') else {
        return Err(SelectorError::MissingColon);
    };
    if namespace.is_empty() || path.is_empty() {
        return Err(SelectorError::Empty);
    }
    if path.contains(':') {
        return Err(SelectorError::InvalidPath);
    }
    Identifier::from_parts(namespace, path).map_err(|error| match error {
        sunrise_core::identifier::IdentifierError::Namespace => SelectorError::InvalidNamespace,
        sunrise_core::identifier::IdentifierError::Path => SelectorError::InvalidPath,
    })
}

/// A resource namespace on its own (`minecraft`, `gtnh`, `1steel`), validated
/// with the identifier rules the registry uses.
// exhaustive: its field is private, so only `parse` builds one.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[wire(via = Identifier)]
pub struct Namespace(Identifier);

impl From<&Namespace> for Identifier {
    fn from(value: &Namespace) -> Self {
        value.0.clone()
    }
}

impl TryFrom<Identifier> for Namespace {
    type Error = ();

    fn try_from(value: Identifier) -> Result<Self, Self::Error> {
        value.path().is_empty().then_some(Self(value)).ok_or(())
    }
}

impl Namespace {
    /// Validate `namespace` as an identifier namespace. A selector namespace is
    /// written without a colon; path characters, emptiness and uppercase letters
    /// are refused.
    pub fn parse(namespace: &str) -> Result<Self, SelectorError> {
        if namespace.is_empty() {
            return Err(SelectorError::Empty);
        }
        if namespace.contains(':') {
            return Err(SelectorError::InvalidNamespace);
        }
        match Identifier::from_parts(namespace, "") {
            Ok(identifier) => Ok(Self(identifier)),
            Err(sunrise_core::identifier::IdentifierError::Namespace) => {
                Err(SelectorError::InvalidNamespace)
            }
            Err(sunrise_core::identifier::IdentifierError::Path) => Err(SelectorError::InvalidPath),
        }
    }

    /// The validated namespace, without a colon.
    pub fn as_str(&self) -> &str {
        self.0.namespace()
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A recipe id or type pattern: a fully qualified resource reference where
/// `*` matches any run of characters (`minecraft:*_planks`, `mymod:*`).
// exhaustive: its field is private, so only `parse` builds one.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[wire(via = String)]
pub struct RecipePattern(String);

impl RecipePattern {
    /// Validate `text` as a `namespace:path` pattern. A `*` stands in for any
    /// run of characters on either side of the colon; the text around it must
    /// still follow the registry's identifier rules.
    pub fn parse(text: &str) -> Result<Self, SelectorError> {
        // Validate the pattern's literal parts without treating `*` as a path character.
        qualified(&text.replace('*', "x"))?;
        Ok(Self(text.to_owned()))
    }

    /// The pattern as written, `*` included.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&RecipePattern> for String {
    fn from(value: &RecipePattern) -> Self {
        value.0.clone()
    }
}

impl TryFrom<String> for RecipePattern {
    type Error = ();

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|_| ())
    }
}

impl fmt::Display for RecipePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An item reference in a recipe filter: a concrete item, or a tag written
/// `#namespace:path`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum RecipeItem {
    /// An item id (`minecraft:iron_ingot`).
    Item(Identifier),
    /// A tag id, written with a leading `#` in a filter (`#c:ingots`).
    Tag(Identifier),
}

impl RecipeItem {
    /// Parse an item id or a `#tag` reference. Both need a namespace.
    pub fn parse(text: &str) -> Result<Self, SelectorError> {
        match text.strip_prefix('#') {
            Some(tag) => Ok(Self::Tag(qualified(tag)?)),
            None => Ok(Self::Item(qualified(text)?)),
        }
    }

    /// The item or tag id, without the leading `#`.
    pub fn identifier(&self) -> &Identifier {
        match self {
            Self::Item(id) | Self::Tag(id) => id,
        }
    }

    /// Whether this reference names a tag.
    pub fn is_tag(&self) -> bool {
        matches!(self, Self::Tag(_))
    }
}

impl fmt::Display for RecipeItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_tag() {
            f.write_str("#")?;
        }
        f.write_str(self.identifier().as_str())
    }
}

/// What a matching filter does: remove the recipe, or swap one input or
/// output reference for another.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(sunrise_mod_api_macros::Wire)]
pub enum RecipeAction {
    /// The recipe is not loaded.
    Remove,
    /// Every input that is `from` becomes `to`.
    ReplaceInput {
        /// The reference to replace.
        from: RecipeItem,
        /// The replacement.
        to: RecipeItem,
    },
    /// Every output that is `from` becomes `to`.
    ReplaceOutput {
        /// The reference to replace.
        from: RecipeItem,
        /// The replacement.
        to: RecipeItem,
    },
}

/// The criteria of one recipe filter. Every present criterion must hold; an
/// empty selector matches every recipe, as vanilla's filter format says.
// exhaustive: its fields are private, so only the `with_*` builders set them.
#[derive(Clone, Debug, Default, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
pub struct RecipeSelector {
    id: Option<RecipePattern>,
    kind: Option<RecipePattern>,
    namespace: Option<Namespace>,
    input: Option<RecipeItem>,
    output: Option<RecipeItem>,
}

impl RecipeSelector {
    /// A selector with no criterion, matching every recipe.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an id pattern (`minecraft:*_planks`).
    pub fn with_id(mut self, id: RecipePattern) -> Self {
        self.id = Some(id);
        self
    }

    /// Add a recipe type pattern (`mymod:*`).
    pub fn with_kind(mut self, kind: RecipePattern) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Add a namespace criterion (`gtnh`).
    pub fn with_namespace(mut self, namespace: Namespace) -> Self {
        self.namespace = Some(namespace);
        self
    }

    /// Add an input item or tag criterion.
    pub fn with_input(mut self, input: RecipeItem) -> Self {
        self.input = Some(input);
        self
    }

    /// Add an output item or tag criterion.
    pub fn with_output(mut self, output: RecipeItem) -> Self {
        self.output = Some(output);
        self
    }

    /// The id pattern, when one is set.
    pub fn id(&self) -> Option<&RecipePattern> {
        self.id.as_ref()
    }

    /// The recipe type pattern, when one is set.
    pub fn kind(&self) -> Option<&RecipePattern> {
        self.kind.as_ref()
    }

    /// The namespace criterion, when one is set.
    pub fn namespace(&self) -> Option<&Namespace> {
        self.namespace.as_ref()
    }

    /// The input criterion, when one is set.
    pub fn input(&self) -> Option<&RecipeItem> {
        self.input.as_ref()
    }

    /// The output criterion, when one is set.
    pub fn output(&self) -> Option<&RecipeItem> {
        self.output.as_ref()
    }

    /// Whether no criterion is set, so every recipe matches.
    pub fn is_empty(&self) -> bool {
        self.id.is_none()
            && self.kind.is_none()
            && self.namespace.is_none()
            && self.input.is_none()
            && self.output.is_none()
    }
}

/// One typed `recipe_filter` entry: its criteria and its one action.
// exhaustive: its fields are private, so `new` is the only constructor.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
pub struct RecipeFilter {
    selector: RecipeSelector,
    action: RecipeAction,
}

impl RecipeFilter {
    /// A filter with `selector` and `action`.
    pub fn new(selector: RecipeSelector, action: RecipeAction) -> Self {
        Self { selector, action }
    }

    /// The criteria every match must hold.
    pub fn selector(&self) -> &RecipeSelector {
        &self.selector
    }

    /// What a match does.
    pub fn action(&self) -> &RecipeAction {
        &self.action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identifier(text: &str) -> Identifier {
        Identifier::parse(text).unwrap()
    }

    #[test]
    fn patterns_accept_a_glob_on_either_side_of_the_colon() {
        assert!(RecipePattern::parse("minecraft:*_planks").is_ok());
        assert!(RecipePattern::parse("mymod:*").is_ok());
        assert!(RecipePattern::parse("*:oak_*").is_ok());
        assert!(RecipePattern::parse("1steel:steel-ingot").is_ok());
        assert_eq!(
            RecipePattern::parse("minecraft:*_planks").unwrap().as_str(),
            "minecraft:*_planks"
        );
    }

    #[test]
    fn malformed_patterns_are_refused_with_a_named_error() {
        assert_eq!(RecipePattern::parse(""), Err(SelectorError::Empty));
        assert_eq!(
            RecipePattern::parse("*_planks"),
            Err(SelectorError::MissingColon)
        );
        assert_eq!(RecipePattern::parse(":planks"), Err(SelectorError::Empty));
        assert_eq!(RecipePattern::parse("mod:"), Err(SelectorError::Empty));
        assert_eq!(
            RecipePattern::parse("Mod:*"),
            Err(SelectorError::InvalidNamespace)
        );
        assert_eq!(
            RecipePattern::parse("mod:a:b"),
            Err(SelectorError::InvalidPath)
        );
    }

    #[test]
    fn items_and_tags_keep_their_reference_kind() {
        let item = RecipeItem::parse("minecraft:iron_ingot").unwrap();
        assert!(!item.is_tag());
        assert_eq!(item.identifier().as_str(), "minecraft:iron_ingot");
        assert_eq!(item.to_string(), "minecraft:iron_ingot");
        let tag = RecipeItem::parse("#c:ingots").unwrap();
        assert!(tag.is_tag());
        assert_eq!(tag.identifier().as_str(), "c:ingots");
        assert_eq!(tag.to_string(), "#c:ingots");
        assert_eq!(RecipeItem::parse("#"), Err(SelectorError::Empty));
        assert_eq!(
            RecipeItem::parse("c:ingots"),
            Ok(RecipeItem::Item(identifier("c:ingots")))
        );
        assert_eq!(
            RecipeItem::parse("ingots"),
            Err(SelectorError::MissingColon)
        );
    }

    #[test]
    fn selectors_carry_every_criterion_and_start_empty() {
        let selector = RecipeSelector::new();
        assert!(selector.is_empty());
        let selector = selector
            .with_id(RecipePattern::parse("minecraft:*_planks").unwrap())
            .with_kind(RecipePattern::parse("minecraft:crafting_*").unwrap())
            .with_namespace(Namespace::parse("minecraft").unwrap())
            .with_input(RecipeItem::parse("#minecraft:oak_logs").unwrap())
            .with_output(RecipeItem::parse("minecraft:oak_planks").unwrap());
        assert!(!selector.is_empty());
        assert_eq!(selector.id().unwrap().as_str(), "minecraft:*_planks");
        assert_eq!(selector.kind().unwrap().as_str(), "minecraft:crafting_*");
        assert_eq!(selector.namespace().unwrap().as_str(), "minecraft");
        assert_eq!(selector.input().unwrap().to_string(), "#minecraft:oak_logs");
        assert_eq!(
            selector.output().unwrap().to_string(),
            "minecraft:oak_planks"
        );
    }

    #[test]
    fn a_filter_carries_its_selector_and_one_action() {
        let selector = RecipeSelector::new().with_namespace(Namespace::parse("gtnh").unwrap());
        let filter = RecipeFilter::new(selector.clone(), RecipeAction::Remove);
        assert_eq!(filter.selector(), &selector);
        assert_eq!(filter.action(), &RecipeAction::Remove);
        let swap = RecipeAction::ReplaceInput {
            from: RecipeItem::parse("#c:ingots/iron").unwrap(),
            to: RecipeItem::parse("#c:ingots/gold").unwrap(),
        };
        assert!(matches!(swap, RecipeAction::ReplaceInput { .. }));
    }

    #[test]
    fn resource_paths_that_start_with_a_digit_or_hold_a_dash_are_valid() {
        assert!(RecipeItem::parse("1steel:steel-ingot").is_ok());
        assert!(Namespace::parse("1steel").is_ok());
        assert!(Namespace::parse("steel-ingot").is_ok());
    }

    #[test]
    fn namespaces_are_validated_like_registry_namespaces() {
        assert_eq!(Namespace::parse(""), Err(SelectorError::Empty));
        assert_eq!(
            Namespace::parse("mymod:path"),
            Err(SelectorError::InvalidNamespace)
        );
        assert_eq!(
            Namespace::parse("MyMod"),
            Err(SelectorError::InvalidNamespace)
        );
    }

    #[test]
    fn wire_round_trips_a_filter() {
        let filter = RecipeFilter::new(
            RecipeSelector::new()
                .with_kind(RecipePattern::parse("mymod:*").unwrap())
                .with_namespace(Namespace::parse("mymod").unwrap())
                .with_output(RecipeItem::parse("mymod:dust").unwrap()),
            RecipeAction::Remove,
        );
        let mut out = crate::wire::Writer::new();
        out.put(&filter);
        assert_eq!(
            crate::wire::Reader::decode::<RecipeFilter>(out.bytes()).unwrap(),
            filter
        );
    }

    #[test]
    fn wire_refuses_a_pattern_a_constructor_would_refuse() {
        let mut out = crate::wire::Writer::new();
        out.put(&String::from("not qualified"));
        assert_eq!(
            crate::wire::Reader::decode::<RecipePattern>(out.bytes()),
            Err(crate::wire::WireError::Invalid("RecipePattern"))
        );
        let mut out = crate::wire::Writer::new();
        out.put(&identifier("1steel:steel-ingot"));
        assert_eq!(
            crate::wire::Reader::decode::<Namespace>(out.bytes()),
            Err(crate::wire::WireError::Invalid("Namespace"))
        );
    }
}
