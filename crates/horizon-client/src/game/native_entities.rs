//! Content mods' entities for native mods (docs/native-mods.md, "Entities",
//! API 16): the hook points `native_entity_tick`, `native_entity_hurt`,
//! `native_entity_died` and `native_entity_interact` (in `native_hooks.rs`)
//! are called from here, and the helpers below read and change an entity by
//! its id. Weft's `on_tick`, `on_hurt`, `on_death` and `on_interact` for the
//! entities a mod declares in `data/<id>/horizon/entity/`. They run where the
//! world is simulated (single player's server thread, a LAN host, a dedicated
//! server), never on a client.
use super::Game;
use horizon_client::mobs::{Damage, Mob, Who, custom};
use horizon_core::geometry::Vec3;

/// A mod's entity as a native mod reads it (`native_entity`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeEntity {
    pub(crate) id: u32,
    /// Its type, `mymod:ghost`.
    pub(crate) kind: String,
    pub(crate) position: [f64; 3],
    pub(crate) velocity: [f64; 3],
    /// Where it faces and looks, degrees.
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) health: f32,
    pub(crate) max_health: f32,
    pub(crate) on_ground: bool,
    /// Ticks since it was made (or loaded).
    pub(crate) age: u32,
    /// What it attacks: a player (`Some(None)`, the one it sees) or an
    /// entity (`Some(Some(id))`).
    pub(crate) target: Option<Option<u32>>,
    /// The animation a mod plays on it (`native_play_animation`).
    pub(crate) animation: Option<String>,
}

/// A hit as a native mod hears it (`native_entity_hurt`, `native_entity_died`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeDamage {
    /// The damage type, `minecraft:player_attack`, `minecraft:fall`...
    pub(crate) kind: &'static str,
    /// What it took of the entity's health.
    pub(crate) amount: f32,
    /// A player hit it (their attack, arrow, trident...).
    pub(crate) by_player: bool,
    /// The entity that hit it, when it was a mob.
    pub(crate) attacker: Option<u32>,
}

impl NativeDamage {
    fn of(d: &Damage, amount: f32) -> Self {
        let by_player = d.attacker == Some(Who::Player);
        Self {
            kind: super::creatures::damage_type(d.kind, by_player),
            amount,
            by_player,
            attacker: match d.attacker {
                Some(Who::Mob(id)) => Some(id),
                _ => None,
            },
        }
    }
}

fn view(m: &Mob, kind: &str) -> NativeEntity {
    NativeEntity {
        id: m.id,
        kind: kind.to_owned(),
        position: [m.position.x, m.position.y, m.position.z],
        velocity: [m.velocity.x, m.velocity.y, m.velocity.z],
        yaw: m.yaw,
        pitch: m.head_pitch,
        health: m.health,
        max_health: m.attrs.max_health,
        on_ground: m.on_ground,
        age: m.age,
        target: m.ai.target.map(|t| match t {
            Who::Player => None,
            Who::Mob(id) => Some(id),
        }),
        animation: m.extra.animation.as_ref().map(|a| a.name.clone()),
    }
}

/// Calls into the hook points.
impl Game<'_> {
    /// `native_entity_tick` for each type of a mod's entities with any alive
    /// in this level, its ids in the level's order, types by id.
    pub(super) fn native_entity_ticks(&mut self) {
        for (entity, ids) in custom::tick_batches(&self.mobs) {
            self.native_entity_tick(entity, &ids);
        }
    }
    /// `native_entity_hurt` for the hits mods' entities took since the last
    /// call, in the level's order.
    pub(super) fn native_entity_hurts(&mut self) {
        for (entity, id, d, amount) in custom::take_hurts(&mut self.mobs) {
            self.native_entity_hurt(entity, id, &NativeDamage::of(&d, amount));
        }
    }
    /// `native_entity_died` as a mod's entity dies (`LivingEntity.die`),
    /// before its loot drops.
    pub(super) fn native_entity_dies(&mut self, m: &Mob) {
        if let Some(c) = &m.kind.custom {
            let damage = m
                .last_damage
                .map(|(d, _)| NativeDamage::of(&d, m.last_hurt));
            self.native_entity_died(c.id, m.id, damage.as_ref());
        }
    }
    /// `native_entity_interact` for a player's right click on `self.mobs[index]`:
    /// true when a mod took it.
    pub(super) fn native_entity_interacts(&mut self, index: usize) -> bool {
        let Some(c) = self.mobs.get(index).and_then(|m| m.kind.custom.clone()) else {
            return false;
        };
        let id = self.mobs[index].id;
        let stack = self.held().cloned();
        let mut claimed = false;
        self.native_entity_interact(c.id, id, stack.as_ref(), &mut claimed);
        claimed
    }
}

/// Helpers for native mods' entities, kept as stable as the hook points.
// Native mods use these; nothing in Horizon does.
#[allow(dead_code)]
impl Game<'_> {
    fn native_mob(&self, id: u32) -> Option<&Mob> {
        self.mobs.iter().find(|m| m.id == id && !m.removed)
    }
    fn native_mob_mut(&mut self, id: u32) -> Option<&mut Mob> {
        self.mobs.iter_mut().find(|m| m.id == id && !m.removed)
    }
    /// Entity `id` of this level, when it is a mob (a mod's or vanilla's).
    pub(super) fn native_entity(&self, id: u32) -> Option<NativeEntity> {
        self.native_mob(id).map(|m| view(m, &m.kind.id))
    }
    /// The ids of this level's alive entities of type `entity` (`mymod:ghost`).
    pub(super) fn native_entities_of(&self, entity: &str) -> Vec<u32> {
        self.mobs
            .iter()
            .filter(|m| !m.removed && m.alive() && *m.kind.id == *entity)
            .map(|m| m.id)
            .collect()
    }
    /// Sets its motion (blocks a tick); it is sent to the players at once.
    pub(super) fn native_set_entity_velocity(&mut self, id: u32, velocity: [f64; 3]) -> bool {
        if !velocity.iter().all(|v| v.is_finite()) {
            return false;
        }
        let Some(m) = self.native_mob_mut(id) else {
            return false;
        };
        m.velocity = Vec3::new(velocity[0], velocity[1], velocity[2]);
        m.needs_sync = true;
        true
    }
    /// Moves it to `position` at once, as `/tp` does.
    pub(super) fn native_teleport_entity(&mut self, id: u32, position: [f64; 3]) -> bool {
        if !position.iter().all(|v| v.is_finite()) {
            return false;
        }
        let Some(m) = self.native_mob_mut(id) else {
            return false;
        };
        let at = Vec3::new(position[0], position[1], position[2]);
        m.position = at;
        m.previous = at;
        m.needs_sync = true;
        true
    }
    /// Hurts it by `amount` of generic damage (armour and invulnerable time
    /// count, as for any hit); false when the hit didn't land.
    pub(super) fn native_hurt_entity(&mut self, id: u32, amount: f32) -> bool {
        if !(amount.is_finite() && amount > 0.0) {
            return false;
        }
        self.native_mob_mut(id).is_some_and(|m| {
            m.damage(
                amount,
                Damage::of(horizon_client::mobs::DamageKind::Generic),
            )
        })
    }
    /// Gives back up to `amount` of its health (`LivingEntity.heal`).
    pub(super) fn native_heal_entity(&mut self, id: u32, amount: f32) -> bool {
        if !(amount.is_finite() && amount > 0.0) {
            return false;
        }
        let Some(m) = self.native_mob_mut(id) else {
            return false;
        };
        if !m.alive() {
            return false;
        }
        m.health = (m.health + amount).min(m.attrs.max_health);
        true
    }
    /// Takes it out of the world with no death and no loot (`discard`).
    pub(super) fn native_remove_entity(&mut self, id: u32) -> bool {
        let Some(m) = self.native_mob_mut(id) else {
            return false;
        };
        m.removed = true;
        true
    }
    /// Plays its animation `name` in place of idle and walk: once, or over
    /// and over when `repeat`; `""` goes back to idle and walk (Weft's
    /// `play_animation`). Every player who sees it sees it. False when it is
    /// not a mod's entity.
    pub(super) fn native_play_animation(&mut self, id: u32, name: &str, repeat: bool) -> bool {
        let Some(m) = self.native_mob_mut(id) else {
            return false;
        };
        if m.kind.custom.is_none() {
            return false;
        }
        m.play_animation(name, repeat);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::NativeDamage;
    use horizon_client::mobs::{Damage, DamageKind};

    /// A hit as the hooks hear it: its damage type, who dealt it.
    #[test]
    fn hits_name_their_damage_type_and_attacker() {
        let by_player = NativeDamage::of(&Damage::player([0.0; 3]), 4.0);
        assert_eq!(
            by_player,
            NativeDamage {
                kind: "minecraft:player_attack",
                amount: 4.0,
                by_player: true,
                attacker: None,
            }
        );
        let by_mob = NativeDamage::of(&Damage::mob(12, [0.0; 3]), 2.0);
        assert_eq!(
            (by_mob.kind, by_mob.by_player, by_mob.attacker),
            ("minecraft:mob_attack", false, Some(12))
        );
        let fall = NativeDamage::of(&Damage::of(DamageKind::Fall), 1.0);
        assert_eq!((fall.kind, fall.attacker), ("minecraft:fall", None));
    }
}
