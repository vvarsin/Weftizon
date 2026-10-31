//! Native mods' say on every hit an entity takes where the world is
//! simulated (docs/native-mods.md, "Events and questions", API 18): Weft's
//! `question::HURT`, which any mod may refuse, and its `chain::DAMAGE`,
//! where each mod may change the amount. The two places every hit goes
//! through ask here: `Mob::damage_with` for mobs and
//! `PlayerMotion::hurt_caused_by` for players. The game binary installs its
//! hook points once (`install`); with none installed (the lib's own tests,
//! tools) every hit is let through unchanged.
use std::sync::OnceLock;

/// A hit as native mods are asked about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit<'a> {
    /// The entity's id; a player's is their entity's id on the server.
    pub entity: u32,
    /// Its type: `minecraft:zombie`, `mymod:ghost`, `minecraft:player`.
    pub entity_type: &'a str,
    /// The damage type: `minecraft:player_attack`, `minecraft:fall`...
    pub damage_type: &'a str,
    /// What it would deal before armour, enchantments and effects.
    pub amount: f32,
    /// A player caused it (their attack, arrow, trident...). Not known yet
    /// for a hit on a player: false.
    pub by_player: bool,
    /// The mob that caused it, when one did. Not known yet for a hit on a
    /// player: none.
    pub attacker: Option<u32>,
}

/// The game binary's answer: `None` when a mod stopped the hit, else the
/// amount it deals.
pub type Hook = fn(&Hit<'_>) -> Option<f32>;

static HOOK: OnceLock<Hook> = OnceLock::new();

/// Installs the game's hook; the first one stays.
pub fn install(hook: Hook) {
    let _ = HOOK.set(hook);
}

/// The amount `hit` deals, or `None` when a mod stopped it. A mod's amount
/// below zero deals none; one that isn't a number is ignored.
pub fn weigh(hit: &Hit<'_>) -> Option<f32> {
    match HOOK.get() {
        Some(hook) => hook(hit).map(|a| if a.is_nan() { hit.amount } else { a.max(0.0) }),
        None => Some(hit.amount),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobs::{Damage, DamageKind, Mob};
    use horizon_core::geometry::Vec3;
    use std::sync::Mutex;

    // The lib's tests share one hook: it acts only on these entity ids,
    // and lets every other hit through as if there were none.
    const REFUSED: u32 = 0xFFFF_FF01;
    const DOUBLED: u32 = 0xFFFF_FF02;
    const NOT_A_NUMBER: u32 = 0xFFFF_FF03;
    const NEGATIVE: u32 = 0xFFFF_FF04;
    const DEAD: u32 = 0xFFFF_FF05;
    static SEEN: Mutex<Vec<(u32, String, String, f32, bool, Option<u32>)>> = Mutex::new(Vec::new());

    fn test_hook(hit: &Hit<'_>) -> Option<f32> {
        if hit.entity >= REFUSED {
            SEEN.lock().unwrap().push((
                hit.entity,
                hit.entity_type.to_owned(),
                hit.damage_type.to_owned(),
                hit.amount,
                hit.by_player,
                hit.attacker,
            ));
        }
        match hit.entity {
            REFUSED => None,
            DOUBLED => Some(hit.amount * 2.0),
            NOT_A_NUMBER => Some(f32::NAN),
            NEGATIVE => Some(-5.0),
            _ => Some(hit.amount),
        }
    }

    /// The hits asked about `entity` of type `kind` (the tests run at once).
    fn seen(entity: u32, kind: &str) -> Vec<(u32, String, String, f32, bool, Option<u32>)> {
        let seen = SEEN.lock().unwrap();
        seen.iter()
            .filter(|s| s.0 == entity && s.1 == kind)
            .cloned()
            .collect()
    }

    #[test]
    fn other_hits_pass_unchanged() {
        install(test_hook);
        let hit = Hit {
            entity: 7,
            entity_type: "minecraft:zombie",
            damage_type: "minecraft:fall",
            amount: 3.5,
            by_player: false,
            attacker: None,
        };
        assert_eq!(weigh(&hit), Some(3.5));
    }

    #[test]
    fn a_mobs_hits_are_asked_before_they_land() {
        install(test_hook);
        let cow = |id| Mob::new(id, crate::mobs::sensor_scan_tests::kind(), Vec3::ZERO, 0.0);
        let mut refused = cow(REFUSED);
        assert!(!refused.damage(4.0, Damage::player([1.0, 0.0, 0.0])));
        assert_eq!(
            (refused.health, refused.hurt_time),
            (10.0, 0),
            "none of it happens"
        );
        assert_eq!(
            seen(REFUSED, "minecraft:cow"),
            [(
                REFUSED,
                "minecraft:cow".to_owned(),
                "minecraft:player_attack".to_owned(),
                4.0,
                true,
                None
            )]
        );
        let mut doubled = cow(DOUBLED);
        assert!(doubled.damage(2.0, Damage::of(DamageKind::Generic)));
        assert_eq!(doubled.health, 6.0);
        // A mod's amount that isn't a number is ignored, one below zero deals none.
        let mut nan = cow(NOT_A_NUMBER);
        assert!(nan.damage(2.0, Damage::of(DamageKind::Generic)));
        assert_eq!(nan.health, 8.0);
        let mut negative = cow(NEGATIVE);
        negative.damage(2.0, Damage::of(DamageKind::Generic));
        assert_eq!(negative.health, 10.0);
        // A dead mob is not asked.
        let mut dead = cow(DEAD);
        dead.health = 0.0;
        assert!(!dead.damage(1.0, Damage::of(DamageKind::Generic)));
        assert!(seen(DEAD, "minecraft:cow").is_empty());
    }

    #[test]
    fn a_players_hits_are_asked_by_their_entity_id() {
        install(test_hook);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client")
            .join(horizon_core::VANILLA_VERSION);
        if !root.join("movement-properties.nbt").exists() {
            return;
        }
        let assets = crate::assets::Assets::load(&root).unwrap();
        let resources = crate::motion::MotionResources::load(&root, &assets).unwrap();
        let (health, rules) = (&resources.health, crate::health::HealthRules::default());
        let id = health.damage.id("minecraft:mob_attack").unwrap();
        let mut player = crate::motion::PlayerMotion::new(&resources, Vec3::ZERO).unwrap();
        player.entity_id = REFUSED;
        assert!(!player.hurt(health, rules, id, 3.0).unwrap());
        assert_eq!(player.health.health(), 20.0);
        assert_eq!(
            seen(REFUSED, "minecraft:player")
                .iter()
                .map(|s| (s.2.as_str(), s.3))
                .collect::<Vec<_>>(),
            [("minecraft:mob_attack", 3.0)]
        );
        let mut player = crate::motion::PlayerMotion::new(&resources, Vec3::ZERO).unwrap();
        player.entity_id = DOUBLED;
        assert!(player.hurt(health, rules, id, 3.0).unwrap());
        assert_eq!(player.health.health(), 14.0);
    }
}
