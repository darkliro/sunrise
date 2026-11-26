//! `Entity.discard` and the living state a damage rule changes, over the mob body.
use super::{Mob, Species, villager};
use sunrise_core::{geometry::Vec3, random::LegacyRandom};
use sunrise_entity_data::{Hurtable, damage::Damage};

impl Hurtable for Mob {
    fn id(&self) -> i32 {
        self.id as i32
    }

    fn position(&self) -> Vec3 {
        self.position
    }

    fn random(&mut self) -> &mut LegacyRandom {
        &mut self.random
    }

    fn is_removed(&self) -> bool {
        self.removed
    }

    fn remove(&mut self) {
        self.removed = true;
    }

    fn health(&self) -> f32 {
        self.health
    }

    fn set_health(&mut self, health: f32) {
        self.health = health;
    }

    fn absorption(&self) -> f32 {
        self.absorption
    }

    fn set_absorption(&mut self, absorption: f32) {
        self.absorption = absorption;
    }

    fn invulnerable_time(&self) -> i32 {
        self.invulnerable_time
    }

    fn set_invulnerable_time(&mut self, ticks: i32) {
        self.invulnerable_time = ticks;
    }

    fn is_villager(&self) -> bool {
        self.species() == Species::Villager
    }

    fn villager_hurt(&mut self, source: &Damage, by: [i32; 4]) {
        villager::hurt_by(self, source, by);
    }

    fn wake_villager(&mut self, game_time: i64) {
        villager::wake(self, game_time, None, None);
    }

    fn hurt_by_player_ticks(&self) -> i32 {
        self.ai.last_hurt_by_player
    }

    fn set_hurt_by_player_ticks(&mut self, ticks: i32) {
        self.ai.last_hurt_by_player = ticks;
    }

    fn recent_hurt_by(&self) -> Option<super::Who> {
        Mob::recent_hurt_by(self)
    }
}
