//! Shields in a mob's hands. `Mob::shield_blocks` (the lib) takes a hit
//! from in front while `extra.blocking` is up; here an axe hit knocks the
//! shield down (`BlocksAttacks.disable`, `getSecondsToDisableBlocking`),
//! and native mods raise and lower the shield (API 74).
use super::Game;
use horizon_client::mobs::Species;
use horizon_core::random::RandomSource;

/// `minecraft:shield`'s `disabled_sound`.
const SHIELD_BREAK: &str = "item.shield.break";

impl Game<'_> {
    /// `LivingEntity.getSecondsToDisableBlocking` of mob `id`: its weapon's
    /// `disable_blocking_seconds`; a warden's flat 5 seconds.
    pub(super) fn mob_disable_seconds(&self, id: u32) -> f32 {
        let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
            return 0.0;
        };
        if m.species() == Species::Warden {
            return 5.0;
        }
        m.extra
            .main_hand
            .as_deref()
            .and_then(|item| self.items().get(item))
            .map_or(0.0, |i| i.disable_blocking_seconds)
    }

    /// After a melee hit on mob `target`: if its shield took the hit and
    /// the weapon disables shields (`disable` seconds), the shield goes
    /// down for that long with the break sound.
    pub(super) fn mob_shield_after_hit(&mut self, target: u32, disable: f32) {
        let time = self.world.game_time as i64;
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == target) else {
            return;
        };
        if !std::mem::take(&mut m.extra.just_blocked) || disable <= 0.0 {
            return;
        }
        m.extra.blocking = false;
        m.extra.using_item = 0;
        m.extra.shield_down_until = time + (disable * 20.0).round() as i64;
        let at = m.center();
        let pitch = 0.8 + self.world.random.next_float() * 0.4;
        self.audio.play(SHIELD_BREAK, Some(at), 0.8, pitch);
    }

    /// Clears a stale block mark before a hit that may read it.
    pub(super) fn mob_shield_clear(&mut self, target: u32) {
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == target) {
            m.extra.just_blocked = false;
        }
    }

    /// Raises (`raise`) or lowers the shield of mob `id`, from the hand
    /// that holds an item with `blocks_attacks` (the off hand first). False
    /// when the mob isn't there, holds no shield, or an axe keeps the
    /// shield down. API 74.
    #[allow(dead_code)]
    pub(super) fn native_mob_shield(&mut self, id: u32, raise: bool) -> bool {
        let time = self.world.game_time as i64;
        let Some(m) = self
            .mobs
            .iter()
            .find(|m| m.id == id && !m.removed && m.alive())
        else {
            return false;
        };
        let blocks = |item: &Option<std::sync::Arc<str>>| {
            item.as_deref()
                .and_then(|i| self.items().get(i))
                .is_some_and(|i| i.blocks_attacks.is_some())
        };
        let off = blocks(&m.extra.off_hand);
        let main = blocks(&m.extra.main_hand);
        let down = m.extra.shield_down_until > time;
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else {
            return false;
        };
        if !raise {
            if m.extra.blocking {
                m.extra.blocking = false;
                m.extra.using_item = 0;
            }
            return true;
        }
        if !(off || main) || down {
            return false;
        }
        if !m.extra.blocking {
            m.extra.blocking = true;
            m.extra.using_off_hand = off;
            m.extra.using_item = 1;
        }
        true
    }
}
