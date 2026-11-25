//! The entity data the save codecs and the simulators both need, below either of them.
//!
//! Vanilla keeps one `Entity` class that owns both the fields `saveWithoutId` writes
//! and the living state `LivingEntity.hurtServer` changes. Sunrise splits that class
//! between the mob simulation, the save/load codecs and the damage code, so the two
//! sides meet here instead of naming each other:
//!
//! - [`EntityFields`] is the record every entity's save starts from, and
//!   [`HasEntityFields`] is how a simulator lends it to a codec (issue #2210's
//!   `EntityFields`, wave 2.2);
//! - [`Hurtable`] is what a damage site needs of the entity it hurts, without naming
//!   the simulator's body type (wave 2.1).
//!
//! Nothing here holds a `World`, a `Game` or a renderer: the crate is portable.

mod fields;
pub use fields::{EntityFields, HasEntityFields};
mod hurt;
pub use hurt::Hurtable;
