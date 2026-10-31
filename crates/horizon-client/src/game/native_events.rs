//! Weft's built-in events and questions for native mods (docs/native-mods.md,
//! "Events and questions", API 65): `question::PICKUP_ITEM` and
//! `ITEM_PICKED_UP`, `PLAYER_JOINED` (and leaving), `question::ATTACK`,
//! `question::ENTER_DIMENSION` and `question::HURT`, and the value chains
//! `chain::DAMAGE` and `chain::ATTACK_DAMAGE`. The hook points are in
//! `native_hooks.rs`; these call them with the acting player's state swapped
//! in, so `native_player` and the item helpers read that player. They run
//! where the world is simulated, for every player of it. Every hit is asked
//! from inside the lib (`horizon_client::hits`), through `weigh_hit`.
use super::{Dim, Game, lan::LanHost};
use horizon_client::{hits::Hit, items::ItemStack};

/// `native_hurt`, then `native_damage`, for a hit (the lib's
/// `hits::Hook`, installed as the game starts): `None` when a mod stopped
/// it, else what it deals.
pub(super) fn weigh_hit(hit: &Hit<'_>) -> Option<f32> {
    let mut canceled = false;
    Game::native_hurt(hit, &mut canceled);
    if canceled {
        return None;
    }
    let mut amount = hit.amount;
    Game::native_damage(hit, &mut amount);
    Some(amount)
}

impl Game<'_> {
    /// `native_items_picked_up` once player `conn` took `count` of `stack`.
    pub(super) fn native_picked_up(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        stack: &ItemStack,
        count: u32,
    ) {
        self.as_guest(host, conn, |g| g.native_items_picked_up(stack, count));
    }
    /// `native_player_attack` for the acting player's hit on entity `id`:
    /// true when a mod stopped it.
    pub(super) fn native_attack_refused(&mut self, id: u32) -> bool {
        let mut canceled = false;
        self.native_player_attack(id, &mut canceled);
        canceled
    }
    /// The acting player's attack damage on a `target` after the mods'
    /// `native_attack_damage`; an amount that isn't a number is ignored.
    pub(super) fn native_attack_amount(&mut self, target: &str, damage: f32) -> f32 {
        let mut amount = damage;
        self.native_attack_damage(target, &mut amount);
        if amount.is_nan() {
            damage
        } else {
            amount.max(0.0)
        }
    }
    /// `native_enter_dimension` for player `conn` about to go through a
    /// portal to `target`: true when a mod keeps them where they are.
    pub(super) fn native_travel_refused(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        target: Dim,
    ) -> bool {
        self.as_guest(host, conn, |g| {
            let mut canceled = false;
            g.native_enter_dimension(target.id(), &mut canceled);
            canceled
        })
        .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use horizon_client::hits::Hit;

    /// With no native mod every hit deals what it would.
    #[test]
    fn without_mods_every_hit_deals_what_it_would() {
        for amount in [0.0, 0.5, 4.0, 1000.0, f32::INFINITY] {
            let hit = Hit {
                entity: 3,
                entity_type: "minecraft:zombie",
                damage_type: "minecraft:arrow",
                amount,
                by_player: true,
                attacker: None,
            };
            assert_eq!(super::weigh_hit(&hit), Some(amount));
        }
    }
}
