# GTNH port tracker

Every mod in GregTech: New Horizons 2.9's [Modlist](https://wiki.gtnewhorizons.com/wiki/Modlist) measured against Sunrise's mod API: what is already ported, what a `.smod` can do today, and which engine capabilities are missing. This is a read-only audit. No mod code was read, copied or decompiled.

Snapshot: wiki revision **24850** (2026-11-02), Sunrise **`1b70e44bc09`** (2026-11-28), `sunrise-mods` registry **`d1cb067f6`** (132 projects).

## Numbers

- **244 unique mods** from 245 Modlist rows (`GTNHLib` is listed twice).
- **9 of 14 engine gates are READY**, 5 PARTIAL, 0 MISSING.
- Verdicts: **READY_TO_PORT** 53, **ALREADY_PORTED** 2, **SUPERSEDED_BY_SUNRISE_MOD** 5, **BLOCKED_BY_GATE** 66, **BLOCKED_BY_DEPENDENCY** 45, **BLOCKED_BY_API_MISMATCH** 10, **NEEDS_LICENSE_AUDIT** 42, **LICENSE_BLOCKED** 3, **OBSOLETE_BY_SUNRISE** 18.
- Licences: **permissive** 66, **copyleft** 90, **unknown** 85, **arr** 3.

## Verdicts

| Verdict | Meaning |
|---|---|
| `READY_TO_PORT` | Every required gate is READY and the licence is identified. The port is still a rewrite in data or Rust, not a loaded 1.7.10 jar. |
| `ALREADY_PORTED` | A published port exists in the Sunrise registry. Contribute to it instead of starting another. |
| `SUPERSEDED_BY_SUNRISE_MOD` | A different native mod covers the need (JEI for NEI, Jade for WAILA). Port the original only for exact parity. |
| `BLOCKED_BY_GATE` | At least one required gate is PARTIAL or MISSING. |
| `BLOCKED_BY_DEPENDENCY` | A hard internal dependency is itself blocked, unlicensed or obsolete. |
| `BLOCKED_BY_API_MISMATCH` | The parent mod is ported, but this GTNH add-on targets the 1.7.10 API and must be rewritten for the modern version. |
| `NEEDS_LICENSE_AUDIT` | Technically unblocked, but no reliable SPDX licence was found for the mod or a dependency. No code may be reused. |
| `LICENSE_BLOCKED` | Explicitly All Rights Reserved. Only observed behaviour and a clean-room rewrite are possible. |
| `OBSOLETE_BY_SUNRISE` | The mod only works around a 1.7.10 runtime or engine limit that Sunrise replaces. |

`PARTIAL` gates are the real bottlenecks: they mean a route exists but there is no first-class equivalent, so the mod has to bring its own framework.

## Engine gates

| Gate | Condition | Status |
|---|---|---|
| **G01** | `.smod` format, loading, lifecycle, dependencies and versions | **READY** |
| **G02** | Block, item, fluid and tag registries (the OreDictionary and 1.7.10 metadata equivalent) | **READY** |
| **G03** | Ticking block entities, NBT/component persistence and client/server sync | **READY** |
| **G04** | Extensible recipes with overrides and recipe removal | **PARTIAL** |
| **G05** | Energy, fluids and item transfer (cables, pipes, EU/RF/steam capabilities) | **READY** |
| **G06** | Multiblocks and structure validation (the StructureLib and hatch equivalent) | **PARTIAL** |
| **G07** | Custom UI, menus and containers with slots, icons and progress bars | **READY** |
| **G08** | Worldgen hooks (ore veins, biomes, structures, trees, crops) | **READY** |
| **G09** | Custom dimensions, teleportation, portals, sky and gravity | **PARTIAL** |
| **G10** | Entities, mobs, AI, loot, effects, hunger, armour and tools | **READY** |
| **G11** | Client UI/HUD/overlay hooks and recipe/block lookup (NEI, WAILA, minimap, quests) | **PARTIAL** |
| **G12** | Custom rendering (dynamic models, TESR/BER, cables/microblocks, fluids, effects) | **READY** |
| **G13** | Replacing 1.7.10 coremods, Mixins and ASM with declarative hooks | **PARTIAL** |
| **G14** | Custom networking and client/server packet channels | **READY** |

### G01 — `.smod` format, loading, lifecycle, dependencies and versions

**READY.** The `.smod` archive, `sunrise-mod.toml`, semver dependencies, load order and package index are implemented and documented.

Limits: 1.7.10 mods are not loaded as they are: a port produces an `.smod` (data-only content or a native Rust mod).

Evidence at the audited commit:

- [`crates/sunrise-mod-package/src/manifest.rs:110`](../crates/sunrise-mod-package/src/manifest.rs#L110) — `pub struct Manifest {`
- [`crates/sunrise-mod-package/src/manifest.rs:201`](../crates/sunrise-mod-package/src/manifest.rs#L201) — `pub enum Dependency {`
- [`crates/sunrise-mod-package/src/index.rs:33`](../crates/sunrise-mod-package/src/index.rs#L33) — `pub struct Index {`
- [`docs/modding/src/reference/manifest.md:1`](../docs/modding/src/reference/manifest.md#L1) — `# The manifest and the package`
- [`docs/modding/src/concepts/how-mods-work.md:43`](../docs/modding/src/concepts/how-mods-work.md#L43) — `## When mods load`

### G02 — Block, item, fluid and tag registries (the OreDictionary and 1.7.10 metadata equivalent)

**READY.** Block, item, fluid, tag, custom-registry and block-state metadata APIs are exposed. Modern tags replace the OreDictionary and block states replace metadata.

Limits: There is no binary compatibility with 1.7.10 IDs or metadata: the mapping must be rebuilt with tags and data components.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/blocks.rs:363`](../crates/sunrise-mod-vocab/src/parts/blocks.rs#L363) — `pub trait BlocksRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/items.rs:218`](../crates/sunrise-mod-vocab/src/parts/items.rs#L218) — `pub trait ItemsRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/fluids.rs:36`](../crates/sunrise-mod-vocab/src/parts/fluids.rs#L36) — `pub trait FluidsRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/tags.rs:4`](../crates/sunrise-mod-vocab/src/parts/tags.rs#L4) — `pub trait TagsRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/custom_registries.rs:12`](../crates/sunrise-mod-vocab/src/parts/custom_registries.rs#L12) — `pub trait CustomRegistriesRegistrar {`
- [`crates/sunrise-mod-vocab/src/block/block_metadata.rs:73`](../crates/sunrise-mod-vocab/src/block/block_metadata.rs#L73) — `pub trait BlockMetadata {`

### G03 — Ticking block entities, NBT/component persistence and client/server sync

**READY.** Block-entity types, ticker selection, persistent parts/states, network NBT and sync payloads exist.

Limits: Ticking and serialization are rewritten in the native mod; there is no automatic mapping from a 1.7.10 `TileEntity`.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/block_entities.rs:367`](../crates/sunrise-mod-vocab/src/parts/block_entities.rs#L367) — `pub trait BlockEntitiesRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/block_entities.rs:376`](../crates/sunrise-mod-vocab/src/parts/block_entities.rs#L376) — `fn set_block_entity_ticker_selector(`
- [`docs/modding/src/reference/maths-and-network-nbt.md:49`](../docs/modding/src/reference/maths-and-network-nbt.md#L49) — `## Network NBT`
- [`crates/sunrise-mod-vocab/src/parts/payloads.rs:331`](../crates/sunrise-mod-vocab/src/parts/payloads.rs#L331) — `pub trait PayloadsRegistrar {`

### G04 — Extensible recipes with overrides and recipe removal

**PARTIAL.** Custom recipe types and serializers can be registered, but no public API for removing or overriding existing recipes was found.

Limits: NewHorizonsCoreMod, Minetweaker/ModTweaker and GT recipes rely heavily on removal: it needs generated data packs or a native hook, with conflict risk.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/crafting_recipes.rs:248`](../crates/sunrise-mod-vocab/src/parts/crafting_recipes.rs#L248) — `pub trait RecipeSerializersRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/crafting_recipes.rs:259`](../crates/sunrise-mod-vocab/src/parts/crafting_recipes.rs#L259) — `fn register_recipe_type(&mut self, name: &str);`
- [`crates/sunrise-mod-vocab/src/parts/recipe_books.rs:6`](../crates/sunrise-mod-vocab/src/parts/recipe_books.rs#L6) — `pub trait RecipeBooksRegistrar {`

### G05 — Energy, fluids and item transfer (cables, pipes, EU/RF/steam capabilities)

**READY.** Generic item, fluid and energy capabilities, a transactional energy handler, attached storage and side/face access are available.

Limits: GT EU, steam, amperage and loss semantics are not provided: each mod implements them over the generic capabilities.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/item/energy_capability.rs:15`](../crates/sunrise-mod-vocab/src/item/energy_capability.rs#L15) — `pub trait EnergyHandler: Send + Sync {`
- [`crates/sunrise-mod-vocab/src/item/energy_capability.rs:104`](../crates/sunrise-mod-vocab/src/item/energy_capability.rs#L104) — `pub const ENERGY: CapabilityKey<dyn EnergyHandler> = CapabilityKey {`
- [`crates/sunrise-mod-vocab/src/content/attached_storage.rs:300`](../crates/sunrise-mod-vocab/src/content/attached_storage.rs#L300) — `pub struct AttachedEnergy {`
- [`crates/sunrise-mod-vocab/src/parts/fluids.rs:36`](../crates/sunrise-mod-vocab/src/parts/fluids.rs#L36) — `pub trait FluidsRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/items.rs:246`](../crates/sunrise-mod-vocab/src/parts/items.rs#L246) — `fn provide_item_capability(`

### G06 — Multiblocks and structure validation (the StructureLib and hatch equivalent)

**PARTIAL.** Blocks, block entities and worldgen structures can implement a multiblock by hand, but there is no multiblock/StructureLib framework or reusable shape validation.

Limits: GT5u, Railcraft, TecTech and add-ons must bring their own validator; there is no StructureLib/StructureCompat equivalent.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/structures.rs:103`](../crates/sunrise-mod-vocab/src/parts/structures.rs#L103) — `pub trait StructuresRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/block_entities.rs:367`](../crates/sunrise-mod-vocab/src/parts/block_entities.rs#L367) — `pub trait BlockEntitiesRegistrar {`

### G07 — Custom UI, menus and containers with slots, icons and progress bars

**READY.** Menus, screen factories, slots, widgets, GUI canvas and render layers are exposed: a functional equivalent of ModularUI.

Limits: There is no ModularUI adapter: GTNH layouts are rebuilt, not ported from Java classes.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/menus.rs:144`](../crates/sunrise-mod-vocab/src/parts/menus.rs#L144) — `pub trait MenusRegistrar {`
- [`crates/sunrise-mod-vocab/src/client/menu_screen.rs:251`](../crates/sunrise-mod-vocab/src/client/menu_screen.rs#L251) — `pub trait MenuScreenFactory: Send + Sync + 'static {`
- [`crates/sunrise-mod-vocab/src/client/gui_canvas.rs:129`](../crates/sunrise-mod-vocab/src/client/gui_canvas.rs#L129) — `pub trait GuiCanvas {`
- [`crates/sunrise-mod-vocab/src/item/menu_slot.rs:143`](../crates/sunrise-mod-vocab/src/item/menu_slot.rs#L143) — `pub enum MenuSlotKind {`

### G08 — Worldgen hooks (ore veins, biomes, structures, trees, crops)

**READY.** Feature types, placement modifiers, biome modifiers, structures/pools/processors and spawn placements can be registered.

Limits: GT ore veins and 1.7.10 generation are rewritten as features/configs; 1.7.10 `WorldGen` classes are not replayed.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/features.rs:58`](../crates/sunrise-mod-vocab/src/parts/features.rs#L58) — `pub trait FeaturesRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/biome_modifiers.rs:30`](../crates/sunrise-mod-vocab/src/parts/biome_modifiers.rs#L30) — `pub trait BiomeModifiersRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/structures.rs:103`](../crates/sunrise-mod-vocab/src/parts/structures.rs#L103) — `pub trait StructuresRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/spawn_placements.rs:38`](../crates/sunrise-mod-vocab/src/parts/spawn_placements.rs#L38) — `pub trait SpawnPlacementsRegistrar {`

### G09 — Custom dimensions, teleportation, portals, sky and gravity

**PARTIAL.** Dimensions, chunk generators, biome sources, world clocks and sky properties are exposed; vanilla portals have hooks. Custom gravity has no dedicated API.

Limits: Galacticraft/GalaxySpace/Amun-Ra had very custom gravity, sky and portals: some of it needs native hooks or mixins.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/dimensions.rs:79`](../crates/sunrise-mod-vocab/src/parts/dimensions.rs#L79) — `pub trait DimensionsRegistrar {`
- [`crates/sunrise-mod-vocab/src/world/dimension.rs:92`](../crates/sunrise-mod-vocab/src/world/dimension.rs#L92) — `pub struct DimensionSettings {`
- [`crates/sunrise-mod-vocab/src/world/types.rs:216`](../crates/sunrise-mod-vocab/src/world/types.rs#L216) — `pub struct PortalShape {`

### G10 — Entities, mobs, AI, loot, effects, hunger, armour and tools

**READY.** Entity types, attributes, parameterised vanilla AI goals, effects, loot modifiers/types and entity capabilities are available.

Limits: Custom Java AI (InfernalMobs, SpecialMobs, Tinkers) is rewritten; only parameterised vanilla goals are covered natively.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/entities.rs:177`](../crates/sunrise-mod-vocab/src/parts/entities.rs#L177) — `pub trait EntitiesRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/entity_types.rs:149`](../crates/sunrise-mod-vocab/src/parts/entity_types.rs#L149) — `pub trait GeneralTypesRegistrar {`
- [`crates/sunrise-mod-vocab/src/entity/types.rs:515`](../crates/sunrise-mod-vocab/src/entity/types.rs#L515) — `pub enum GoalSpec {`
- [`crates/sunrise-mod-vocab/src/parts/effects.rs:35`](../crates/sunrise-mod-vocab/src/parts/effects.rs#L35) — `pub trait EffectsRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/loot_modifiers.rs:4`](../crates/sunrise-mod-vocab/src/parts/loot_modifiers.rs#L4) — `pub trait GlobalLootModifiersRegistrar {`

### G11 — Client UI/HUD/overlay hooks and recipe/block lookup (NEI, WAILA, minimap, quests)

**PARTIAL.** GUI/HUD layers, debug entries, the recipe book and key mappings exist, but there is no ready-made NEI/WAILA/quest engine: those mods are rebuilt over the hooks.

Limits: BetterQuesting, NEI, WAILA, JourneyMap and VisualProspecting need custom screens/overlays and recipe registries; UI effort is significant.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/client/gui_layers.rs:44`](../crates/sunrise-mod-vocab/src/client/gui_layers.rs#L44) — `pub trait GuiLayerRegistrar {`
- [`crates/sunrise-mod-vocab/src/client/debug_entries.rs:135`](../crates/sunrise-mod-vocab/src/client/debug_entries.rs#L135) — `pub trait DebugEntryStates {`
- [`crates/sunrise-mod-vocab/src/parts/recipe_books.rs:6`](../crates/sunrise-mod-vocab/src/parts/recipe_books.rs#L6) — `pub trait RecipeBooksRegistrar {`
- [`crates/sunrise-mod-vocab/src/parts/key_mappings.rs:8`](../crates/sunrise-mod-vocab/src/parts/key_mappings.rs#L8) — `pub trait KeyMappingsRegistrar {`

### G12 — Custom rendering (dynamic models, TESR/BER, cables/microblocks, fluids, effects)

**READY.** Entity renderers, baked models, layers, fluid rendering, the render pipeline and block-entity extraction (`draw.block_entity`) are exposed.

Limits: TESR Java code is not ported directly; ForgeMultipart microblocks are reimplemented as custom models/geometry.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/client/entity_renderers.rs:146`](../crates/sunrise-mod-vocab/src/client/entity_renderers.rs#L146) — `pub trait EntityRenderer: Send + Sync + 'static {`
- [`crates/sunrise-mod-vocab/src/client/baked_models.rs:107`](../crates/sunrise-mod-vocab/src/client/baked_models.rs#L107) — `pub struct BakedModelView<'a> {`
- [`docs/modding/src/how-to/client/model-drawing.md:477`](../docs/modding/src/how-to/client/model-drawing.md#L477) — ``draw.fluid_box(&stack, bounds, options)`, where `bounds` is`
- [`docs/modding/src/how-to/client/model-drawing.md:510`](../docs/modding/src/how-to/client/model-drawing.md#L510) — `they carry with `draw.block_entity(&entity)`, vanilla's `BlockEntityRenderDispatcher` extract and`

### G13 — Replacing 1.7.10 coremods, Mixins and ASM with declarative hooks

**PARTIAL.** Sunrise provides declarative hooks and native Rust mixins (`[[native.mixin]]`) to patch one function, but no Java 1.7.10 coremod/ASM loader.

Limits: GTNHLib, Hodgepodge, UniMixins, Archaicfix and 1.7.10 patches are not reusable: each patch is rewritten in Rust or replaced by a native capability.

Evidence at the audited commit:

- [`docs/modding/src/concepts/native-route/2-mixins.md:1`](../docs/modding/src/concepts/native-route/2-mixins.md#L1) — `# Mixins: `[[native.mixin]]``
- [`crates/sunrise-mod-package/src/manifest.rs:461`](../crates/sunrise-mod-package/src/manifest.rs#L461) — `pub struct Native {`
- [`docs/modding/src/concepts/hooks-and-the-host.md:1`](../docs/modding/src/concepts/hooks-and-the-host.md#L1) — `# Hooks and the Host`

### G14 — Custom networking and client/server packet channels

**READY.** Typed and raw payloads, client and server handlers, byte channels and network NBT quotas are exposed. G14 complements G03's block-entity sync.

Limits: 1.7.10 packets (`SimpleNetworkWrapper`/`FMLEventChannel`) are re-declared and versioned; there is no binary channel compatibility.

Evidence at the audited commit:

- [`crates/sunrise-mod-vocab/src/parts/payloads.rs:331`](../crates/sunrise-mod-vocab/src/parts/payloads.rs#L331) — `pub trait PayloadsRegistrar {`
- [`crates/sunrise-mod-runtime/src/byte_channel.rs:63`](../crates/sunrise-mod-runtime/src/byte_channel.rs#L63) — `pub struct Permit<T> {`
- [`docs/modding/src/reference/maths-and-network-nbt.md:49`](../docs/modding/src/reference/maths-and-network-nbt.md#L49) — `## Network NBT`

## Already ported or covered

The public [`sunrise-mods`](https://github.com/sunrise-org/sunrise-mods) registry has real ports and native mods that cover a GTNH need. These are not `READY_TO_PORT`.

| GTNH mod | Status | Sunrise project | Version | Note |
|---|---|---|---|---|
| Applied-Energistics-2-Unofficial | `PORTED_EARLY_ACCESS` | [Applied Energistics 2 (Early Access)](https://github.com/Joeypc6/AE2-Sunrise) | 15.4.15 | Early Access port of a modern release, not the GTNH 1.7.10 fork; GTNH add-ons need a rewrite. |
| TinkersConstruct | `PORTED_EARLY_ACCESS` | [Tinkers' Construct (Early Access)](https://github.com/Joeypc6/TinkersConstruct-Sunrise) | 3.12.3 | Early Access port of a modern release, not the GTNH 1.7.10 fork; GTNH add-ons need a rewrite. |
| FloodLights | `SUPERSEDED` | [Simply Light](https://github.com/arismoko/simply_light) | 0.3.8 | A native replacement covers the need; port the original only for exact parity. |
| NotEnoughItems | `SUPERSEDED` | [JEI](https://github.com/Joeypc6/JEI-Sunrise) | 0.5.6 | A native replacement covers the need; port the original only for exact parity. |
| SleepingBags | `SUPERSEDED` | [Comforts](https://github.com/arismoko/sunrise-ports) | 0.5.5 | A native replacement covers the need; port the original only for exact parity. |
| WAILA | `SUPERSEDED` | [Jade](https://github.com/Joeypc6/Jade-Sunrise) | 26.3.9 | A native replacement covers the need; port the original only for exact parity. |
| WAILAHarvestability | `SUPERSEDED` | [Jade](https://github.com/Joeypc6/Jade-Sunrise) | 26.3.9 | A native replacement covers the need; port the original only for exact parity. |
| AdventureBackpack2 | `PARTIAL_EQUIVALENT` | [Sophisticated Backpacks and Storage](https://github.com/arismoko/sunrise-ports) | 0.5.4 | Covers part of the need; the remaining parity is a separate port. |
| BuildCraft | `PARTIAL_EQUIVALENT` | [Classic Pipes](https://github.com/sofia-gros/horizon-mods) | 1.0.0 | Covers part of the need; the remaining parity is a separate port. |
| GuideNH | `PARTIAL_EQUIVALENT` | [GuideME](https://github.com/Joeypc6/GuideME-Sunrise) | 0.1.1 | Covers part of the need; the remaining parity is a separate port. |
| JourneyMap | `PARTIAL_EQUIVALENT` | [Sunrise Minimap](https://github.com/sunrise-org/sunrise-minimap) | — | Covers part of the need; the remaining parity is a separate port. |
| LogisticsPipes | `PARTIAL_EQUIVALENT` | [Classic Pipes](https://github.com/sofia-gros/horizon-mods) | 1.0.0 | Covers part of the need; the remaining parity is a separate port. |
| Minecraft-Backpack-Mod | `PARTIAL_EQUIVALENT` | [Sophisticated Backpacks and Storage](https://github.com/arismoko/sunrise-ports) | 0.5.4 | Covers part of the need; the remaining parity is a separate port. |
| StorageDrawers | `PARTIAL_EQUIVALENT` | [Tom's Simple Storage](https://github.com/arismoko/sunrise-ports) | 0.4.3 | Covers part of the need; the remaining parity is a separate port. |
| WAILAPlugins | `PARTIAL_EQUIVALENT` | [Jade](https://github.com/Joeypc6/Jade-Sunrise) | 26.3.9 | Covers part of the need; the remaining parity is a separate port. |
| WAWLA | `PARTIAL_EQUIVALENT` | [Jade](https://github.com/Joeypc6/Jade-Sunrise) | 26.3.9 | Covers part of the need; the remaining parity is a separate port. |
| AE2FluidCraft-Rework | `PARENT_PORTED_API_MISMATCH` | [Applied Energistics 2 (Early Access)](https://github.com/Joeypc6/AE2-Sunrise) | 15.4.15 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| BetterP2P | `PARENT_PORTED_API_MISMATCH` | [Applied Energistics 2 (Early Access)](https://github.com/Joeypc6/AE2-Sunrise) | 15.4.15 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| IguanaTweaksTConstruct | `PARENT_PORTED_API_MISMATCH` | [Tinkers' Construct (Early Access)](https://github.com/Joeypc6/TinkersConstruct-Sunrise) | 3.12.3 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| NotEnoughEnergistics | `PARENT_PORTED_API_MISMATCH` | [Applied Energistics 2 (Early Access)](https://github.com/Joeypc6/AE2-Sunrise) | 15.4.15 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| Super-TiC | `PARENT_PORTED_API_MISMATCH` | [Tinkers' Construct (Early Access)](https://github.com/Joeypc6/TinkersConstruct-Sunrise) | 3.12.3 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| ThaumicEnergistics | `PARENT_PORTED_API_MISMATCH` | [Applied Energistics 2 (Early Access)](https://github.com/Joeypc6/AE2-Sunrise) | 15.4.15 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| Tinkers-Defense | `PARENT_PORTED_API_MISMATCH` | [Tinkers' Construct (Early Access)](https://github.com/Joeypc6/TinkersConstruct-Sunrise) | 3.12.3 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| Tinkers-Gregworks | `PARENT_PORTED_API_MISMATCH` | [Tinkers' Construct (Early Access)](https://github.com/Joeypc6/TinkersConstruct-Sunrise) | 3.12.3 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| TinkersMechworks | `PARENT_PORTED_API_MISMATCH` | [Tinkers' Construct (Early Access)](https://github.com/Joeypc6/TinkersConstruct-Sunrise) | 3.12.3 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |
| WirelessCraftingTerminal | `PARENT_PORTED_API_MISMATCH` | [Applied Energistics 2 (Early Access)](https://github.com/Joeypc6/AE2-Sunrise) | 15.4.15 | The parent port exists, but this 1.7.10 add-on targets the old API and needs a rewrite. |

## Where to start

These are the mods with no engine blocker in the two earliest ages. An equivalent in the last column means the need is partly covered already; port only for exact parity.

| Age | Mod | Type | Gates | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| Stone Age | AdventureBackpack2 | Content | G01 G02 | LGPL-3.0 | S | equivalent → sophisticated-backpacks-and-storage |
| Stone Age | ArchitectureCraft | Content | G01 G02 G12 | MIT | M | — |
| Stone Age | CarpentersBlocks | Content | G01 G02 G12 | LGPL-2.1 | M | — |
| Stone Age | Chisel | Content | G01 G02 G12 | GPL-2.0 | M | — |
| Stone Age | EnhancedLootBags | Content | G01 G02 | GPL-3.0 | S | — |
| Stone Age | Et Futurum Requiem | Content | G01 G02 G08 G10 G12 | LGPL-3.0 | L | — |
| Stone Age | ironchest | Content | G01 G02 | GPL-3.0 | S | — |
| Stone Age | LittleTiles | Content | G01 G02 G12 | GPL-3.0 | M | — |
| Stone Age | Minecraft-Backpack-Mod | Content | G01 G02 | LGPL-3.0 | S | equivalent → sophisticated-backpacks-and-storage |
| Stone Age | Modern Markings | Content | G01 G02 | GPL-3.0 | S | — |
| Stone Age | StorageDrawers | Content | G01 G02 G03 G07 G12 | LGPL-3.0 | M | equivalent → toms-simple-storage |
| Stone Age | Village Names | Script | G01 G10 | GPL-3.0 | S | — |
| Stone Age | Ztones | Content | G01 G02 G12 | MIT | M | — |
| Steam Age | MalisisDoors | Content | G01 G02 G12 | MIT | M | — |
| Steam Age | OpenBlocks | Content | G01 G02 G03 G07 G10 G12 | MIT | M | — |
| Steam Age | Random-Things | Content | G01 G02 G03 G07 G10 G12 | LGPL-3.0 | L | — |
| Steam Age | SC2 | Content | G01 G02 G03 G05 G07 G10 G12 | MIT | L | — |
| Steam Age | WirelessRedstone-CBE | Content | G01 G02 G14 | MIT | M | — |

## Mods by age

The age is the GTNH tier where the mod becomes materially useful, not a wiki field. Utility mods, libraries and most UI mods are `Cross-Age`. Within an age, actionable verdicts come first.

### Stone Age (32)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| AdventureBackpack2 | Content | G01 G02 | READY_TO_PORT | LGPL-3.0 | S | equivalent → sophisticated-backpacks-and-storage |
| ArchitectureCraft | Content | G01 G02 G12 | READY_TO_PORT | MIT | M | — |
| CarpentersBlocks | Content | G01 G02 G12 | READY_TO_PORT | LGPL-2.1 | M | — |
| Chisel | Content | G01 G02 G12 | READY_TO_PORT | GPL-2.0 | M | — |
| EnhancedLootBags | Content | G01 G02 | READY_TO_PORT | GPL-3.0 | S | — |
| Et Futurum Requiem | Content | G01 G02 G08 G10 G12 | READY_TO_PORT | LGPL-3.0 | L | — |
| ironchest | Content | G01 G02 | READY_TO_PORT | GPL-3.0 | S | — |
| LittleTiles | Content | G01 G02 G12 | READY_TO_PORT | GPL-3.0 | M | — |
| Minecraft-Backpack-Mod | Content | G01 G02 | READY_TO_PORT | LGPL-3.0 | S | equivalent → sophisticated-backpacks-and-storage |
| Modern Markings | Content | G01 G02 | READY_TO_PORT | GPL-3.0 | S | — |
| StorageDrawers | Content | G01 G02 G03 G07 G12 | READY_TO_PORT | LGPL-3.0 | M | equivalent → toms-simple-storage |
| Village Names | Script | G01 G10 | READY_TO_PORT | GPL-3.0 | S | — |
| Ztones | Content | G01 G02 G12 | READY_TO_PORT | MIT | M | — |
| FloodLights | Content | G01 G02 G12 | SUPERSEDED_BY_SUNRISE_MOD | LGPL-3.0 | M | superseded → simply-light |
| SleepingBags | Content | G01 G02 | SUPERSEDED_BY_SUNRISE_MOD | LGPL-3.0 | S | superseded → comforts |
| Battlegear2-for-Backhand | Content / UI | G01 G02 G10 G11 | BLOCKED_BY_GATE | GPL-3.0 | M | — |
| Baubles Expanded | UI | G01 G10 G11 | BLOCKED_BY_GATE | NOASSERTION | M | — |
| CookingForBlockheads | Content | G01 G02 G03 G06 G07 | BLOCKED_BY_GATE | LGPL-3.0 | L | — |
| NaturesCompass | Content | G01 G02 G08 G10 G11 | BLOCKED_BY_GATE | NOASSERTION | L | — |
| Nutrition | Script / UI | G01 G10 G11 | BLOCKED_BY_GATE | LGPL-3.0 | M | — |
| SpiceOfLife - Carrot Edition | Script / UI | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| BiblioCraft: BiblioWoods Forestry Edition | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | UNKNOWN | S | — |
| BetterBuildersWands | Content | G01 G02 G10 | NEEDS_LICENSE_AUDIT | UNKNOWN | M | — |
| BiblioCraft | Content | G01 G02 | NEEDS_LICENSE_AUDIT | UNKNOWN | M | — |
| BiblioCraft: BiblioWoods Biomes O'Plenty Edition | Content | G01 G02 G08 | NEEDS_LICENSE_AUDIT | UNKNOWN | M | — |
| BiblioCraft: BiblioWoods Natura Edition | Content | G01 G02 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| Catwalks-2 | Content | G01 G02 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| ChiselTones | Content | G01 G02 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| Hunger-Overhaul | Script | G01 G10 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| JABBA | Content | G01 G02 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| Natura | Content | G01 G02 G08 | NEEDS_LICENSE_AUDIT | NOASSERTION | M | — |
| harvestcraft | Content | G01 G02 G10 | LICENSE_BLOCKED | ARR | M | — |

### Steam Age (28)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| MalisisDoors | Content | G01 G02 G12 | READY_TO_PORT | MIT | M | — |
| OpenBlocks | Content | G01 G02 G03 G07 G10 G12 | READY_TO_PORT | MIT | M | — |
| Random-Things | Content | G01 G02 G03 G07 G10 G12 | READY_TO_PORT | LGPL-3.0 | L | — |
| SC2 | Content | G01 G02 G03 G05 G07 G10 G12 | READY_TO_PORT | MIT | L | — |
| WirelessRedstone-CBE | Content | G01 G02 G14 | READY_TO_PORT | MIT | M | — |
| TinkersConstruct | Content | G01 G02 G03 G05 G07 G10 G11 G12 | ALREADY_PORTED | LGPL-3.0 | XL | ported 3.12.3 |
| BuildCraftOilTweak | Script | G01 G04 | BLOCKED_BY_GATE | MIT | S | — |
| ForestryMC | Content | G01 G02 G03 G05 G06 G07 G08 G10 G12 | BLOCKED_BY_GATE | LGPL-3.0 | XL | — |
| Railcraft | Content | G01 G02 G03 G05 G06 G07 G08 G10 G12 | BLOCKED_BY_GATE | NOASSERTION | XL | — |
| BeeBetterAtBees-GTNH | UI | G01 G11 | BLOCKED_BY_DEPENDENCY | NOASSERTION | S | — |
| Binnie | Library | G01 G02 G03 G05 G07 G08 G10 G12 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | XL | — |
| CropsNH | Content | G01 G02 G08 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | M | — |
| Fether | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | S | — |
| gendustry | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | UNKNOWN | S | — |
| MagicBees | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | WTFPL | S | — |
| IguanaTweaksTConstruct | Content | G01 G02 G04 | BLOCKED_BY_API_MISMATCH | CC-BY-SA-4.0 | M | 1.7.10 addon → tconstruct |
| Super-TiC | Content | G01 G02 | BLOCKED_BY_API_MISMATCH | LGPL-3.0 | S | 1.7.10 addon → tconstruct |
| Tinkers-Defense | Content | G01 G02 | BLOCKED_BY_API_MISMATCH | GPL-3.0 | S | 1.7.10 addon → tconstruct |
| Tinkers-Gregworks | Content | G01 G02 | BLOCKED_BY_API_MISMATCH | NOASSERTION | S | 1.7.10 addon → tconstruct |
| TinkersMechworks | Content | G01 G02 | BLOCKED_BY_API_MISMATCH | NOASSERTION | S | 1.7.10 addon → tconstruct |
| Biomes O' Plenty | Content | G01 G02 G08 | NEEDS_LICENSE_AUDIT | UNKNOWN | M | — |
| BuildCraft | Content | G01 G02 G03 G05 G07 G12 | NEEDS_LICENSE_AUDIT | NOASSERTION | XL | equivalent → pipes |
| BuildCraftCompat | Script | G01 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| Extra Utilities | Content | G01 G02 G03 G05 G07 G10 G12 | NEEDS_LICENSE_AUDIT | UNKNOWN | L | — |
| IronChestMinecarts | Content | G01 G02 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| IronTankMinecarts | Content | G01 G02 G03 G05 G07 | NEEDS_LICENSE_AUDIT | MIT | L | — |
| Irontanks | Content | G01 G02 G03 G05 G07 | NEEDS_LICENSE_AUDIT | MIT | L | — |
| Translocators | Content | G01 G02 G14 | NEEDS_LICENSE_AUDIT | NOASSERTION | M | — |

### LV (29)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| AlchemyGrate | Content | G01 G02 | READY_TO_PORT | LGPL-3.0 | S | — |
| IFU | Content | G01 G02 G08 | READY_TO_PORT | LGPL-3.0 | M | — |
| Matter Manipulator | Content | G01 G02 | READY_TO_PORT | LGPL-3.0 | S | — |
| BloodMagic | Content | G01 G02 G03 G05 G06 G07 G10 G11 G12 | BLOCKED_BY_GATE | NOASSERTION | XL | — |
| Industrial Craft 2 | Content | G01 G02 G03 G05 G07 G10 G11 G12 | BLOCKED_BY_GATE | UNKNOWN | XL | — |
| twilightforest | Content | G01 G02 G03 G07 G08 G09 G10 G12 | BLOCKED_BY_GATE | LGPL-2.1 | XL | — |
| BloodArsenal | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | CC-BY-4.0 | S | — |
| Compact Kinetic Generators | Content | G01 G02 G03 G05 G07 | BLOCKED_BY_DEPENDENCY | MIT | L | — |
| Electro-Magic-Tools | Content | G01 G02 G10 | BLOCKED_BY_DEPENDENCY | GPL-3.0 | M | — |
| ForbiddenMagic | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | WTFPL | S | — |
| Gadomancy | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | S | — |
| GTNH-TC-Wands | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | WTFPL | S | — |
| HydroEnergy | Content | G01 G02 G03 G05 G07 | BLOCKED_BY_DEPENDENCY | MIT | L | — |
| Nodal-Mechanics | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | UNKNOWN | S | — |
| Nuclear-Control | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | NOASSERTION | S | — |
| RandomBoubles | Content | G01 G02 G10 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | M | — |
| Salis-Arcana | Script / UI | G01 G11 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | S | — |
| Tainted-Magic | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | NOASSERTION | S | — |
| ThaumcraftMobAspects | Script | G01 G10 | BLOCKED_BY_DEPENDENCY | Unlicense | S | — |
| Thaumic Machina | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | S | — |
| Thaumic_Exploration | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | NOASSERTION | S | — |
| ThaumicBases | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | CC0-1.0 | S | — |
| thaumicboots | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | MIT | S | — |
| thaumicinsurgence | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | GPL-3.0 | S | — |
| ThaumicTinkerer | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | NOASSERTION | S | — |
| Automagy-GTNH | Content | G01 G02 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| Botania | Content | G01 G02 G03 G05 G07 G10 G12 | NEEDS_LICENSE_AUDIT | UNKNOWN | XL | — |
| Botanic-horizons | Content | G01 G02 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| Thaumcraft | Content | G01 G02 G03 G05 G07 G08 G10 G11 G12 G13 | LICENSE_BLOCKED | ARR | XL | — |

### MV (25)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| EnderStorage | Content | G01 G02 G03 G05 G07 | READY_TO_PORT | MIT | L | — |
| EnderZoo | Content | G01 G02 G10 | READY_TO_PORT | CC0-1.0 | M | — |
| OpenModularTurrets | Content | G01 G02 G10 | READY_TO_PORT | GPL-3.0 | M | — |
| RemoteIO | Content | G01 G02 G14 | READY_TO_PORT | MIT | M | — |
| Roguelike-Dungeons | Script | G01 G08 | READY_TO_PORT | GPL-3.0 | S | — |
| TooMuchLoot | Script | G01 | READY_TO_PORT | MIT | S | — |
| LogisticsPipes | Content | G01 G02 G03 G05 G07 G11 G12 G14 | BLOCKED_BY_GATE | NOASSERTION | L | equivalent → pipes |
| OpenComputers | Content | G01 G02 G03 G05 G07 G10 G11 G12 G14 | BLOCKED_BY_GATE | MIT | L | — |
| VendingMachine | Content | G01 G02 G03 G05 G07 G11 | BLOCKED_BY_GATE | MIT | L | — |
| Computronics | Content | G01 G02 G03 G05 G07 | BLOCKED_BY_DEPENDENCY | MIT | L | — |
| OCGlasses | Script | G01 G03 G05 G07 | BLOCKED_BY_DEPENDENCY | MIT | M | — |
| OpenPrinter | Content | G01 G02 G03 G05 G07 | BLOCKED_BY_DEPENDENCY | MIT | L | — |
| OpenSecurity | Content | G01 G02 G03 G05 G07 | BLOCKED_BY_DEPENDENCY | MIT | L | — |
| ThaumicHorizons | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | S | — |
| WarpTheory | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | UNKNOWN | S | — |
| WitcheryExtras | Script | G01 | BLOCKED_BY_DEPENDENCY | GPL-3.0 | S | — |
| WitchingGadgets | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | LGPL-3.0 | S | — |
| Infernal-Mobs | Content | G01 G02 G10 | NEEDS_LICENSE_AUDIT | UNKNOWN | M | — |
| LootGames | Content | G01 G02 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| ProjectBlue | Content | G01 G02 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| ProjectRed | Content | G01 G02 G12 G14 | NEEDS_LICENSE_AUDIT | MIT | L | — |
| SpecialMobs | Content | G01 G02 G10 | NEEDS_LICENSE_AUDIT | NOASSERTION | M | — |
| Steve-s-Factory-Manager | Content | G01 G02 G14 | NEEDS_LICENSE_AUDIT | NOASSERTION | M | — |
| StevesAddons | Content | G01 G02 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| Witchery | Content | G01 G02 G03 G07 G10 G12 | LICENSE_BLOCKED | ARR | XL | — |

### HV (9)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| Applied-Energistics-2-Unofficial | Content | G01 G02 G03 G05 G07 G11 G14 | ALREADY_PORTED | LGPL-3.0 | XL | ported 15.4.15 |
| Galacticraft | Content | G01 G02 G03 G05 G07 G08 G09 G10 G11 G12 | BLOCKED_BY_GATE | NOASSERTION | XL | — |
| PersonalSpace | Content | G01 G02 G09 | BLOCKED_BY_GATE | LGPL-3.0 | M | — |
| SGCraft | Content | G01 G02 G03 G05 G07 G09 | BLOCKED_BY_GATE | MIT | L | — |
| AE2FluidCraft-Rework | Content | G01 G02 G03 G05 G07 G14 | BLOCKED_BY_API_MISMATCH | LGPL-3.0 | L | 1.7.10 addon → ae2 |
| BetterP2P | Content | G01 G02 | BLOCKED_BY_API_MISMATCH | GPL-3.0 | S | 1.7.10 addon → ae2 |
| ThaumicEnergistics | Content | G01 G02 G03 G05 G07 G14 | BLOCKED_BY_API_MISMATCH | NOASSERTION | L | 1.7.10 addon → ae2 |
| WirelessCraftingTerminal | Content | G01 G02 G14 | BLOCKED_BY_API_MISMATCH | GPL-3.0 | M | 1.7.10 addon → ae2 |
| EnderIO | Content | G01 G02 G03 G05 G07 G10 G12 | NEEDS_LICENSE_AUDIT | NOASSERTION | XL | — |

### EV (8)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| amunra | Content | G01 G02 G09 | BLOCKED_BY_GATE | MIT | M | — |
| Draconic-Evolution | Content | G01 G02 G03 G05 G07 G09 G10 G12 | BLOCKED_BY_GATE | LGPL-3.0 | L | — |
| Hardcore-Ender-Expansion | Content | G01 G02 G09 | BLOCKED_BY_GATE | UNKNOWN | M | — |
| AFSU | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | MIT | S | — |
| Galaxy-Space-GTNH | Content | G01 G02 G09 | BLOCKED_BY_DEPENDENCY | UNKNOWN | M | — |
| Gravitation-Suite-Neo | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | NOASSERTION | S | — |
| Gravitation-Suite-old | Content | G01 G02 | BLOCKED_BY_DEPENDENCY | UNKNOWN | S | — |
| Advanced Solar Panel For 1.7.10 (Unofficial) | Content | G01 G02 G03 G05 G07 | NEEDS_LICENSE_AUDIT | UNKNOWN | L | — |

### IV (0)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|

### LuV (0)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|

### ZPM (1)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| supersolarpanels | Content | G01 G02 G03 G05 G07 | NEEDS_LICENSE_AUDIT | UNKNOWN | L | — |

### UV (0)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|

### UHV (0)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|

### Endgame (4)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| Avaritia | Content | G01 G02 G07 | READY_TO_PORT | MIT | L | — |
| Avaritiaddons | Content | G01 G02 | READY_TO_PORT | MPL-2.0 | S | — |
| Eternal-Singularity | Content | G01 G02 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| Universal-Singularities | Content | G01 G02 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |

### Cross-Age (108)

| Mod | Type | Gates | Verdict | Licence | Effort | Sunrise |
|---|---|---|---|---|---|---|
| Amazing-Trophies | Content | G01 G02 | READY_TO_PORT | LGPL-3.0 | S | — |
| AngerMod | Script | G01 G10 | READY_TO_PORT | WTFPL | S | — |
| AppleCore | Library | G01 G10 | READY_TO_PORT | LGPL-3.0 | S | — |
| AsieLib | Library | G01 | READY_TO_PORT | MIT | S | — |
| Craft-Presence | Script | G01 | READY_TO_PORT | MIT | S | — |
| Darkener | Script | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| Default-Configs | Script | G01 | READY_TO_PORT | MIT | S | — |
| DefaultServerList | Script | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| EnderCore | Library | G01 | READY_TO_PORT | CC0-1.0 | S | — |
| GTNHExtLib | Library | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| GTNHLib | Library | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| ModularUI | Library | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| ModularUI2 | Library | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| MrTJPCore | Library | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| oauth | Script | G01 | READY_TO_PORT | GPL-3.0 | S | — |
| OpenModsLib | Library | G01 G10 | READY_TO_PORT | MIT | S | — |
| Realistic-World-Gen | Script | G01 G08 | READY_TO_PORT | GPL-3.0 | S | — |
| Realistic-World-Gen-2 | Script | G01 G08 | READY_TO_PORT | GPL-3.0 | S | — |
| Schematica | Utility | G01 | READY_TO_PORT | MIT | S | — |
| ServerUtilities | Utility | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| SimpleSkinBackport | Library | G01 | READY_TO_PORT | MIT | S | — |
| TX-Loader | Script | G01 | READY_TO_PORT | LGPL-3.0 | S | — |
| WanionLib | Library | G01 | READY_TO_PORT | MPL-2.0 | S | — |
| Yamcl | Library | G01 | READY_TO_PORT | GPL-2.0 | S | — |
| NotEnoughItems | UI | G01 G11 | SUPERSEDED_BY_SUNRISE_MOD | LGPL-3.0 | S | superseded → jei |
| WAILA | UI | G01 G11 | SUPERSEDED_BY_SUNRISE_MOD | NOASSERTION | S | superseded → jade |
| WAILAHarvestability | UI | G01 G10 G11 | SUPERSEDED_BY_SUNRISE_MOD | Unlicense | M | superseded → jade |
| AkashicTome | Content | G01 G02 G11 | BLOCKED_BY_GATE | NOASSERTION | M | — |
| Backhand | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| BetterAchievements | UI | G01 G11 | BLOCKED_BY_GATE | NOASSERTION | S | — |
| BetterLoadingScreen | UI | G01 G11 | BLOCKED_BY_GATE | GPL-2.0 | S | — |
| BetterQuesting | Content | G01 G02 G11 | BLOCKED_BY_GATE | MIT | M | — |
| BlockRenderer6343 | UI | G01 G11 G12 | BLOCKED_BY_GATE | LGPL-3.0 | M | — |
| ChromaticTooltips | UI | G01 G11 | BLOCKED_BY_GATE | GPL-3.0 | S | — |
| ChromaticTooltipsCompat | Library | G01 G11 | BLOCKED_BY_GATE | GPL-3.0 | S | — |
| Controlling | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| CosmeticArmorReworked | UI | G01 G10 G11 | BLOCKED_BY_GATE | NOASSERTION | M | — |
| CraftTweaker | Script | G01 G04 | BLOCKED_BY_GATE | UNKNOWN | S | — |
| Custom-Main-Menu | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| DuraDisplay | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| Extreme Sound Muffler | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| FindIt | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| Freecam | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| GregoryTweaksForCrafting | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| GT5-Unofficial | Content | G01 G02 G03 G04 G05 G06 G07 G08 G09 G10 G11 G12 G13 G14 | BLOCKED_BY_GATE | LGPL-3.0 | XL | — |
| GTNH-Credits | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| GTNH-Web-Map | Utility | G01 G11 | BLOCKED_BY_GATE | Apache-2.0 | S | — |
| GuideNH | UI | G01 G11 G12 | BLOCKED_BY_GATE | LGPL-3.0 | M | equivalent → guideme |
| HelpFixer | UI | G01 G11 | BLOCKED_BY_GATE | WTFPL | S | — |
| Hodgepodge | Script | G01 G13 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| HoloInventory | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| InGame-Info-XML | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| InventoryBogoSorter | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| JourneyMap | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-2.1 | S | equivalent → minimap |
| JourneyMap Server | Script | G01 G11 | BLOCKED_BY_GATE | UNKNOWN | S | — |
| LunatriusCore | Library | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| Mobs Info | UI | G01 G10 G11 | BLOCKED_BY_GATE | LGPL-3.0 | M | — |
| Mouse Tweaks | UI | G01 G11 | BLOCKED_BY_GATE | NOASSERTION | S | — |
| Navigator | Library | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| nei-custom-diagram | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-3.0 | S | — |
| NEI-Integration | Library | G01 G11 | BLOCKED_BY_GATE | NOASSERTION | S | — |
| neiaddons | UI | G01 G11 G14 | BLOCKED_BY_GATE | NOASSERTION | M | — |
| NewHorizonsCoreMod | Script | G01 G04 G13 | BLOCKED_BY_GATE | GPL-3.0 | XL | — |
| NoHotbarNeeded | Library | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| OverloadedArmorBar | UI | G01 G10 G11 | BLOCKED_BY_GATE | MIT | M | — |
| Share-Where-I-am | UI | G01 G11 | BLOCKED_BY_GATE | MIT | S | — |
| StructureLib | Library | G01 G06 | BLOCKED_BY_GATE | GPL-3.0 | S | — |
| TiC-Tooltips | UI | G01 G11 | BLOCKED_BY_GATE | Unlicense | S | — |
| ToroHealth | UI | G01 G10 G11 | BLOCKED_BY_GATE | GPL-3.0 | M | — |
| VisualProspecting | UI | G01 G08 G11 | BLOCKED_BY_GATE | MIT | M | — |
| WAILAPlugins | UI | G01 G11 | BLOCKED_BY_GATE | NOASSERTION | S | equivalent → jade |
| WAWLA | UI | G01 G11 | BLOCKED_BY_GATE | LGPL-2.1 | S | equivalent → jade |
| Minetweaker-Gregtech-5-Addon | Script | G01 G04 | BLOCKED_BY_DEPENDENCY | GPL-3.0 | S | — |
| ModTweaker | Script | G01 G04 | BLOCKED_BY_DEPENDENCY | MIT | S | — |
| StructureCompat | Library | G01 G06 | BLOCKED_BY_DEPENDENCY | AGPL-3.0 | S | — |
| TC-4-Tweaks | UI | G01 G11 | BLOCKED_BY_DEPENDENCY | AGPL-3.0 | S | — |
| TCNodeTracker | UI | G01 G11 | BLOCKED_BY_DEPENDENCY | NOASSERTION | S | — |
| Thaumcraft NEI Plugin | UI | G01 G11 | BLOCKED_BY_DEPENDENCY | UNKNOWN | S | — |
| thaumcraft-research-tweaks | UI | G01 G11 | BLOCKED_BY_DEPENDENCY | MIT | S | — |
| NotEnoughEnergistics | UI | G01 G11 | BLOCKED_BY_API_MISMATCH | MIT | S | 1.7.10 addon → ae2 |
| bdlib | Library | G01 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| DefaultWorldGenerator | Script | G01 G03 G05 G07 G08 | NEEDS_LICENSE_AUDIT | NOASSERTION | L | — |
| DummyCore | Script | G01 | NEEDS_LICENSE_AUDIT | CC-BY-SA-4.0 | S | — |
| ForgeMultipart | Content / Library | G01 G02 G12 G14 | NEEDS_LICENSE_AUDIT | NOASSERTION | M | — |
| ForgeRelocation | Script | G01 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| ForgeRelocationFMP | Script | G01 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| JarJar | Library | G01 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| Mantle | Library | G01 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| Opis | Utility | G01 G10 | NEEDS_LICENSE_AUDIT | NOASSERTION | S | — |
| UniLib | Script | G01 | NEEDS_LICENSE_AUDIT | UNKNOWN | S | — |
| Angelica | Utility | G01 G12 G13 | OBSOLETE_BY_SUNRISE | NOASSERTION | M | — |
| Archaicfix | Script | G01 G13 | OBSOLETE_BY_SUNRISE | UNKNOWN | S | — |
| BetterCrashes | Script | G01 | OBSOLETE_BY_SUNRISE | MIT | S | — |
| BlockLimiter | Script | G01 G09 | OBSOLETE_BY_SUNRISE | WTFPL | S | — |
| Bug-Torch | Script | G01 G13 | OBSOLETE_BY_SUNRISE | MIT | S | — |
| Chunk API | Library | G01 | OBSOLETE_BY_SUNRISE | NOASSERTION | S | — |
| CodeChickenCore | Library | G01 G03 G05 G07 G14 | OBSOLETE_BY_SUNRISE | MIT | L | — |
| CoFH Core | Library | G01 | OBSOLETE_BY_SUNRISE | UNKNOWN | S | — |
| CoreTweaks | Script | G01 G13 | OBSOLETE_BY_SUNRISE | MIT | S | — |
| EndlessIDs | Library | G01 | OBSOLETE_BY_SUNRISE | UNKNOWN | S | — |
| FalsepatternLib | Library | G01 G13 | OBSOLETE_BY_SUNRISE | UNKNOWN | S | — |
| Forgelin | Script | G01 G13 | OBSOLETE_BY_SUNRISE | LGPL-3.0 | S | — |
| Healer | Script | G01 G13 | OBSOLETE_BY_SUNRISE | UNKNOWN | S | — |
| lwjgl3ify | Script | G01 G13 | OBSOLETE_BY_SUNRISE | LGPL-3.0 | S | — |
| NetherPortalFix | Script | G01 G13 | OBSOLETE_BY_SUNRISE | NOASSERTION | S | — |
| Player-API | Library | G01 G13 | OBSOLETE_BY_SUNRISE | LGPL-3.0 | S | — |
| Postea | Library | G01 G13 | OBSOLETE_BY_SUNRISE | MIT | S | — |
| UniMixins | Library | G01 G13 | OBSOLETE_BY_SUNRISE | NOASSERTION | S | — |

## Method and limits

- The mod list comes from the wiki's MediaWiki API (`action=parse&prop=wikitext`), not from rendered HTML. The 245 rows become 244 unique mods after merging `GTNHLib`.
- The gate audit reads public Sunrise sources only. It never builds, checks or tests the game, and records `file:line` evidence at the commit above.
- The registry snapshot reads every `project.toml`, `page.toml` and release name in `sunrise-org/sunrise-mods`. Only the public registry is visible: private development ports are not counted, so a mod can be `ABSENT` while a private port exists.
- Licences come from GitHub's SPDX metadata. `NOASSERTION` or a missing licence is `unknown`, never assumed permissive. Three mods are explicitly ARR.
- The age and the effort (`S`, `M`, `L`, `XL`) are editorial estimates. The gates, licences, descriptions and registry versions are factual.
- A `READY_TO_PORT` verdict means no engine capability is missing, not that the port is small. Every GTNH mod is a rewrite: content mods use data and assets, behaviour mods need Rust.
- G14 (custom networking) complements G03 (block-entity sync); the other 13 gates follow the porting conditions used for this audit.

The page is generated from the snapshot above and is not hand-edited.
