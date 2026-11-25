//! What a damage site needs of the living entity it hurts (`LivingEntity.hurtServer`).
use sunrise_core::{geometry::Vec3, random::LegacyRandom};

/// A living body a hit can be applied to, without naming the simulator's type.
///
/// The damage code owns the rules (immunity, reduction, knockback, effects); the
/// simulator owns the body. This is the surface the rules touch: identity, the
/// random their sounds and draws use, and the stored state a hit changes. A mob, a
/// player in place and a parked guest implement it over their own body.
pub trait Hurtable {
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
}
