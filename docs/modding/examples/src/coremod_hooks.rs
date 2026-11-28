//! A 1.7.10 coremod's event patch, rebuilt as a hook.
use mod_api::{self, Hooks, PlayerLoggedInEvent, ServerHost};

// ANCHOR: hook
#[derive(Default)]
struct Mod;

#[mod_api::sunrise_mod]
impl Hooks for Mod {
    /// The old coremod transformed the join method to run its own code after the
    /// player entered the world. The `player_logged_in` hook is that moment,
    /// with the event's fields instead of a patched local.
    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        let _ = (host, event.player, &event.name);
    }
}
// ANCHOR_END: hook
