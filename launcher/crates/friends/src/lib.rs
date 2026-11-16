//! Play with Friends invite codes: the launcher checks one and the game does the joining.
pub mod code;
pub mod invite;

pub use code::Code;

/// The game's server address for a friend's world: `friend:K7QM-3XPD`
/// (`sunrise-client --server friend:…`, what the launcher's Join a friend starts).
pub const ADDRESS_PREFIX: &str = "friend:";
