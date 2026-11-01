//! Content mods' entities for native mods (docs/native-mods.md, "Entities",
//! API 63): the hook points `native_entity_tick`, `native_entity_hurt`,
//! `native_entity_died` and `native_entity_interact` (in `native_hooks.rs`)
//! are called from here, and the helpers below read and change an entity by
//! its id. Weft's `on_tick`, `on_hurt`, `on_death` and `on_interact` for the
//! entities a mod declares in `data/<id>/horizon/entity/`. They run where the
//! world is simulated (single player's server thread, a LAN host, a dedicated
//! server), never on a client.
use super::Game;
use horizon_client::mobs::{Damage, Mob, Who, custom};
use horizon_core::geometry::Vec3;
use horizon_world::nbt::Tag;

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
    fn mod_mob(&self, id: u32) -> Option<&Mob> {
        self.mobs.iter().find(|m| m.id == id && !m.removed)
    }
    fn mod_mob_mut(&mut self, id: u32) -> Option<&mut Mob> {
        self.mobs.iter_mut().find(|m| m.id == id && !m.removed)
    }
    /// Entity `id` of this level, when it is a mob (a mod's or vanilla's).
    pub(super) fn native_entity(&self, id: u32) -> Option<NativeEntity> {
        self.mod_mob(id).map(|m| view(m, &m.kind.id))
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
        let Some(m) = self.mod_mob_mut(id) else {
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
        let Some(m) = self.mod_mob_mut(id) else {
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
        self.mod_mob_mut(id).is_some_and(|m| {
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
        let Some(m) = self.mod_mob_mut(id) else {
            return false;
        };
        if !m.alive() {
            return false;
        }
        m.health = (m.health + amount).min(m.attrs.max_health);
        true
    }
    /// Takes it out of the world with no death and no loot (`discard`).
    pub(super) fn native_discard_entity(&mut self, id: u32) -> bool {
        let Some(m) = self.mod_mob_mut(id) else {
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
        let Some(m) = self.mod_mob_mut(id) else {
            return false;
        };
        if m.kind.custom.is_none() {
            return false;
        }
        m.play_animation(name, repeat);
        true
    }
    /// Spawns a mob of type `kind` (`mymod:ghost`, `minecraft:zombie`;
    /// no namespace is vanilla's) at `position` in this level, made as a
    /// natural spawn makes it (its equipment, variant and difficulty rolls)
    /// but persistent, so it never despawns (Weft's `spawn`). Every player
    /// near it sees it. Its id, or None when `kind` is not a mob type, the
    /// position is not a number, or it is a monster on Peaceful (as
    /// `/summon` refuses it). API 67.
    pub(super) fn native_spawn_entity(&mut self, kind: &str, position: [f64; 3]) -> Option<u32> {
        if !position.iter().all(|v| v.is_finite()) {
            return None;
        }
        let kind = if kind.contains(':') {
            kind.to_owned()
        } else {
            format!("minecraft:{kind}")
        };
        let monster = self.res.mob_types().get(&kind)?.category == "monster";
        if monster && self.ui.difficulty == 0 {
            return None;
        }
        self.spawn_mob(&kind, position, true)
    }
    /// Sends it walking to `target` by itself, around obstacles, at `speed`
    /// times its movement speed (Weft's `navigate`, vanilla's
    /// `PathNavigation.moveTo`); 0 or less stops it. True when a path was
    /// found. The path is the mob's own navigation: a goal that moves it
    /// (melee, wander) may take it over on a later tick, as in vanilla, so
    /// call it again as the target moves. API 67.
    pub(super) fn native_navigate(&mut self, id: u32, target: [f64; 3], speed: f64) -> bool {
        let Some(i) = self.mobs.iter().position(|m| m.id == id && !m.removed) else {
            return false;
        };
        if !(speed.is_finite() && speed > 0.0) {
            self.mobs[i].nav_stop();
            return false;
        }
        if !target.iter().all(|v| v.is_finite()) {
            return false;
        }
        let mut mobs = std::mem::take(&mut self.mobs);
        let found = mobs[i].nav_move_to(&self.mob_world_base(&[]), target, speed);
        self.mobs = mobs;
        found
    }
    /// Steps it onto the block at `target`, straight at its centre, at
    /// `speed` times its movement speed: its move control aims there every
    /// tick (`MoveControl.setWantedPosition`, jumping a block up on the way)
    /// until it stands there, or for 2 seconds. Its path stops. A path
    /// (`native_navigate`, accuracy 1) counts the next block as reached
    /// already; this moves a mob one block. False when the mob isn't there.
    /// API 77.
    #[allow(dead_code)]
    pub(super) fn native_step_to(&mut self, id: u32, target: [f64; 3], speed: f64) -> bool {
        if !(speed.is_finite() && speed > 0.0) || !target.iter().all(|v| v.is_finite()) {
            return false;
        }
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id && !m.removed) else {
            return false;
        };
        m.nav_step_to(target, speed);
        true
    }
    /// Takes every goal named `name` off mob `id` (`GoalSelector.removeGoal`;
    /// the names are the goals' own: `RandomStroll` or `Stroll` is a mob's wandering), so a
    /// mod can steer a vanilla mob its own way: a pillager that marches
    /// instead of strolling. False when the mob isn't there or has no such
    /// goal. API 82.
    #[allow(dead_code)]
    pub(super) fn native_remove_goal(&mut self, id: u32, name: &str) -> bool {
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id && !m.removed) else {
            return false;
        };
        m.ai.goals.remove(name)
    }
    /// Mob `id` backs away from `from` for `ticks` ticks while it faces it
    /// (piglins' `BackUpIfTooClose`: it looks at it and strafes backwards),
    /// so a shield it holds up stays toward its foe. `speed` is the strafe
    /// (vanilla's 0.75). False when the mob isn't there. API 81.
    #[allow(dead_code)]
    pub(super) fn native_back_off(
        &mut self,
        id: u32,
        from: [f64; 3],
        speed: f32,
        ticks: u32,
    ) -> bool {
        if !(speed.is_finite() && speed > 0.0) || !from.iter().all(|v| v.is_finite()) {
            return false;
        }
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id && !m.removed) else {
            return false;
        };
        m.nav_back_off(from, speed, i64::from(ticks));
        true
    }
    /// Whether it still walks a path (`native_navigate`'s or its goals'):
    /// false once it arrived, the path ran out or it got stuck. API 67.
    pub(super) fn native_navigating(&self, id: u32) -> bool {
        self.mod_mob(id).is_some_and(|m| m.ai.nav.is_in_progress())
    }
    /// Whether the path it walks ends at its target (`Path.canReach`). A
    /// target behind a wall gets a path to the nearest block it can reach:
    /// false then, and a mob that should dig through knows to. API 67.
    pub(super) fn native_path_reaches(&self, id: u32) -> bool {
        self.mod_mob(id)
            .and_then(|m| m.ai.nav.path.as_ref())
            .is_some_and(|p| p.reached)
    }
    /// Value `name` (`mymod:mana`) of its per-entity components (Weft's
    /// `entity_component`): its type's declared value until a mod changes
    /// it, saved with it. None when it has no such value.
    pub(super) fn native_entity_component(&self, id: u32, name: &str) -> Option<Tag> {
        self.mod_mob(id)?
            .extra
            .components
            .get(&name.into())
            .cloned()
    }
    /// Sets value `name` of its per-entity components; `None` takes it out.
    /// Any mob may carry them (Weft's `attach` on another's kind); the world
    /// saves them with it. Server-side only: clients don't get them.
    pub(super) fn native_set_entity_component(
        &mut self,
        id: u32,
        name: &str,
        value: Option<Tag>,
    ) -> bool {
        let Some(m) = self.mod_mob_mut(id) else {
            return false;
        };
        match value {
            Some(v) => m.extra.components.insert(name.into(), v),
            None => m.extra.components.remove(&name.into()),
        };
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
