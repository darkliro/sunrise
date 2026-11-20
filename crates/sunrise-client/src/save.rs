//! World persistence in the vanilla 26.3 layout, readable by both Sunrise and
//! the vanilla game:
//! - `level.dat` (`Data`: name, game type, time, spawn, difficulty, version)
//!   plus the saved-data files `data/minecraft/{weather,world_clocks,
//!   world_gen_settings,game_rules}.dat`;
//! - the player in `players/data/<uuid>.dat` (`singleplayer_uuid`);
//! - per dimension `dimensions/minecraft/<name>/region/r.X.Z.mca` (chunks:
//!   paletted sections with light, heightmaps, block entities, scheduled
//!   block/fluid ticks) and `entities/r.X.Z.mca` (entities, `entity_nbt`).
//!
//! Data Sunrise does not simulate (structure references, unknown block
//! entities and entities, player fields such as the ender chest or effects,
//! game rules) is kept from the loaded files and written back unchanged.
//! Worlds saved by older Sunrise builds (`region/`, `DIM-1/`, `Player` in
//! `level.dat`) are migrated on open.
use crate::mod_chunk_tickets::{ModTicket, ModTickets, TicketOwner};
mod backpressure;
use compression_pool::{CompressedWrite, compress_records};
pub mod biome_regions;
pub mod block_entity_encode;
mod block_entity_order;
mod far_regions;
mod horizon_names;
pub(crate) use horizon_names::adopt_in_player as adopt_horizon_player;
#[cfg(test)]
mod horizon_names_tests;
pub mod level_mods;
mod respawn;
mod writes;
use writes::{Outstanding, QueuedWrite, Writer};

use crate::{
    Result,
    beehive::BeehiveExt,
    biome::Biomes,
    chunk::{BIOME_STRATEGY, Chunk, ColumnLayout, Light, Section, block_strategy},
    mobs::{spawner::SpawnerExt, trial_spawner::TrialSpawnerExt},
    streaming::ChunkGenerator,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
};
use sunrise_core::{
    paletted::PalettedContainer,
    position::{BlockPos, ChunkPos},
    ticks::{SavedTick, TickPriority, TickTypeId},
};
use sunrise_mod_runtime::attachments::{ATTACHMENTS_KEY, attachment_compound};
use sunrise_world::worldgen::blending::BlendingData;
use {
    sunrise_core::{
        nbt,
        nbt::{Compound, JavaString, Limits, NamedTag, Tag},
    },
    sunrise_world::{
        DATA_VERSION,
        chunk::{BlockState, block_entity_pos},
        chunk_status::ChunkStatus,
        compression::Compression,
        heightmap::{Heightmap, HeightmapChunk, HeightmapState, HeightmapType},
        region::{ChunkRecord, RegionFile},
        registry::{BlockRegistry, BlockStateId},
        worldgen::terrain::TerrainBlockStates,
    },
};

pub const VERSION_NAME: &str = sunrise_core::VANILLA_VERSION;

mod chunk_disk;
#[cfg(test)]
mod chunk_disk_tests;
#[cfg(test)]
mod compression_bench;
mod compression_pool;
#[cfg(test)]
mod compression_tests;
mod gateway;
#[cfg(test)]
mod loot_tests;
mod record;
use record::{Encoding, Record};

/// Snapshot live block entities and custom names for a save batch without
/// mutating the world. Each map is visited once, retaining its iteration order
/// within each chunk (including repeated chunk coordinates in the input).
/// An empty batch skips both maps; one chunk uses the direct filter path.
pub fn chunks_with_block_entities<'a, N: std::hash::BuildHasher>(
    chunks: impl IntoIterator<Item = &'a Arc<Chunk>>,
    entities: &HashMap<(i32, i32, i32), crate::containers::BlockEntity>,
    names: &HashMap<(i32, i32, i32), Tag, N>,
) -> Vec<Chunk> {
    let chunks: Vec<_> = chunks.into_iter().collect();
    if chunks.is_empty() {
        return Vec::new();
    }
    if chunks.len() == 1 {
        let mut c = (**chunks[0]).clone();
        c.block_entities = entities
            .iter()
            .filter(|(p, _)| p.0 >> 4 == c.x && p.2 >> 4 == c.z)
            .map(|(p, e)| (*p, e.clone()))
            .collect();
        c.block_names = names
            .iter()
            .filter(|(p, _)| p.0 >> 4 == c.x && p.2 >> 4 == c.z)
            .map(|(p, n)| (*p, n.clone()))
            .collect();
        return vec![c];
    }
    // Index only requested chunks, borrowing their entries so unsaved entities
    // are never cloned. Clone one chunk at a time for inventory cache locality.
    let mut grouped = crate::fast_hash::FxHashMap::<_, (Vec<_>, Vec<_>)>::default();
    grouped.reserve(chunks.len());
    for c in &chunks {
        grouped.entry((c.x, c.z)).or_default();
    }
    for (p, e) in entities {
        if let Some((entries, _)) = grouped.get_mut(&(p.0 >> 4, p.2 >> 4)) {
            entries.push((p, e));
        }
    }
    for (p, n) in names {
        if let Some((_, entries)) = grouped.get_mut(&(p.0 >> 4, p.2 >> 4)) {
            entries.push((p, n));
        }
    }
    chunks
        .into_iter()
        .map(|chunk| {
            let mut c = (**chunk).clone();
            let (entities, names) = &grouped[&(c.x, c.z)];
            c.block_entities = entities.iter().map(|(p, e)| (**p, (*e).clone())).collect();
            c.block_names = names.iter().map(|(p, n)| (**p, (*n).clone())).collect();
            c
        })
        .collect()
}

use sunrise_client_core::nbt_value::string;
pub use sunrise_client_core::nbt_value::{float, number, text};
/// Insert or replace a key of a compound tag.
pub(crate) fn put(tag: &mut Tag, key: &str, value: Tag) {
    if let Tag::Compound(map) = tag {
        map.insert(key.into(), value);
    }
}
fn compound_or_empty(tag: Option<Tag>) -> Tag {
    match tag {
        Some(t @ Tag::Compound(_)) => t,
        _ => Tag::Compound(Compound::new()),
    }
}

/// `dimensions/<namespace>/<path>` inside a world folder. An id that is no safe folder name
/// (`test:../minecraft/overworld`, an absolute path) never leaves the world: it gets a folder
/// of its own under `dimensions/%invalid/`, named by its bytes, which no valid id can reach.
pub fn dimension_dir(world: &Path, dimension: &str) -> PathBuf {
    let id = if dimension.contains(':') {
        dimension.to_owned()
    } else {
        format!("minecraft:{dimension}")
    };
    let dimensions = world.join("dimensions");
    match id.split_once(':') {
        Some((namespace, path))
            if sunrise_core::level_id::valid_level_namespace(namespace)
                && sunrise_core::level_id::valid_level_path(path) =>
        {
            dimensions.join(namespace).join(path)
        }
        _ => {
            let hex: String = id.bytes().map(|b| format!("{b:02x}")).collect();
            dimensions.join("%invalid").join(hex)
        }
    }
}
mod session;
pub use session::{
    SessionLock, downgraded_from, incompatible_version, migrate_legacy, needs_file_fixes,
    upgrade_older_world, upgrade_older_world_with,
};

/// `LevelStorageAccess.checkForLowDiskSpace`: under 64 MiB usable.
pub const LOW_DISK_SPACE: u64 = 67_108_864;

/// The `PlayerData::inventory` slots of `EquipmentSlot.BODY` and `SADDLE`.
/// Vanilla saves them in the `equipment` compound with the armor and off hand.
pub const BODY_SLOT: u8 = 151;
pub const SADDLE_SLOT: u8 = 152;

/// The `EquipmentSlot` ordinals a player saves in `equipment`, with the
/// `PlayerData::inventory` slot each is held in: armor 100 (feet) to 103
/// (head), the off hand 150, body 151, saddle 152. The main hand is the
/// selected inventory slot and never in `equipment`.
const PLAYER_EQUIPMENT_SLOTS: [(usize, u8); 7] = [
    (sunrise_items::equipment::FEET, 100),
    (sunrise_items::equipment::LEGS, 101),
    (sunrise_items::equipment::CHEST, 102),
    (sunrise_items::equipment::HEAD, 103),
    (sunrise_items::equipment::OFFHAND, 150),
    (sunrise_items::equipment::BODY, BODY_SLOT),
    (sunrise_items::equipment::SADDLE, SADDLE_SLOT),
];

/// The `EquipmentSlot` ordinal a saved slot number stands for, for the
/// slots a player holder keeps in its `Equipment`.
pub fn equipment_slot(slot: u8) -> Option<usize> {
    match slot {
        BODY_SLOT => Some(sunrise_items::equipment::BODY),
        SADDLE_SLOT => Some(sunrise_items::equipment::SADDLE),
        _ => None,
    }
}
/// The saved slot number of a held `EquipmentSlot` ordinal (the inverse).
pub fn saved_slot(equipment_slot: usize) -> Option<u8> {
    match equipment_slot {
        sunrise_items::equipment::BODY => Some(BODY_SLOT),
        sunrise_items::equipment::SADDLE => Some(SADDLE_SLOT),
        _ => None,
    }
}
/// `LevelStorageAccess.estimateDiskSpace`: the usable bytes of the disk holding `dir`
/// (`FileStore.getUsableSpace`), `u64::MAX` when it cannot be asked.
pub fn estimate_disk_space(dir: &Path) -> u64 {
    fs4::available_space(dir).unwrap_or(u64::MAX)
}
/// `LevelStorageAccess.checkForLowDiskSpace`.
pub fn low_disk_space(dir: &Path) -> bool {
    estimate_disk_space(dir) < LOW_DISK_SPACE
}
mod worldgen;
pub use worldgen::*;
/// A data pack's JSON as the NBT `dimension_is_vanilla_like` reads (what a codec gets from
/// either one is the same data).
pub(crate) fn json_tag(value: &serde_json::Value) -> Tag {
    use serde_json::Value;
    match value {
        Value::Null => Tag::compound(std::iter::empty::<(&str, Tag)>()),
        Value::Bool(b) => Tag::Byte(i8::from(*b)),
        Value::Number(n) => n.as_i64().map_or_else(
            || Tag::Double(n.as_f64().unwrap_or_default().to_bits()),
            Tag::Long,
        ),
        Value::String(s) => Tag::String(s.as_str().into()),
        Value::Array(items) => Tag::List(items.iter().map(json_tag).collect()),
        Value::Object(map) => Tag::compound(map.iter().map(|(k, v)| (k.as_str(), json_tag(v)))),
    }
}

/// Registry lookups shared by chunk encoding and decoding.
pub struct SaveTables {
    /// Block registry IDs (`BlockId`, scheduled tick types) in registry order.
    block_names: Vec<String>,
    block_ids: HashMap<String, u16>,
    /// Default state of each state's block, to write vanilla's short palette form.
    default_state: Vec<bool>,
    block_entity: Vec<bool>,
    heightmaps: crate::assets::Properties,
    air: u32,
    /// The POI type and its max tickets of each state (`PoiTypes`).
    poi: Vec<Option<(&'static str, i32)>>,
}
impl SaveTables {
    pub fn new(assets: &crate::assets::Assets) -> Self {
        let registry = assets.registry;
        let mut block_names: Vec<String> = Vec::new();
        let mut default_state = Vec::with_capacity(registry.len());
        for id in 0..registry.len() {
            let state = registry
                .state(BlockStateId(id as u32))
                .expect("registry state");
            if block_names.last() != Some(&state.name) {
                block_names.push(state.name.clone());
            }
            default_state
                .push(registry.default_state(&state.name) == Some(BlockStateId(id as u32)));
        }
        let block_ids = block_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), i as u16))
            .collect();
        Self {
            block_names,
            block_ids,
            default_state,
            block_entity: Vec::new(),
            heightmaps: assets.properties.clone(),
            air: registry.default_state("minecraft:air").map_or(0, |s| s.0),
            poi: (0..registry.len())
                .map(|id| {
                    registry
                        .state(BlockStateId(id as u32))
                        .and_then(sunrise_world::poi::poi_type)
                })
                .collect(),
        }
    }
    /// Tables with only the heightmaps' state facts, for tests.
    #[cfg(test)]
    pub(crate) fn for_heightmaps(heightmaps: crate::assets::Properties) -> Self {
        Self {
            block_names: Vec::new(),
            block_ids: HashMap::new(),
            default_state: Vec::new(),
            block_entity: Vec::new(),
            heightmaps,
            air: 0,
            poi: Vec::new(),
        }
    }
    /// Which states carry a block entity (`BlockFacts`), to drop entries for
    /// unknown block entities whose block is gone.
    pub fn with_block_entities(mut self, facts: &crate::blocks::BlockFacts) -> Self {
        self.block_entity = facts.states.iter().map(|s| s.block_entity).collect();
        self
    }
}

pub use crate::chunk::{ChunkExtras, PendingContainerLoot, SavedProto};

fn tick_list(ticks: &[SavedTick], name: impl Fn(u32) -> Option<String>) -> Tag {
    Tag::List(
        ticks
            .iter()
            .filter_map(|t| {
                Some(Tag::compound([
                    ("i", string(&name(t.kind.0)?)),
                    ("x", Tag::Int(t.pos.x)),
                    ("y", Tag::Int(t.pos.y)),
                    ("z", Tag::Int(t.pos.z)),
                    ("t", Tag::Int(t.delay)),
                    ("p", Tag::Int(t.priority as i32)),
                ]))
            })
            .collect(),
    )
}
fn read_ticks(
    tag: Option<&Tag>,
    chunk: (i32, i32),
    id: impl Fn(&str) -> Option<u32>,
) -> Vec<SavedTick> {
    let Some(Tag::List(list)) = tag else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|t| {
            let kind = id(&text(t.get("i"))?)?;
            let pos = BlockPos::new(
                number(t.get("x"))? as i32,
                number(t.get("y"))? as i32,
                number(t.get("z"))? as i32,
            );
            // SavedTick.filterTickListForChunk
            if (pos.x >> 4, pos.z >> 4) != chunk {
                return None;
            }
            Some(SavedTick {
                kind: TickTypeId(kind),
                pos,
                delay: number(t.get("t")).unwrap_or(0) as i32,
                priority: TickPriority::by_value(number(t.get("p")).unwrap_or(0) as i32),
            })
        })
        .collect()
}
fn fluid_name(kind: u32) -> Option<String> {
    use sunrise_world::fluid::FluidKind;
    match FluidKind::from_tick_id(kind)? {
        FluidKind::Empty => None,
        fluid => Some(fluid.name().to_owned()),
    }
}
fn fluid_id(name: &str) -> Option<u32> {
    use sunrise_world::fluid::FluidKind;
    FluidKind::from_name(name)
        .filter(|kind| *kind != FluidKind::Empty)
        .map(FluidKind::tick_id)
}

pub use sunrise_items::item_nbt::{
    decode_items, encode_item, encode_items, item_from_nbt, item_to_nbt,
};
mod block_entity_decode;
pub use block_entity_decode::*;

mod villager_poi;
pub use villager_poi::encode_poi;
mod column;
pub use column::*;

pub use crate::chunk::LevelShape;

type RegionCache = Mutex<RegionFiles>;
mod region;
#[cfg(test)]
use region::{MAX_OPEN_REGIONS, cached_region, write_chunk_record};
use region::{RegionFiles, read_region_record, region_path, resolve_poi, write_region_records};

/// `job(i)` for every `i` below `count`, in order, shared between this thread
/// and up to `helpers - 1` scoped threads, each taking the next index as it
/// finishes one. A helper the system refuses to start (`start` false, or a
/// container at its limit of threads: EAGAIN) leaves its share to the
/// others: this thread does all of it at worst, and never panics for it, as
/// `thread::scope`'s own `spawn` would, which stopped the region writer.
#[cfg(test)]
fn share_out<R: Send>(
    count: usize,
    helpers: usize,
    job: impl Fn(usize) -> R + Sync,
    mut start: impl FnMut() -> bool,
) -> Vec<R> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let work = || {
        let mut out = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            if i >= count {
                return out;
            }
            out.push((i, job(i)));
        }
    };
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let threads: Vec<_> = (1..helpers)
            .filter(|_| start())
            .filter_map(|_| std::thread::Builder::new().spawn_scoped(scope, work).ok())
            .collect();
        let mut done = work();
        for t in threads {
            done.extend(t.join().unwrap_or_else(|e| std::panic::resume_unwind(e)));
        }
        done
    });
    done.sort_unstable_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

/// The `__context` the chunk fixes read: the dimension and its generator type
/// (`ChunkMap.getChunkDataFixContextTag`).
struct DatafixContext {
    dimension: String,
    generator: Option<String>,
}
impl DatafixContext {
    /// From a dimension folder made by [`dimension_dir`] and the world's
    /// `world_gen_settings.dat`; None for a folder outside a world's `dimensions/`.
    fn of(root: &Path) -> Option<Self> {
        let mut parts = Vec::new();
        let mut at = root;
        while at.file_name()? != "dimensions" {
            parts.push(at.file_name()?.to_str()?);
            at = at.parent()?;
        }
        let world = at.parent()?;
        let namespace = parts.pop()?;
        if parts.is_empty() {
            return None;
        }
        parts.reverse();
        let dimension = format!("{namespace}:{}", parts.join("/"));
        let generator = sunrise_world::datafix::files::generators(world)
            .ok()
            .and_then(|g| g.into_iter().find(|(d, _)| *d == dimension))
            .map(|(_, g)| g);
        Some(Self {
            dimension,
            generator,
        })
    }
}
/// `SimpleRegionStorage.upgradeChunkTag`, as `ChunkMap.upgradeChunkTag` (chunks),
/// `EntityStorage` and `SectionStorage.PackedChunk.parse` (POI) call it on every
/// record they read: a world 26.3 has opened keeps the chunks nobody loaded since
/// an older version in that version's format, and `upgrade_older_world` only runs
/// for an older level.dat. A record without a DataVersion counts as -1 (POI: 1945).
/// `context` is needed only for an older chunk; without one it stays as saved.
fn upgrade_record<'a>(
    folder: Folder,
    record: &mut NamedTag,
    context: impl FnOnce() -> Option<&'a DatafixContext>,
) -> Result<()> {
    use sunrise_world::datafix::{self, schema::Ty};
    horizon_names::adopt_in_record(folder, &mut record.tag);
    let default = if folder == Folder::Poi { 1945 } else { -1 };
    if number(record.tag.get("DataVersion")).unwrap_or(default) >= i64::from(DATA_VERSION) {
        return Ok(());
    }
    let context = match folder {
        Folder::Region => match context() {
            Some(c) => Some(c),
            None => return Ok(()),
        },
        _ => None,
    };
    let tag = &mut record.tag;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match (folder, context) {
        (Folder::Region, Some(c)) => {
            datafix::update_chunk(tag, &c.dimension, c.generator.as_deref());
        }
        (Folder::Entities, _) => datafix::update_region_tag(Ty::EntityChunk, tag),
        (Folder::Poi, _) => datafix::update_region_tag(Ty::PoiChunk, tag),
        (Folder::Region, None) => {}
    }))
    .map_err(|_| "upgrading an older record failed".into())
}
/// Chunk and entity records waiting for the writer thread, by (entities?,
/// x, z): reads see them before the region files do.
type PendingWrites = Mutex<HashMap<(Folder, i32, i32), (u64, Record)>>;
type WriteBatch = (Folder, Vec<QueuedWrite>);
/// The region folders of a dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Folder {
    Region,
    Entities,
    Poi,
}
impl Folder {
    fn of(name: &str) -> Self {
        match name {
            "entities" => Self::Entities,
            "poi" => Self::Poi,
            _ => Self::Region,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::Entities => "entities",
            Self::Poi => "poi",
        }
    }
}

/// One dimension folder (`dimensions/minecraft/<name>`). Region files are
/// read lazily by worker threads.
pub struct SaveDir {
    pub root: PathBuf,
    /// The loaded catalog for stack encoding; absent for metadata-only readers.
    items: Option<Arc<crate::items::Items>>,
    regions: Arc<RegionCache>,
    entity_regions: Arc<RegionCache>,
    poi_regions: Arc<RegionCache>,
    pending: Arc<PendingWrites>,
    sequence: AtomicU64,
    disk: OnceLock<Arc<chunk_disk::DiskRuntime>>,
    writer: Mutex<Option<Writer>>,
    outstanding: Arc<Outstanding>,
    /// Set by `Persistent` before the first load.
    pub tables: OnceLock<Arc<SaveTables>>,
    professions: std::sync::RwLock<Option<Arc<crate::mobs::villager::ResolvedProfessions>>>,
    trusted_fluid_load: bool,
    /// The dimension's skylight and height, from its folder name.
    pub shape: LevelShape,
    /// The region file headers kept for far readers (`far_record`).
    far_regions: far_regions::FarRegions,
    /// Saved chunks decoded for far terrain, shared by its sampling threads
    /// (`lod_surfaces_shared`), oldest first.
    lod_decoded: Mutex<LodDecoded>,
    /// What older chunks are upgraded with, found on the first one read.
    datafix_context: OnceLock<Option<DatafixContext>>,
    /// The Overworld's old-world blending cache, read when a save is requested
    /// (installed where `SavedOldWorld` is made for this same folder).
    blending_lookup: OnceLock<BlendingLookup>,
}
mod tickets;
use tickets::*;
mod lod;
pub use lod::*;
impl SaveDir {
    /// This dimension's server-owned mod attachments: `data/sunrise/attachments.dat`, Sunrise's
    /// own namespace. A file that doesn't read is moved aside to `attachments.dat.corrupt`, with
    /// a warning, and the level opens without them (as vanilla's `SavedDataStorage` starts a
    /// saved data over when its file is broken), so the next save can't overwrite it.
    pub fn load_attachments(&self) -> sunrise_mod_runtime::attachments::Attachments {
        let mut path = self.attachments_path();
        let mut read = try_read_dat(&path);
        if matches!(read, Err(DatIssue::Missing)) {
            // A dimension Horizon saved: its file, written under Sunrise's name at the next save.
            path = horizon_names::horizon_attachments_path(&self.root);
            read = try_read_dat(&path);
        }
        let data = match read {
            Err(DatIssue::Missing) => return Default::default(),
            Ok(root) => match root.get("data") {
                Some(Tag::Compound(data)) => Ok(data.clone()),
                _ => Err("no data compound".to_owned()),
            },
            Err(DatIssue::Unreadable(why)) => Err(why),
        };
        data.map(sunrise_mod_runtime::attachments::Attachments::from_nbt)
            .unwrap_or_else(|why| {
                let aside = path.with_extension("dat.corrupt");
                log::warn!(
                    "{}: {why}; mod attachments start over, the file is kept as {}",
                    path.display(),
                    aside.display()
                );
                let _ = fs::rename(&path, &aside);
                Default::default()
            })
    }

    /// Save this dimension's server-owned mod attachments; nothing while it has none and never had.
    pub fn save_attachments(
        &self,
        attachments: &sunrise_mod_runtime::attachments::Attachments,
    ) -> Result<()> {
        let path = self.attachments_path();
        let data = attachments.to_nbt();
        // Emptied after a Horizon file was read: an empty file under Sunrise's name keeps it unread.
        if data.is_empty()
            && !path.exists()
            && !horizon_names::horizon_attachments_path(&self.root).exists()
        {
            return Ok(());
        }
        write_dat(
            &path,
            Tag::compound([
                ("data", Tag::Compound(data)),
                ("DataVersion", Tag::Int(DATA_VERSION)),
            ]),
        )
    }

    fn attachments_path(&self) -> PathBuf {
        self.root
            .join("data")
            .join("sunrise")
            .join("attachments.dat")
    }

    /// The saves of a level of `shape` at `root` (its dimension folder).
    pub fn new(root: PathBuf, shape: LevelShape, items: Option<Arc<crate::items::Items>>) -> Self {
        Self {
            root,
            items,
            regions: Default::default(),
            entity_regions: Default::default(),
            poi_regions: Default::default(),
            pending: Default::default(),
            sequence: AtomicU64::new(0),
            disk: OnceLock::new(),
            writer: Mutex::new(None),
            outstanding: Default::default(),
            tables: OnceLock::new(),
            professions: Default::default(),
            trusted_fluid_load: false,
            shape,
            far_regions: Default::default(),
            lod_decoded: Default::default(),
            datafix_context: OnceLock::new(),
            blending_lookup: OnceLock::new(),
        }
    }
    fn items_for_save(&self) -> Result<&Arc<crate::items::Items>> {
        self.items
            .as_ref()
            .ok_or_else(|| "cannot save chunks without a loaded item catalog".into())
    }
    /// Mark this as a server-owned world save, not a remote cache.
    pub fn enable_trusted_fluid_load(&mut self) {
        self.trusted_fluid_load = true;
    }
    /// Lets saves of this folder take an old chunk's calculated blending data
    /// (`lookup` must not calculate). Set once, before the folder saves.
    pub fn set_blending_lookup(&self, lookup: BlendingLookup) {
        let _ = self.blending_lookup.set(lookup);
    }
    /// The calculated blending data of the old chunk at `x, z`, if this folder
    /// has a lookup and its cache holds one.
    pub fn calculated_blending(&self, x: i32, z: i32) -> Option<Arc<BlendingData>> {
        self.blending_lookup.get().and_then(|lookup| lookup(x, z))
    }
    /// 26.3's TicketStorage: persistent tickets live in each dimension's
    /// data/minecraft/chunk_tickets.dat, with ChunkPos.CODEC's [x, z] int array.
    pub fn load_forced_chunks(&self) -> HashSet<(i32, i32)> {
        let Some(data) = read_saved_data(&self.root, "chunk_tickets") else {
            return HashSet::new();
        };
        let Some(Tag::List(tickets)) = data.get("tickets") else {
            return HashSet::new();
        };
        tickets
            .iter()
            .filter(|t| is_forced_ticket(t))
            .filter_map(|t| match t.get("chunk_pos")? {
                Tag::IntArray(pos) if pos.len() == 2 => Some((pos[0], pos[1])),
                _ => None,
            })
            .collect()
    }
    /// The saved `PORTAL` tickets, for `TicketStorage.activateAllDeactivatedTickets`
    /// (`Ticket.CODEC`: a non-negative level, `ticks_left` 0 when missing).
    pub fn load_portal_tickets(&self) -> Vec<crate::ticket_levels::PortalTicket> {
        let Some(Tag::List(tickets)) =
            read_saved_data(&self.root, "chunk_tickets").and_then(|d| d.get("tickets").cloned())
        else {
            return Vec::new();
        };
        tickets
            .iter()
            .filter(|t| is_portal_ticket(t))
            .filter_map(|t| {
                let chunk = match t.get("chunk_pos")? {
                    Tag::IntArray(pos) if pos.len() == 2 => (pos[0], pos[1]),
                    _ => return None,
                };
                let level = match t.get("level")? {
                    &Tag::Int(level) if level >= 0 => level,
                    _ => return None,
                };
                let ticks_left = match t.get("ticks_left") {
                    None => 0,
                    Some(&Tag::Long(n)) => n,
                    Some(&Tag::Int(n)) => n.into(),
                    Some(&Tag::Short(n)) => n.into(),
                    Some(&Tag::Byte(n)) => n.into(),
                    Some(_) => return None,
                };
                Some(crate::ticket_levels::PortalTicket {
                    chunk,
                    level,
                    ticks_left,
                })
            })
            .collect()
    }
    /// `TicketStorage.packTickets`: the forced chunks and the `PORTAL` tickets
    /// the level holds now, in place of the ones saved before.
    pub fn save_chunk_tickets(
        &self,
        forced: &HashSet<(i32, i32)>,
        portal: &[crate::ticket_levels::PortalTicket],
    ) -> Result<()> {
        if forced.is_empty()
            && portal.is_empty()
            && !saved_data_path(&self.root, "chunk_tickets").exists()
        {
            return Ok(());
        }
        let mut data = compound_or_empty(read_saved_data(&self.root, "chunk_tickets"));
        // Preserve persistent tickets Sunrise does not simulate.
        let mut tickets = match data.get("tickets") {
            Some(Tag::List(tickets)) => tickets
                .iter()
                .filter(|t| !is_forced_ticket(t) && !is_portal_ticket(t))
                .cloned()
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        let mut chunks: Vec<_> = forced.iter().copied().collect();
        chunks.sort_unstable();
        tickets.extend(chunks.into_iter().map(|(x, z)| {
            Tag::compound([
                ("type", Tag::String("minecraft:forced".into())),
                ("level", Tag::Int(31)), // ChunkMap.FORCED_TICKET_LEVEL: ENTITY_TICKING.
                ("chunk_pos", Tag::IntArray(vec![x, z])),
            ])
        }));
        tickets.extend(portal.iter().map(|t| {
            let mut ticket = Tag::compound([
                ("type", Tag::String("minecraft:portal".into())),
                ("level", Tag::Int(t.level)),
                ("chunk_pos", Tag::IntArray(vec![t.chunk.0, t.chunk.1])),
            ]);
            // `optionalFieldOf("ticks_left", 0L)` leaves out its default.
            if t.ticks_left != 0 {
                put(&mut ticket, "ticks_left", Tag::Long(t.ticks_left));
            }
            ticket
        }));
        put(&mut data, "tickets", Tag::List(tickets));
        write_saved_data(&self.root, "chunk_tickets", data)
    }
    /// The chunk tickets of mods (`data/sunrise/mod_chunk_tickets.dat`), kept
    /// out of vanilla's `chunk_tickets.dat` because vanilla decodes its ticket
    /// types through a registry. Each is saved under its controller's id;
    /// `slot_of` gives the slot of an id a loaded mod registered. A ticket of
    /// any other id (a mod that is gone, or a file from before controllers) is
    /// dropped with a warning, and one that doesn't read is skipped.
    pub fn load_mod_tickets(&self, slot_of: impl Fn(&str) -> Option<u32>) -> Vec<ModTicket> {
        let Some(root) = read_dat(&self.mod_tickets_path()) else {
            return Vec::new();
        };
        let Some(Tag::List(tickets)) = root.get("data").and_then(|data| data.get("tickets")) else {
            return Vec::new();
        };
        let mut dropped = std::collections::BTreeSet::new();
        let loaded = tickets
            .iter()
            .filter_map(|tag| {
                let Some((controller, ticket)) = mod_ticket_from_nbt(tag) else {
                    log::warn!("Dropping a saved mod chunk ticket that has no readable controller");
                    return None;
                };
                let Some(slot) = slot_of(&controller) else {
                    dropped.insert(controller);
                    return None;
                };
                Some(ModTicket {
                    controller: slot,
                    ..ticket
                })
            })
            .collect();
        for controller in dropped {
            log::warn!(
                "Dropping the saved chunk tickets of ticket controller {controller}: no loaded mod registers it"
            );
        }
        loaded
    }
    /// Save the tickets of mods under their controllers' ids (`id_of` gives the id of a slot);
    /// nothing while there are none and never were.
    pub fn save_mod_tickets(
        &self,
        tickets: &ModTickets,
        id_of: impl Fn(u32) -> Option<&'static str>,
    ) -> Result<()> {
        let path = self.mod_tickets_path();
        let list: Vec<Tag> = tickets
            .to_save()
            .filter_map(|t| Some(mod_ticket_to_nbt(t, id_of(t.controller)?)))
            .collect();
        if list.is_empty() && !path.exists() {
            return Ok(());
        }
        write_dat(
            &path,
            Tag::compound([
                ("data", Tag::compound([("tickets", Tag::List(list))])),
                ("DataVersion", Tag::Int(DATA_VERSION)),
            ]),
        )
    }
    fn mod_tickets_path(&self) -> PathBuf {
        self.root
            .join("data")
            .join("sunrise")
            .join("mod_chunk_tickets.dat")
    }
    fn region_path(&self, folder: &str, rx: i32, rz: i32) -> PathBuf {
        region_path(&self.root, folder, rx, rz)
    }
    /// The chunks of a region file that an older version generated, as
    /// `IOWorker.createOldDataForRegion` finds them (bit `x + z * 32`): saved
    /// before DataVersion 4997, or with `blending_data`.
    pub fn old_chunk_bits(&self, rx: i32, rz: i32) -> [u64; 16] {
        let mut bits = [0u64; 16];
        let Ok(region) = RegionFile::open(self.region_path("region", rx, rz)) else {
            return bits;
        };
        let present: Vec<ChunkPos> = region.chunks().map(|(pos, _)| pos).collect();
        drop(region);
        for pos in present {
            // `IOWorker` reads the record as saved, before any upgrade.
            let Some(root) = self.read_record_raw("region", pos.x, pos.z) else {
                continue;
            };
            if record_is_old(&root) {
                let index = ((pos.x & 31) + (pos.z & 31) * 32) as usize;
                bits[index / 64] |= 1 << (index % 64);
            }
        }
        bits
    }
    /// Whether the saved chunk at `x, z` is old ([`Self::old_chunk_bits`]'s test), from its
    /// record alone: false for a chunk not saved.
    pub fn is_old_chunk(&self, x: i32, z: i32) -> bool {
        self.read_record_raw("region", x, z)
            .is_some_and(|root| record_is_old(&root))
    }
    /// A saved chunk brought to the current version, for the readers of its
    /// fields that `load` does not decode (`blending_data`).
    pub fn raw_chunk(&self, x: i32, z: i32) -> Option<NamedTag> {
        self.read_record("region", x, z)
    }
    /// The references a saved chunk keeps for its structure start `name`
    /// (`StructureStart.getReferences`, saved as `references`): 0 for a chunk
    /// not saved yet, a start it does not have or one without the field.
    pub fn start_references(&self, x: i32, z: i32, name: &str) -> i32 {
        let Some(root) = self.read_record("region", x, z) else {
            return 0;
        };
        match root
            .tag
            .get("structures")
            .and_then(|s| s.get("starts"))
            .and_then(|s| s.get(name))
            .and_then(|s| s.get("references"))
        {
            Some(Tag::Int(n)) => *n,
            _ => 0,
        }
    }
    /// A record as saved: the pending write first, then the region file.
    fn read_record_raw(&self, folder: &str, x: i32, z: i32) -> Option<NamedTag> {
        self.try_read_record_raw(folder, x, z, Record::tag_for_metadata)
            .ok()
            .flatten()
    }
    /// `read_record_raw`, telling a record that isn't there (`Ok(None)`) from a read that failed
    /// (`Err`, already reported): the second may succeed on a later read.
    fn try_read_record_raw(
        &self,
        folder: &str,
        x: i32,
        z: i32,
        read_pending: fn(&Record) -> Option<Arc<NamedTag>>,
    ) -> std::result::Result<Option<NamedTag>, ()> {
        let pending = self
            .pending
            .lock()
            .map_err(|_| ())?
            .get(&(Folder::of(folder), x, z))
            .map(|(_, r)| r.clone());
        if let Some(record) = pending {
            return Ok(read_pending(&record).map(|t| (*t).clone()));
        }
        let cache = match Folder::of(folder) {
            Folder::Region => &self.regions,
            Folder::Entities => &self.entity_regions,
            Folder::Poi => &self.poi_regions,
        };
        read_region_record(&self.root, cache, folder, x, z)
            .inspect(|record| {
                if self.trusted_fluid_load
                    && folder == "region"
                    && let Some(root) = record
                {
                    intern_saved_fluids(root);
                }
            })
            .map_err(|e| {
                crate::chunk_io::report_load_failure(&self.root, x, z, &e);
            })
    }
    /// A record brought to the current version (`upgrade_record`); `Err` when that failed.
    fn try_read_record(
        &self,
        folder: &str,
        x: i32,
        z: i32,
        read_pending: fn(&Record) -> Option<Arc<NamedTag>>,
    ) -> std::result::Result<Option<NamedTag>, ()> {
        let Some(mut record) = self.try_read_record_raw(folder, x, z, read_pending)? else {
            return Ok(None);
        };
        match self.upgrade_record(Folder::of(folder), &mut record) {
            Ok(()) => Ok(Some(record)),
            Err(e) => {
                crate::chunk_io::report_load_failure(&self.root, x, z, &e);
                Err(())
            }
        }
    }
    /// A record brought to the current version (`upgrade_record`).
    fn read_record(&self, folder: &str, x: i32, z: i32) -> Option<NamedTag> {
        self.try_read_record(folder, x, z, Record::tag_for_metadata)
            .ok()
            .flatten()
    }
    fn upgrade_record(&self, folder: Folder, record: &mut NamedTag) -> Result<()> {
        upgrade_record(folder, record, || {
            self.datafix_context
                .get_or_init(|| DatafixContext::of(&self.root))
                .as_ref()
        })
    }
    pub fn load(&self, x: i32, z: i32, registry: &BlockRegistry, biomes: &Biomes) -> Option<Chunk> {
        self.try_load(x, z, registry, biomes).ok().flatten()
    }
    /// `load`, telling a chunk that isn't saved as a full chunk (`Ok(None)`) from a read that
    /// failed (`Err`): a later read may succeed.
    fn try_load(
        &self,
        x: i32,
        z: i32,
        registry: &BlockRegistry,
        biomes: &Biomes,
    ) -> std::result::Result<Option<Chunk>, ()> {
        let Some(root) = self.try_read_record("region", x, z, Record::tag)? else {
            return Ok(None);
        };
        let tables = self.tables.get().map(|t| &**t);
        let Some(mut chunk) = decode_with(&root, registry, biomes, tables, self.shape.column())
        else {
            return Ok(None);
        };
        load_light(&root.tag, &mut chunk, self.shape.sky_light);
        chunk.entities = sunrise_world::chunk::legacy_entities(&root.tag);
        // A failing load listener does not fail the load: the chunk keeps its stored attachments.
        self.load_disk_attachments(&mut chunk, mod_api::ChunkDiskKind::Full)
            .map_err(|_| ())?;
        Ok(Some(chunk))
    }
    /// A proto chunk saved before it became final (`decode_proto`).
    pub fn load_proto(
        &self,
        x: i32,
        z: i32,
        registry: &BlockRegistry,
        biomes: &Biomes,
    ) -> Option<SavedProto> {
        let root = self.read_record("region", x, z)?;
        let tables = self.tables.get().map(|t| &**t);
        let mut proto = decode_proto(&root, registry, biomes, tables, self.shape.column())?;
        self.load_disk_attachments(&mut proto.column, mod_api::ChunkDiskKind::Proto)
            .ok()?;
        Some(proto)
    }
    /// Queue proto chunks for the writer thread; loads see them meanwhile.
    pub fn save_protos(
        &self,
        protos: Vec<SavedProto>,
        registry: &'static BlockRegistry,
        biomes: &Arc<Biomes>,
    ) -> Result<()> {
        biomes.ensure_saveable()?;
        let encoding = Arc::new(Encoding {
            items: self.items_for_save()?.clone(),
            registry,
            biomes: biomes.clone(),
            tables: self.tables.get().cloned(),
            shape: self.shape,
        });
        let records = protos
            .into_iter()
            .map(|p| {
                (
                    (p.column.x, p.column.z),
                    Record::Proto(Arc::new(p), encoding.clone()),
                )
            })
            .collect();
        self.queue(Folder::Region, records, false)
    }
    /// Entities saved for a chunk in `entities/` (an old chunk's own list: [`Self::load`]).
    pub fn load_entities(&self, x: i32, z: i32) -> Vec<Tag> {
        let mut out = Vec::new();
        if let Some(root) = self.read_record("entities", x, z)
            && let Some(Tag::List(list)) = root.tag.get("Entities")
        {
            out.extend(list.iter().cloned());
        }
        out
    }
    /// Write `chunks` into their region files and wait until they are on disk.
    pub fn save(
        &self,
        chunks: &[Arc<Chunk>],
        registry: &BlockRegistry,
        biomes: &Biomes,
    ) -> Result<usize> {
        biomes.ensure_saveable()?;
        let items = self.items_for_save()?;
        if let Some(result) = self.save_disk_sync(chunks, registry, biomes, items) {
            return result;
        }
        let tables = self.tables.get().map(|t| &**t);
        let records = chunks
            .iter()
            .map(|c| {
                let blending = self.calculated_blending(c.x, c.z);
                (
                    (c.x, c.z),
                    Some(encode_saved(
                        c,
                        registry,
                        biomes,
                        tables,
                        self.shape,
                        blending.as_deref(),
                        items,
                    )),
                )
            })
            .collect();
        self.write_records("region", records, true)?;
        self.save_poi(chunks, true)?;
        Ok(chunks.len())
    }
    fn save_poi(&self, chunks: &[Arc<Chunk>], wait: bool) -> Result<()> {
        let Some(tables) = self.tables.get() else {
            return Ok(());
        };
        let professions = self
            .professions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let records = chunks
            .iter()
            .map(|c| {
                (
                    (c.x, c.z),
                    Record::Poi(c.clone(), tables.clone(), professions.clone()),
                )
            })
            .collect();
        self.queue(Folder::Poi, records, wait)
    }
    /// `save` on the writer thread: returns before the files are written;
    /// loads see the chunks meanwhile.
    pub fn save_later(
        &self,
        chunks: &[Arc<Chunk>],
        registry: &'static BlockRegistry,
        biomes: &Arc<Biomes>,
    ) -> Result<usize> {
        let encoding = self.encoding_for_save_later(chunks, registry, biomes)?;
        let records = chunks
            .iter()
            .map(|c| {
                let blending = self.calculated_blending(c.x, c.z);
                (
                    (c.x, c.z),
                    Record::Chunk(c.clone(), encoding.clone(), blending),
                )
            })
            .collect();
        self.queue(Folder::Region, records, false)?;
        self.save_poi(chunks, false)?;
        Ok(chunks.len())
    }
    /// Write the entity chunks (`EntityStorage`): an empty list deletes it.
    pub fn save_entities(&self, chunks: Vec<((i32, i32), Vec<Tag>)>) -> Result<()> {
        self.write_entities(chunks, true)
    }
    /// `save_entities` on the writer thread.
    pub fn save_entities_later(&self, chunks: Vec<((i32, i32), Vec<Tag>)>) -> Result<()> {
        self.write_entities(chunks, false)
    }
    fn write_entities(&self, chunks: Vec<((i32, i32), Vec<Tag>)>, wait: bool) -> Result<()> {
        let records = chunks
            .into_iter()
            .map(|((x, z), entities)| {
                let tag = (!entities.is_empty()).then(|| NamedTag {
                    name: "".into(),
                    tag: Tag::compound([
                        ("DataVersion", Tag::Int(DATA_VERSION)),
                        ("Position", Tag::IntArray(vec![x, z])),
                        ("Entities", Tag::List(entities)),
                    ]),
                });
                ((x, z), tag)
            })
            .collect();
        self.write_records("entities", records, wait)
    }
}

mod level_dat;
pub use level_dat::*;
mod saved_data;
use saved_data::{read_saved_data, saved_data_path, write_saved_data};
include!("save/preset_saved.rs");
pub use sunrise_client_core::uuid::uuid_from_text;
pub use sunrise_core::uuid::uuid_text;
/// A random (version 4) UUID as vanilla's four-int form.
///
/// Every call gets its own: the generator is seeded once from the clock and
/// the process, then each call takes the next step of one shared counter. A
/// seed from the clock alone repeats within its resolution (a microsecond on
/// macOS): a pack of mobs generated together all had the same UUID, and a
/// vanilla client keeps only one entity per UUID (the rest never moved).
pub fn random_uuid() -> [i32; 4] {
    use std::sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    };
    static NEXT: OnceLock<AtomicU64> = OnceLock::new();
    let next = NEXT.get_or_init(|| {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        AtomicU64::new(t ^ (std::process::id() as u64).rotate_left(32))
    });
    // SplitMix64: distinct steps give distinct outputs.
    let split = || {
        let mut z = next
            .fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
            .wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let (a, b) = (split(), split());
    let a = (a & !0xF000) | 0x4000;
    let b = (b & !(0xC000_0000_0000_0000)) | 0x8000_0000_0000_0000;
    [(a >> 32) as i32, a as i32, (b >> 32) as i32, b as i32]
}
/// `Mth.createInsecureUUID`: a new entity's UUID is its own random's first two
/// draws (`Entity.uuid`).
pub fn insecure_uuid(random: &mut impl sunrise_core::random::RandomSource) -> [i32; 4] {
    let most = random.next_long() & -61441 | 16384;
    let least = random.next_long() & 4_611_686_018_427_387_903 | i64::MIN;
    [
        (most >> 32) as i32,
        most as i32,
        (least >> 32) as i32,
        least as i32,
    ]
}
pub(crate) fn uuid_tag(u: [i32; 4]) -> Tag {
    Tag::IntArray(u.to_vec())
}
pub(crate) fn uuid_from_tag(tag: Option<&Tag>) -> Option<[i32; 4]> {
    match tag? {
        Tag::IntArray(v) if v.len() == 4 => Some([v[0], v[1], v[2], v[3]]),
        Tag::String(s) => uuid_from_text(&s.to_string_lossy()),
        _ => None,
    }
}
// `DoubleTag.valueOf` / `FloatTag.valueOf` store -0.0 as the shared +0.0
// `ZERO`, so a resting entity's `Motion` reads `0.0d`, never `-0.0d`.
pub(crate) fn doubles(values: &[f64]) -> Tag {
    Tag::List(
        values
            .iter()
            .map(|&v| Tag::Double(if v == 0.0 { 0 } else { v.to_bits() }))
            .collect(),
    )
}
pub(crate) fn floats(values: &[f32]) -> Tag {
    Tag::List(
        values
            .iter()
            .map(|&v| Tag::Float(if v == 0.0 { 0 } else { v.to_bits() }))
            .collect(),
    )
}
pub(crate) fn read_doubles<const N: usize>(tag: Option<&Tag>) -> Option<[f64; N]> {
    match tag? {
        Tag::List(l) if l.len() == N => {
            let mut out = [0.0; N];
            for (o, v) in out.iter_mut().zip(l) {
                *o = float(Some(v))?;
            }
            Some(out)
        }
        _ => None,
    }
}

include!("save/preset_generation.rs");

mod level;
pub use level::*;

impl SaveDir {
    /// `lod_surfaces`, decoded once for all far sampling threads: each saved
    /// chunk lies under far tiles of several levels, sampled on any thread.
    fn lod_surfaces_shared(
        &self,
        x: i32,
        z: i32,
        assets: &crate::assets::Assets,
        biomes: &Biomes,
    ) -> Option<LodSurfaces> {
        if let Some((at, columns)) = self.lod_decoded.lock().unwrap().chunks.get(&(x, z))
            && at.elapsed() < LOD_DECODED_TTL
        {
            return Some(columns.clone());
        }
        let columns = self.lod_surfaces(x, z, assets, biomes)?;
        let mut decoded = self.lod_decoded.lock().unwrap();
        let fresh = (std::time::Instant::now(), columns.clone());
        if decoded.chunks.insert((x, z), fresh).is_none() {
            decoded.order.push_back((x, z));
            while decoded.order.len() > LOD_DECODED {
                if let Some(old) = decoded.order.pop_front() {
                    decoded.chunks.remove(&old);
                }
            }
        }
        Some(columns)
    }
    fn lod_surfaces(
        &self,
        x: i32,
        z: i32,
        assets: &crate::assets::Assets,
        biomes: &Biomes,
    ) -> Option<LodSurfaces> {
        let root = self.far_record(x, z)?;
        // Proto saves may still be missing decoration from their neighbors.
        if text(root.tag.get("Status"))
            .is_some_and(|status| status != "minecraft:full" && status != "full")
        {
            return None;
        }
        decode_lod_surfaces(
            &root,
            assets,
            biomes,
            self.shape.column(),
            self.shape.sky_light,
        )
    }
}

#[cfg(test)]
mod tests;

/// Far-terrain floating-cell census on a real save: `SUNRISE_TEST_RESOURCES`
/// (prepared data), `SUNRISE_FAR_SAVE` (a world folder), `SUNRISE_FAR_SEED`
/// and `SUNRISE_FAR_AT` ("x,z", the camera). Samples the tiles each level
/// draws around the camera as the far sampler gives them (saved chunks up to
/// 16 blocks a cell, the seed beyond) and counts components of drawn cells
/// that touch neither the world's bottom nor the tile's halo.
#[cfg(test)]
mod far_census;

#[cfg(test)]
mod jukebox_song_clock_tests;

/// Added controls (H077): an old chunk's calculated blending data is taken
/// when its save is requested and written with `BlendingData.pack`; the raw
/// compound stays for the rest. The heights are the official 26.3 server's
/// for the constructed edge chunk (12, 0).
#[cfg(test)]
mod calculated_blending_save_tests;

#[cfg(test)]
mod biome_tests;
