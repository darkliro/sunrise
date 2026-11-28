//! Recipe filters: data files that remove recipes, or swap one of their
//! inputs or outputs, by what the recipes are rather than by id.
//!
//! A file `data/<ns>/recipe_filter/<path>.json` holds `{"filters": [...]}`.
//! Each filter has match keys, all of which must hold (none matches every
//! recipe), and exactly one action:
//! - `id`: the recipe id, `*` matching any run of characters
//!   (`minecraft:*_planks`);
//! - `type`: the recipe's `type`, with the same `*`;
//! - `namespace`: the recipe id's namespace (`gtceu`);
//! - `input`: an item id or `#tag` among the recipe's inputs;
//! - `output`: an item id or `#tag` among its outputs (under `result`,
//!   `results`, `output` or `outputs`);
//! - `remove: true`, or `replace_input` / `replace_output`
//!   `{"from": ..., "to": ...}`, which swaps every input (or output) that is
//!   `from` for `to`.
//!
//! Filters run on each recipe after the data packs are merged and its load
//! conditions held, so they see the final set and reach every recipe type,
//! mods' own included: they work on the recipe's JSON. Files apply in id
//! order, each file's filters in order. A pack's file replaces a file of the
//! same id, as any data file does. A filter with an unknown key, an invalid
//! reference or no action is skipped with a warning, so a typo never matches
//! every recipe. Each entry is read into the typed values the mod API
//! publishes (`RecipeFilter`); matching and application stay here.
use mod_vocab::{Namespace, RecipeAction, RecipeFilter, RecipeItem, RecipePattern, RecipeSelector};
use serde_json::Value;
use std::path::Path;
use sunrise_world::loot::PackOverlay;

/// The keys under which a recipe lists what it makes.
const OUTPUT_KEYS: [&str; 4] = ["result", "results", "output", "outputs"];
/// Keys whose strings are never items: a type, a group, a category, either loader's load
/// conditions.
const NOT_ITEMS: [&str; 5] = [
    "type",
    "group",
    "category",
    sunrise_world::conditions::NEOFORGE_KEY,
    sunrise_world::conditions::FABRIC_KEY,
];

/// Every recipe filter of a load, in the order they apply.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecipeFilters {
    filters: Vec<RecipeFilter>,
}

impl RecipeFilters {
    /// The filters of the content root's mods (`data/<ns>/recipe_filter/`)
    /// with the world's packs' files over them.
    pub fn load(root: &Path, overlay: &PackOverlay) -> Self {
        let mut files: std::collections::BTreeMap<String, Vec<u8>> = Default::default();
        for ns in crate::content_namespaces::namespaces(root) {
            let dir = root.join("data").join(&ns).join("recipe_filter");
            read_dir(&dir, &format!("{ns}:"), &mut files);
        }
        for (id, versions) in overlay.kind("recipe_filter") {
            files.insert(id.to_owned(), PackOverlay::top(versions).to_vec());
        }
        Self::parse(files, overlay.conditions)
    }

    fn parse(
        files: std::collections::BTreeMap<String, Vec<u8>>,
        conditions: &sunrise_world::conditions::LoadConditions,
    ) -> Self {
        let mut filters = Vec::new();
        for (file, bytes) in files {
            let v: Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("Sunrise: recipe filter {file} not loaded: {e}");
                    continue;
                }
            };
            if !conditions.allows_id(&v, "recipe_filter", &file) {
                continue;
            }
            for (i, f) in v["filters"].as_array().into_iter().flatten().enumerate() {
                match parse_filter(f) {
                    Ok(f) => filters.push(f),
                    Err(e) => log::warn!("Sunrise: recipe filter {file} #{i} skipped: {e}"),
                }
            }
        }
        Self { filters }
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    /// Runs the filters over recipe `id` (`ns:path`): false when one removes
    /// it, else `recipe` with every swap applied.
    pub fn apply(&self, id: &str, recipe: &mut Value) -> bool {
        for filter in &self.filters {
            if !matches(filter.selector(), id, recipe) {
                continue;
            }
            match filter.action() {
                RecipeAction::Remove => return false,
                RecipeAction::ReplaceInput { from, to } => replace(recipe, from, to, false),
                RecipeAction::ReplaceOutput { from, to } => replace(recipe, from, to, true),
                _ => {}
            }
        }
        true
    }
}

fn read_dir(dir: &Path, prefix: &str, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            read_dir(&path, &format!("{prefix}{name}/"), out);
        } else if let Some(stem) = name.strip_suffix(".json")
            && let Ok(bytes) = std::fs::read(&path)
        {
            out.insert(format!("{prefix}{stem}"), bytes);
        }
    }
}

/// Read one JSON entry into its typed selector and action. A refusal names
/// the field, so a skipped filter is fixable from the log line.
fn parse_filter(v: &Value) -> Result<RecipeFilter, String> {
    let obj = v.as_object().ok_or("not an object")?;
    let text = |k: &str| -> Result<Option<String>, String> {
        match obj.get(k) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(format!("`{k}` is not a string")),
        }
    };
    let swap = |k: &str| -> Result<Option<(RecipeItem, RecipeItem)>, String> {
        let Some(s) = obj.get(k) else {
            return Ok(None);
        };
        match (s["from"].as_str(), s["to"].as_str()) {
            (Some(from), Some(to)) => Ok(Some((
                RecipeItem::parse(from).map_err(|e| format!("`{k}.from`: {e}"))?,
                RecipeItem::parse(to).map_err(|e| format!("`{k}.to`: {e}"))?,
            ))),
            _ => Err(format!("`{k}` needs `from` and `to` strings")),
        }
    };
    for k in obj.keys() {
        if !matches!(
            k.as_str(),
            "id" | "type"
                | "namespace"
                | "input"
                | "output"
                | "remove"
                | "replace_input"
                | "replace_output"
        ) {
            return Err(format!("unknown key `{k}`"));
        }
    }
    let mut actions = Vec::new();
    match obj.get("remove") {
        None | Some(Value::Bool(false)) => {}
        Some(Value::Bool(true)) => actions.push(RecipeAction::Remove),
        Some(_) => return Err("`remove` is not a boolean".into()),
    }
    if let Some((from, to)) = swap("replace_input")? {
        actions.push(RecipeAction::ReplaceInput { from, to });
    }
    if let Some((from, to)) = swap("replace_output")? {
        actions.push(RecipeAction::ReplaceOutput { from, to });
    }
    let [action] = <[RecipeAction; 1]>::try_from(actions)
        .map_err(|_| "needs exactly one of `remove`, `replace_input`, `replace_output`")?;

    let mut selector = RecipeSelector::new();
    if let Some(id) = text("id")? {
        selector = selector.with_id(RecipePattern::parse(&id).map_err(|e| format!("`id`: {e}"))?);
    }
    if let Some(kind) = text("type")? {
        selector =
            selector.with_kind(RecipePattern::parse(&kind).map_err(|e| format!("`type`: {e}"))?);
    }
    if let Some(namespace) = text("namespace")? {
        selector = selector
            .with_namespace(Namespace::parse(&namespace).map_err(|e| format!("`namespace`: {e}"))?);
    }
    if let Some(input) = text("input")? {
        selector =
            selector.with_input(RecipeItem::parse(&input).map_err(|e| format!("`input`: {e}"))?);
    }
    if let Some(output) = text("output")? {
        selector =
            selector.with_output(RecipeItem::parse(&output).map_err(|e| format!("`output`: {e}"))?);
    }
    Ok(RecipeFilter::new(selector, action))
}

/// Whether every criterion of `selector` holds for recipe `id` and its JSON.
fn matches(selector: &RecipeSelector, id: &str, recipe: &Value) -> bool {
    selector.id().is_none_or(|p| glob(p.as_str(), id))
        && selector
            .kind()
            .is_none_or(|p| glob(p.as_str(), recipe["type"].as_str().unwrap_or("")))
        && selector
            .namespace()
            .is_none_or(|ns| id.split_once(':').is_some_and(|(n, _)| n == ns.as_str()))
        && selector
            .input()
            .is_none_or(|i| contains(recipe, i, false, false))
        && selector
            .output()
            .is_none_or(|o| contains(recipe, o, false, true))
}

/// `pattern` with `*` as any run of characters, against `s`.
fn glob(pattern: &str, s: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = s.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// Whether `s` at `key` is `item`. A `tag` key's value is a tag without its
/// `#`; an item is never a tag, and the other way round.
fn references(key: Option<&str>, s: &str, item: &RecipeItem) -> bool {
    if item.is_tag() {
        (key == Some("tag") && s == item.identifier().as_str())
            || s.strip_prefix('#') == Some(item.identifier().as_str())
    } else {
        key != Some("tag") && s == item.identifier().as_str()
    }
}

/// Whether `item` is among `v`'s inputs (`outputs` false) or outputs
/// (`outputs` true). `inside` says the walk is already under an output key.
fn contains(v: &Value, item: &RecipeItem, inside: bool, outputs: bool) -> bool {
    fn walk(v: &Value, key: Option<&str>, item: &RecipeItem, inside: bool, outputs: bool) -> bool {
        match v {
            Value::String(s) => inside == outputs && references(key, s, item),
            Value::Array(a) => a.iter().any(|v| walk(v, key, item, inside, outputs)),
            Value::Object(o) => o.iter().any(|(k, v)| {
                !NOT_ITEMS.contains(&k.as_str())
                    && walk(
                        v,
                        Some(k),
                        item,
                        inside || OUTPUT_KEYS.contains(&k.as_str()),
                        outputs,
                    )
            }),
            _ => false,
        }
    }
    walk(v, None, item, inside, outputs)
}

/// Swaps every input (`outputs` false) or output that is `from` for `to`.
fn replace(v: &mut Value, from: &RecipeItem, to: &RecipeItem, outputs: bool) {
    fn walk(
        v: &mut Value,
        key: Option<&str>,
        from: &RecipeItem,
        to: &RecipeItem,
        inside: bool,
        outputs: bool,
    ) {
        match v {
            Value::String(s) => {
                if inside == outputs && references(key, s, from) {
                    // A `tag` key holds the tag without its `#`.
                    *s = if to.is_tag() {
                        if key == Some("tag") {
                            to.identifier().as_str().to_owned()
                        } else {
                            format!("#{}", to.identifier().as_str())
                        }
                    } else {
                        to.identifier().as_str().to_owned()
                    };
                }
            }
            Value::Array(a) => a
                .iter_mut()
                .for_each(|v| walk(v, key, from, to, inside, outputs)),
            Value::Object(o) => {
                for (k, v) in o.iter_mut() {
                    if !NOT_ITEMS.contains(&k.as_str()) {
                        let inside = inside || OUTPUT_KEYS.contains(&k.as_str());
                        walk(v, Some(k), from, to, inside, outputs);
                    }
                }
            }
            _ => {}
        }
    }
    walk(v, None, from, to, false, outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn filters(v: Value) -> RecipeFilters {
        RecipeFilters::parse(
            [("test:f".to_owned(), v.to_string().into_bytes())].into(),
            &sunrise_world::conditions::LoadConditions::vanilla(),
        )
    }

    #[test]
    fn globs_match_runs() {
        assert!(glob("minecraft:*_planks", "minecraft:oak_planks"));
        assert!(glob("*", "anything"));
        assert!(glob("a*b*c", "a-x-b-y-c"));
        assert!(!glob("a*b*c", "a-x-c"));
        assert!(!glob("minecraft:oak", "minecraft:oak_planks"));
        assert!(glob("*:oak_*", "mod:oak_log"));
        assert!(!glob("ab*ba", "aba"));
    }

    #[test]
    fn remove_by_id_type_and_items() {
        let planks = json!({"type": "minecraft:crafting_shapeless",
            "ingredients": ["#minecraft:oak_logs"], "result": {"id": "minecraft:oak_planks", "count": 4}});
        let f = filters(json!({"filters": [{"id": "minecraft:*_planks", "remove": true}]}));
        assert!(!f.apply("minecraft:oak_planks", &mut planks.clone()));
        assert!(f.apply("minecraft:stick", &mut planks.clone()));
        // The input and output keys look on their own side only.
        let f = filters(json!({"filters": [{"output": "#minecraft:oak_logs", "remove": true}]}));
        assert!(f.apply("x:y", &mut planks.clone()));
        let f = filters(json!({"filters": [{"input": "#minecraft:oak_logs",
            "type": "minecraft:crafting_*", "remove": true}]}));
        assert!(!f.apply("x:y", &mut planks.clone()));
        let f = filters(
            json!({"filters": [{"output": "minecraft:oak_planks", "type": "minecraft:smelting", "remove": true}]}),
        );
        assert!(f.apply("x:y", &mut planks.clone()));
    }

    #[test]
    fn a_bare_star_pattern_matches_every_recipe() {
        let f = filters(json!({"filters": [{"id": "*", "remove": true}]}));
        assert!(!f.apply("minecraft:stick", &mut json!({})));
        let f = filters(json!({"filters": [{"type": "*", "remove": true}]}));
        assert!(!f.apply("mymod:any", &mut json!({"type": "gtceu:macerator"})));
    }

    #[test]
    fn remove_by_namespace() {
        let f = filters(json!({"filters": [{"namespace": "gtceu", "remove": true}]}));
        assert!(!f.apply("gtceu:macerator/iron", &mut json!({})));
        assert!(f.apply("minecraft:stick", &mut json!({})));
        // A namespace is not a path prefix: `gtceu_extra` is another namespace.
        assert!(f.apply("gtceu_extra:machine", &mut json!({})));
    }

    #[test]
    fn swaps_reach_mod_types_and_tag_objects() {
        let mut machine = json!({"type": "gtceu:macerator",
            "inputs": {"item": [{"tag": "c:ingots/iron"}, "minecraft:iron_ingot"]},
            "outputs": {"item": ["minecraft:iron_ingot"]}});
        let f = filters(json!({"filters": [
            {"replace_input": {"from": "#c:ingots/iron", "to": "#c:ingots/gold"}},
            {"type": "gtceu:*", "replace_output": {"from": "minecraft:iron_ingot", "to": "gtceu:iron_dust"}}
        ]}));
        assert!(f.apply("gtceu:macerator/iron", &mut machine));
        assert_eq!(machine["inputs"]["item"][0]["tag"], "c:ingots/gold");
        // Only outputs were swapped for the item: the input stays.
        assert_eq!(machine["inputs"]["item"][1], "minecraft:iron_ingot");
        assert_eq!(machine["outputs"]["item"][0], "gtceu:iron_dust");
        // `type` is never an item.
        assert_eq!(machine["type"], "gtceu:macerator");
    }

    #[test]
    fn bad_filters_are_skipped_not_widened() {
        let f = filters(json!({"filters": [
            {"Id": "minecraft:stick", "remove": true},
            {"id": "minecraft:stick"},
            {"remove": true, "replace_input": {"from": "a:b", "to": "c:d"}},
            {"replace_input": {"from": "a:b"}},
            {"namespace": "MyMod", "remove": true},
            {"id": "stick", "remove": true},
            {"output": "#", "remove": true}
        ]}));
        assert!(f.is_empty());
        assert!(
            RecipeFilters::parse(
                [("t:f".to_owned(), b"{nope".to_vec())].into(),
                &sunrise_world::conditions::LoadConditions::vanilla(),
            )
            .is_empty()
        );
    }

    #[test]
    fn packs_replace_a_mods_file_and_files_apply_in_id_order() {
        let root =
            std::env::temp_dir().join(format!("sunrise-recipe-filters-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("data/mymod/recipe_filter");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            root.join(crate::content_namespaces::MARKER),
            json!(["mymod"]).to_string(),
        )
        .unwrap();
        let remove_sticks = json!({"filters": [{"id": "minecraft:stick", "remove": true}]});
        std::fs::write(dir.join("a.json"), remove_sticks.to_string()).unwrap();
        let swap = json!({"filters": [{"replace_output": {"from": "minecraft:stone", "to": "minecraft:dirt"}}]});
        std::fs::write(dir.join("b.json"), swap.to_string()).unwrap();
        let mut stone = json!({"type": "minecraft:smelting", "ingredient": "minecraft:cobblestone", "result": {"id": "minecraft:stone"}});
        let f = RecipeFilters::load(&root, &PackOverlay::NONE);
        assert!(!f.apply("minecraft:stick", &mut json!({})));
        assert!(f.apply("minecraft:stone", &mut stone));
        assert_eq!(stone["result"]["id"], "minecraft:dirt");
        // A pack's file of the same id replaces the mod's: sticks stay.
        let files = vec![(
            "recipe_filter/mymod:a".to_owned(),
            vec![json!({"filters": []}).to_string().into_bytes()],
        )];
        let f = RecipeFilters::load(&root, &PackOverlay::new(&files));
        assert!(f.apply("minecraft:stick", &mut json!({})));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn recipe_filters_evaluate_load_conditions_on_winning_files() {
        let root = std::env::temp_dir().join(format!(
            "sunrise-recipe-filters-cond-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("data/mymod/recipe_filter");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            root.join(crate::content_namespaces::MARKER),
            json!(["mymod"]).to_string(),
        )
        .unwrap();
        // Guarded by neoforge:mod_loaded("chemlib").
        let guarded = json!({
            "neoforge:conditions": [{"type": "neoforge:mod_loaded", "modid": "chemlib"}],
            "filters": [{"id": "minecraft:stick", "remove": true}]
        });
        std::fs::write(dir.join("guarded.json"), guarded.to_string()).unwrap();
        // Unconditional baseline overridden by a higher pack whose condition fails:
        // the failing higher pack must omit the filter without uncovering the baseline.
        let base = json!({
            "filters": [{"id": "minecraft:bowl", "remove": true}]
        });
        std::fs::write(dir.join("overridden.json"), base.to_string()).unwrap();
        let files = vec![(
            "recipe_filter/mymod:overridden".to_owned(),
            vec![
                json!({
                    "neoforge:conditions": [{"type": "neoforge:mod_loaded", "modid": "absent"}],
                    "filters": [{"id": "minecraft:bowl", "remove": true}]
                })
                .to_string()
                .into_bytes(),
            ],
        )];

        let with_chemlib = sunrise_world::conditions::LoadConditions::new(["chemlib"]);
        let overlay = PackOverlay {
            conditions: &with_chemlib,
            files: &files,
            ..PackOverlay::default()
        };
        let loaded = RecipeFilters::load(&root, &overlay);
        assert!(
            !loaded.apply("minecraft:stick", &mut json!({})),
            "guarded filter must run when chemlib is loaded"
        );
        assert!(
            loaded.apply("minecraft:bowl", &mut json!({})),
            "failing higher override must not uncover lower filter"
        );

        let without_chemlib = sunrise_world::conditions::LoadConditions::vanilla();
        let overlay_off = PackOverlay {
            conditions: &without_chemlib,
            files: &files,
            ..PackOverlay::default()
        };
        let loaded_off = RecipeFilters::load(&root, &overlay_off);
        assert!(
            loaded_off.apply("minecraft:stick", &mut json!({})),
            "guarded filter must be omitted when chemlib is absent"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
