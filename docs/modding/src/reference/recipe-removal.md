# Recipe removal and filters

A removal drops recipes from the table in force; a filter can also swap one of a recipe's inputs or outputs.
The declarative format is `data/<namespace>/recipe_filter/*.json`, read after the packs are merged and their
load conditions held, before the recipes are parsed. The full file format, its wildcard rules and its build
order are on [Items, recipes and loot](../how-to/items/recipe-rules.md#recipe-filters); this page covers the
typed values the game reads each entry into.

Removing is not the same as disabling a pack: a disabled pack also drops its blocks, items and every other
file. To change a recipe, remove it and add the replacement, so the pack's conditions and the server's ordering
stay visible.

## Typed filters

Every file entry becomes one `RecipeFilter`: a `RecipeSelector` with all of its present criteria, and exactly
one `RecipeAction`.

| Value | Holds | Example |
| --- | --- | --- |
| `RecipePattern` | a recipe id or type with `*` wildcards | `minecraft:*_planks` |
| `Namespace` | one resource namespace | `gtnh` |
| `RecipeItem` | an item, or a tag written `#namespace:path` | `#c:ores/iron` |
| `RecipeSelector` | the criteria every match must hold | id, type, namespace, input, output |
| `RecipeAction` | what a match does | `Remove`, `ReplaceInput`, `ReplaceOutput` |

A reference is validated where it is read: ids, types, items and tags need a `namespace:path`, namespaces
need a valid namespace, and `1steel:steel-ingot` is as valid as `examplemod:steel-ingot`. A `*` may stand for
any run of characters on either side of the colon, but the literal part around it still follows the resource
rules, so `minecraft:*_planks` is accepted and `MyMod:*` is refused. A pattern that is only `*` matches every
id or type, as the file format always allowed. An empty selector matches every recipe.

{{#include ../../examples/src/recipe_filter.rs:filters}}

The engine matches the selector against the recipe's id, its `type`, and the item and tag strings in its JSON:
`input` looks at strings outside the output keys, and `output` at strings under `result`, `results`, `output`
or `outputs`. It applies the action to those same strings; nothing is resolved to a registry entry first, so a
filter reaches a mod's own recipe types. A malformed entry is skipped with a warning naming its file; it never
widens to match every recipe. The `namespace` criterion reads the recipe id's namespace.

## `remove`, not disable

| You want | Declare |
| --- | --- |
| Every recipe of a type gone | a selector with `type`, action `Remove` |
| Every recipe in one namespace gone | a selector with `namespace` (or the id pattern `namespace:*`), action `Remove` |
| One item no longer accepted | a selector with `input`, action `ReplaceInput` |
| One result replaced | a selector with `output`, action `ReplaceOutput` |

A removal that matches nothing is a no-op: the file is still valid, and a pack that is absent or inactive
contributes no filters at all.
