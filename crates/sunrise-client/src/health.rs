//! Stored player health and server damage for the resident scene.
//! World/audio/network consumers are queued explicitly; full death and tick
//! lifecycles (drops, respawn, effects) remain separate work.
use crate::{
    Result,
    items::ItemStack,
    motion::{MotionEvent, PlayerMotion},
};
pub use mod_api::entity::health::{HurtSource, ShieldBlock, Victim};
use std::{fs, io, path::Path};
use sunrise_client_core::who::Who;
use sunrise_entity_data::{
    Hurtable,
    damage::{Damage, DamageKind, NamedDamage},
};
use sunrise_world::{
    attributes::live::LiveAttributes,
    block_landing::LandingDamage,
    combat_rules::ArmorWorld,
    damage_immunity::{self, DamageRule, EntityImmunityWorld, ImmunityTag, ImmunityWorld},
    damage_reduction::Bypass,
    damage_source::{Catalog, DamageSource, DamageTypeId, Scaling},
    food::{Difficulty, FoodData},
    living_damage::{DamageStat, DamageState},
    living_hurt::{HurtState, HurtTag, HurtWorld, PendingHurt},
    player_health::{PlayerAttributeHealth, PlayerFloatIds},
    player_hurt::{CausingEntity, PlayerHurtWorld, ServerPlayerHurtWorld},
    server_player_combat::{CombatEnvironment, ServerPlayerCombat},
    server_player_damage::{PlayerBodyRead, ServerDamageEnvironment},
    synced_floats::{FloatValue, Observer, SyncedFloats},
};
pub(crate) type Attributes = LiveAttributes<Box<dyn sunrise_world::attributes::live::Identities>>;
pub(crate) type State = HurtState<DamageTypeId>;

pub use crate::world::HealthRules;
mod accepted_hit;
mod attack;
mod kill_credit;
use kill_credit::Credit;
pub use kill_credit::{KnockbackCredit, PlayerMemory, PlayerResolver};
mod hit_attempt;
pub use accepted_hit::AcceptedHit;
pub use attack::{damage_type, melee, mob_source, source_damage_type};
include!("health/hit.rs");

include!("health/damage.rs");

include!("health/player.rs");

include!("health/environment.rs");

#[cfg(test)]
mod datapack_tests;
