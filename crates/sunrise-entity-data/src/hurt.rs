//! What a damage site needs of the living entity it hurts (`LivingEntity.hurtServer`).
use crate::damage::Damage;
use sunrise_client_core::who::Who;
use sunrise_core::{geometry::Vec3, random::LegacyRandom};

/// A living body a hit can be applied to, without naming the simulator's type.
///
/// The damage code owns the rules (immunity, reduction, knockback, effects); the
/// simulator owns the body. This is the surface the rules touch: identity, the
/// random their sounds and draws use, and the stored state a hit changes. A mob, a
/// player in place and a parked guest implement it over their own body.
///
/// The `Any` bound lets the simulator that owns the body recover it where a rule
/// needs a read this contract deliberately keeps out (a mob's attributes and
/// equipment, say), without the damage code naming the simulator's type.
pub trait Hurtable: std::any::Any {
    /// The entity id the level addresses it by.
    fn id(&self) -> i32;
    /// Its position, for the hit's sounds and the knockback direction.
    fn position(&self) -> Vec3;
    /// `Entity.random`: the entity's own `LegacyRandom`, drawn in vanilla's order.
    fn random(&mut self) -> &mut LegacyRandom;
    /// `Entity.isRemoved`: a removed body takes no further hit.
    fn is_removed(&self) -> bool;
    /// `Entity.discard`: drops the body from its level.
    fn remove(&mut self);
    /// `LivingEntity.getHealth`.
    fn health(&self) -> f32;
    /// `LivingEntity.setHealth`.
    fn set_health(&mut self, health: f32);
    /// `LivingEntity.getAbsorptionAmount`.
    fn absorption(&self) -> f32;
    /// `LivingEntity.setAbsorptionAmount`.
    fn set_absorption(&mut self, absorption: f32);
    /// `LivingEntity.invulnerableTime`: the ticks left in which a same-source hit is ignored.
    fn invulnerable_time(&self) -> i32;
    /// `LivingEntity.invulnerableTime = ticks`.
    fn set_invulnerable_time(&mut self, ticks: i32);

    /// Whether this is a villager, the one species whose hurt path records who hurt it.
    fn is_villager(&self) -> bool;
    /// `Villager.setLastHurtByMob`: the remembered damage and the UUID that caused it.
    fn villager_hurt(&mut self, source: &Damage, by: [i32; 4]);
    /// `Villager.wake`: stand a sleeping villager up.
    fn wake_villager(&mut self, game_time: i64);
    /// `LivingEntity.lastHurtByPlayerTime`: the ticks left of the remembered player.
    fn hurt_by_player_ticks(&self) -> i32;
    /// `LivingEntity.lastHurtByPlayerTime = ticks`.
    fn set_hurt_by_player_ticks(&mut self, ticks: i32);
    /// The entity that last hurt this one inside vanilla's inclusive hundred-tick
    /// credit window (`getLastHurtByMob`).
    fn recent_hurt_by(&self) -> Option<Who>;
}
