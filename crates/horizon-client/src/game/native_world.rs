//! The World API for native mods, API 21 (docs/native-mods.md, "Spawning
//! with gear, effects, weather and projectiles"): spawning any entity from
//! its saved data, potion effects and fire on mobs and players, lightning and
//! weather, shooting projectiles, and finding blocks around a place. What a
//! mod such as Invasion Mod needs beside API 20's waves. They run where the
//! world is simulated (single player's server thread, a LAN host, a dedicated
//! server), never on a client, and act on the level in place: call them from
//! `native_level_tick` or another server hook.
use super::Game;
use super::commands::Target;
use horizon_client::{
    brewing,
    effects::EffectInstance,
    mobs::Who,
    projectiles::{Projectile, ProjectileKind},
};
use horizon_core::geometry::Vec3;

/// A projectile a native mod shoots (`native_shoot`), by its entity type.
fn projectile_kind(kind: &str) -> Option<ProjectileKind> {
    Some(match kind.trim_start_matches("minecraft:") {
        "arrow" => ProjectileKind::Arrow {
            base_damage: 2.0,
            critical: false,
            effect: None,
        },
        "trident" => ProjectileKind::Trident,
        "snowball" => ProjectileKind::Snowball,
        "egg" => ProjectileKind::Egg,
        "small_fireball" => ProjectileKind::SmallFireball,
        "fireball" => ProjectileKind::LargeFireball { power: 1 },
        "wither_skull" => ProjectileKind::WitherSkull { dangerous: false },
        "wind_charge" => ProjectileKind::WindCharge,
        "llama_spit" => ProjectileKind::LlamaSpit,
        _ => return None,
    })
}

/// The id a command target carries, when it is an entity of this level.
fn target_id(t: Target) -> Option<u32> {
    match t {
        Target::Mob(id)
        | Target::Item(id)
        | Target::Orb(id)
        | Target::Projectile(id)
        | Target::Stand(id)
        | Target::Frame(id)
        | Target::Painting(id)
        | Target::Boat(id)
        | Target::Cart(id)
        | Target::Other(id)
        | Target::Crystal(id)
        | Target::Dragon(id) => Some(id),
        Target::Player | Target::Guest(_) => None,
    }
}

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Horizon does.
#[allow(dead_code)]
impl Game<'_> {
    /// Spawns an entity of type `kind` at `position` from its saved data
    /// `nbt`, as `/summon <kind> <pos> <nbt>` does: equipment
    /// (`{equipment:{mainhand:{id:"minecraft:trident",count:1}}}`), a charged
    /// creeper (`{powered:1b}`), fire (`{Fire:2000s}`), effects
    /// (`{active_effects:[{id:"minecraft:speed",duration:600}]}`), a name, a
    /// scale attribute, riders. Like `/summon` with data, the mob gets no
    /// natural equipment or variant rolls: what `nbt` says is what it has.
    /// Add `PersistenceRequired:1b` to keep a mob from despawning. Its id, or
    /// None when the data does not parse, the type cannot be summoned, or it
    /// made an entity with no id of its own (primed TNT, falling blocks).
    /// API 21.
    pub(super) fn native_summon(
        &mut self,
        kind: &str,
        position: [f64; 3],
        nbt: &str,
    ) -> Option<u32> {
        if !position.iter().all(|v| v.is_finite()) {
            return None;
        }
        let kind = if kind.contains(':') {
            kind.to_owned()
        } else {
            format!("minecraft:{kind}")
        };
        if matches!(
            kind.as_str(),
            "minecraft:player" | "minecraft:fishing_bobber"
        ) {
            return None;
        }
        let nbt = if nbt.trim().is_empty() { "{}" } else { nbt };
        self.summon_with_nbt(&kind, position, nbt)
            .ok()
            .flatten()
            .and_then(target_id)
    }
    /// Gives mob `id` the effect `effect` (`minecraft:glowing`, `speed`...)
    /// for `ticks` (-1 for ever) at `amplifier` (0 is level I), as `/effect
    /// give` does: a stronger or longer one already there stays. False when
    /// it is not a mob of this level, the effect is unknown, or the mob is
    /// immune to it (a wither to wither, a spider to poison). API 21.
    pub(super) fn native_mob_effect(
        &mut self,
        id: u32,
        effect: &str,
        ticks: i32,
        amplifier: i32,
    ) -> bool {
        let Some(name) = super::potions::mob_effect_id(effect) else {
            return false;
        };
        let short = name.trim_start_matches("minecraft:");
        let tags = &self.res.stations().enchantments.entity_tags;
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && !m.removed && m.alive())
        else {
            return false;
        };
        if !super::potions::mob_can_be_affected(tags, &m.kind.id, short) {
            return false;
        }
        m.add_effect(name, ticks.max(-1), amplifier.clamp(0, 255));
        m.needs_sync = true;
        true
    }
    /// Gives the player `uuid` the effect `effect` (`minecraft:darkness`,
    /// `weakness`, `wither`...) for `ticks` (-1 for ever) at `amplifier`, as
    /// `/effect give` does. False when they are not on, or the effect is
    /// unknown. API 21.
    pub(super) fn native_player_effect(
        &mut self,
        uuid: u128,
        effect: &str,
        ticks: i32,
        amplifier: i32,
    ) -> bool {
        let Some(e) = brewing::effect(effect.trim_start_matches("minecraft:")) else {
            return false;
        };
        let instance = EffectInstance::new(e.id, ticks.max(-1), amplifier);
        let Some(mut host) = self.lan.take() else {
            return false;
        };
        let conn = host
            .hub
            .guests
            .iter()
            .find(|g| g.uuid == uuid && g.placed)
            .map(|g| g.conn);
        let given = conn
            .and_then(|conn| {
                self.as_guest(&mut host, conn, |g| {
                    g.add_player_effect(instance);
                })
            })
            .is_some();
        self.lan = Some(host);
        given
    }
    /// Sets mob `id` on fire for `ticks` (as fire or lava would; a longer
    /// fire already burning stays). Fire-immune mobs only look on fire. Call
    /// it again to keep it burning. False when it is not a mob of this
    /// level. API 21.
    pub(super) fn native_set_entity_fire(&mut self, id: u32, ticks: i32) -> bool {
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && !m.removed && m.alive())
        else {
            return false;
        };
        if m.fire_ticks < ticks {
            m.fire_ticks = ticks;
        }
        m.needs_sync = true;
        true
    }
    /// A lightning bolt at `position`: its flash, thunder, fire and damage,
    /// a creeper struck turns charged, a pig a piglin; `visual_only` keeps
    /// only the flash and the thunder (Weft's and vanilla's
    /// `setVisualOnly`). API 21.
    pub(super) fn native_lightning(&mut self, position: [f64; 3], visual_only: bool) -> bool {
        if !position.iter().all(|v| v.is_finite()) {
            return false;
        }
        self.spawn_bolt(position, visual_only);
        true
    }
    /// Sets the weather of the level in place for `ticks`, as `/weather`
    /// does: `clear`, `rain` or `thunder`. The overworld's weather is the
    /// world's: call it from the overworld's `native_level_tick`. False for
    /// another kind. API 21.
    pub(super) fn native_set_weather(&mut self, kind: &str, ticks: i32) -> bool {
        let ticks = ticks.max(1);
        let (clear, time, raining, thundering) = match kind {
            "clear" => (ticks, 0, false, false),
            "rain" => (0, ticks, true, false),
            "thunder" => (0, ticks, true, true),
            _ => return false,
        };
        let w = &mut self.world.weather;
        w.clear_time = clear;
        w.rain_time = time;
        w.thunder_time = time;
        w.raining = raining;
        w.thundering = thundering;
        true
    }
    /// Shoots a projectile of type `kind` (`arrow`, `trident`, `snowball`,
    /// `egg`, `small_fireball`, `fireball`, `wither_skull`, `wind_charge`,
    /// `llama_spit`) from `from` along `direction` at `speed` blocks a tick
    /// (a bow's full draw is 3), as a mob shoots it. `owner` is the mob it
    /// counts as shot by (its kills and its own hits ignored), or none.
    /// `burning` lights it, as a Flame bow does: an arrow sets what it hits on
    /// fire. Its id, or None for another kind or a direction that isn't one.
    /// API 21.
    pub(super) fn native_shoot(
        &mut self,
        kind: &str,
        from: [f64; 3],
        direction: [f64; 3],
        speed: f32,
        owner: Option<u32>,
        burning: bool,
    ) -> Option<u32> {
        let kind = projectile_kind(kind)?;
        let length = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
        if !from.iter().chain(&direction).all(|v| v.is_finite())
            || length < 1.0e-6
            || !speed.is_finite()
        {
            return None;
        }
        let owner = owner.filter(|id| self.mobs.iter().any(|m| m.id == *id && !m.removed));
        let mut p = Projectile::new(
            kind,
            Vec3::new(from[0], from[1], from[2]),
            owner.map(Who::Mob),
        );
        p.shoot(&mut self.world.random, direction, speed.max(0.0), 0.0);
        if burning {
            p.bonus.flame = true;
            if let Some(a) = p.arrow.as_mut() {
                a.ignite_for_seconds(100.0);
            }
        }
        p.needs_sync = true;
        self.add_projectile(p);
        Some(self.next_entity)
    }
    /// The positions within `radius` blocks (a cube, at most 32) of
    /// `center` whose block is `block` (`minecraft:obsidian`) or in the
    /// block tag `#tag` (`#minecraft:logs`), nearest first, at most `limit`.
    /// Only loaded chunks are searched. API 21.
    pub(super) fn native_find_blocks(
        &self,
        center: [i32; 3],
        radius: i32,
        block: &str,
        limit: usize,
    ) -> Vec<[i32; 3]> {
        let radius = radius.clamp(0, 32);
        let tag = block.strip_prefix('#');
        let wanted = match tag {
            Some(t) if !t.contains(':') => format!("minecraft:{t}"),
            Some(t) => t.to_owned(),
            None if !block.contains(':') => format!("minecraft:{block}"),
            None => block.to_owned(),
        };
        let tags = &self.res.items().block_tags;
        let members = tag.map(|_| tags.get(&wanted));
        if members.is_some_and(|m| m.is_none()) {
            return Vec::new();
        }
        let facts = self.res.facts();
        let (bottom, top) = (self.world.bottom_y(), self.world.top_y());
        let mut found = Vec::new();
        for x in center[0] - radius..=center[0] + radius {
            for z in center[2] - radius..=center[2] + radius {
                if !self.world.loaded(x, z) {
                    continue;
                }
                let ys = (center[1] - radius).max(bottom)..=(center[1] + radius).min(top - 1);
                for y in ys {
                    let name = &*facts.get(self.world.block(x, y, z)).name;
                    let hit = match members {
                        Some(Some(set)) => set.contains(name),
                        _ => name == wanted,
                    };
                    if hit {
                        found.push([x, y, z]);
                    }
                }
            }
        }
        let d = |p: &[i32; 3]| {
            (0..3)
                .map(|i| i64::from(p[i] - center[i]).pow(2))
                .sum::<i64>()
        };
        found.sort_by_key(d);
        found.truncate(limit);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::projectile_kind;

    #[test]
    fn projectiles_are_named_by_their_entity_type() {
        for (name, id) in [
            ("arrow", "minecraft:arrow"),
            ("minecraft:trident", "minecraft:trident"),
            ("fireball", "minecraft:fireball"),
            ("small_fireball", "minecraft:small_fireball"),
            ("wither_skull", "minecraft:wither_skull"),
        ] {
            assert_eq!(projectile_kind(name).map(|k| k.id()), Some(id), "{name}");
        }
        assert!(projectile_kind("minecraft:zombie").is_none());
        assert!(projectile_kind("ender_pearl").is_none());
    }
}
