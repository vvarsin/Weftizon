//! The World API for native mods, API 68 (docs/native-mods.md, "Spawning
//! with gear, effects, weather and projectiles"): spawning any entity from
//! its saved data, potion effects and fire on mobs and players, lightning and
//! weather, shooting projectiles, and finding blocks around a place. What a
//! mod such as Invasion Mod needs beside API 67's waves. They run where the
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
    /// API 68.
    pub(super) fn native_summon_snbt(
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
    /// immune to it (a wither to wither, a spider to poison). API 68.
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
    /// unknown. API 68.
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
    /// level. API 68.
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
    /// `setVisualOnly`). API 68.
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
    /// another kind. API 68.
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
    /// API 68.
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
    /// Only loaded chunks are searched. API 68.
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

/// Players and light for native mods (API 70).
#[allow(dead_code)]
impl Game<'_> {
    /// The player `uuid` as a command target: the one whose turn it is, or a
    /// guest in play.
    fn native_player_target(&self, uuid: u128) -> Option<Target> {
        if self.lan_acting.is_some()
            && horizon_client::lan::playerdata::ints_uuid(self.acting_uuid()) == uuid
        {
            return Some(Target::Player);
        }
        let host = self.lan.as_ref()?;
        host.hub
            .guests
            .iter()
            .find(|g| g.uuid == uuid && g.placed)
            .map(|g| Target::Guest(g.conn))
    }
    /// Hurts the player `uuid` by `amount` of damage type `damage_type`
    /// (`minecraft:magic`, `minecraft:generic`...), as `/damage` does: armour,
    /// enchantments and effects count, creative players take none. `attacker`
    /// is the mob the hit is credited to (death messages, thorns), if any.
    /// Works inside a player's own hook (the one whose turn it is) too. False
    /// when they aren't on, the type is unknown, or nothing was dealt.
    /// API 70.
    pub(super) fn native_hurt_player(
        &mut self,
        uuid: u128,
        amount: f32,
        damage_type: &str,
        attacker: Option<u32>,
    ) -> bool {
        if !(amount.is_finite() && amount > 0.0) {
            return false;
        }
        let ty = if damage_type.contains(':') {
            damage_type.to_owned()
        } else {
            format!("minecraft:{damage_type}")
        };
        let Some(target) = self.native_player_target(uuid) else {
            return false;
        };
        self.for_player(target, |g| {
            !g.spectator() && g.hurt_player_from(amount, None, &ty, Some(attacker))
        })
        .unwrap_or(false)
    }
    /// Plays `sound` (`minecraft:block.beacon.activate`, a mod's own event) at
    /// `at` in the level in place for every player near it, as
    /// `GameLevel::native_play_sound` does from a level hook: `source` is the
    /// sound slider (0 master, 4 blocks, 5 hostile...), `volume` 1 is heard
    /// 16 blocks away. From any server hook. False for a name that isn't one
    /// or a number that isn't one. API 70.
    pub(super) fn native_sound(
        &mut self,
        sound: &str,
        at: [f64; 3],
        source: usize,
        volume: f32,
        pitch: f32,
    ) -> bool {
        let named = sound.split_once(':').is_some_and(|(ns, path)| {
            !ns.is_empty()
                && !path.is_empty()
                && sound
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-/:".contains(&b))
        });
        if !named
            || source >= horizon_client::settings::SOUND_SOURCES.len()
            || !at.iter().all(|v| v.is_finite())
            || !(volume.is_finite() && volume >= 0.0 && pitch.is_finite() && pitch >= 0.0)
        {
            return false;
        }
        self.creatures
            .lan_sounds
            .push(super::creatures::NativeSound {
                dimension: self.dim.id(),
                sound: sound.to_owned(),
                at,
                source: source as i32,
                volume,
                pitch,
            });
        true
    }
    /// The block light and sky light at `pos` (0 to 15 each) in the level in
    /// place: glowstone's 15 next to it, fading a level a block. None where no
    /// chunk is loaded. API 70.
    pub(super) fn native_light(&self, pos: [i32; 3]) -> Option<(u8, u8)> {
        if !self.world.loaded(pos[0], pos[2]) {
            return None;
        }
        let l = self.world.light(pos[0], pos[1], pos[2]);
        Some((l & 15, l >> 4))
    }
}

/// Explosions and hunting one player for native mods (API 71).
#[allow(dead_code)]
impl Game<'_> {
    /// An explosion at `center` of `power` (TNT 4, a charged creeper 6, at
    /// most 32), as `Level.explode`: it hurts and pushes what is near,
    /// breaks blocks unless `break_blocks` is false (drops as TNT's do), and
    /// lights fires when `fire`. In the level in place. False for a power or
    /// position that isn't one. API 71.
    pub(super) fn native_explode(
        &mut self,
        center: [f64; 3],
        power: f32,
        fire: bool,
        break_blocks: bool,
    ) -> bool {
        if !center.iter().all(|v| v.is_finite()) || !(power.is_finite() && power > 0.0) {
            return false;
        }
        let interaction = if break_blocks {
            super::explosion::BlockInteraction::DestroyWithDecay
        } else {
            super::explosion::BlockInteraction::Keep
        };
        match self.server_explode(center, power.min(32.0), interaction, fire) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("a native mod's explosion at {center:?}: {e}");
                false
            }
        }
    }
    /// Sends mob `id` after the player `uuid`: it targets them, and keeps
    /// them over nearer players while they are a valid target (alive, not in
    /// creative or spectator), as a mob keeps a player who hit it. A mob
    /// that thinks with a brain (a piglin, a warden) gets them as its attack
    /// target. False when the mob or the player isn't there. API 71.
    pub(super) fn native_target_player(&mut self, id: u32, uuid: u128) -> bool {
        let conn = self.lan.as_ref().and_then(|host| {
            host.hub
                .guests
                .iter()
                .find(|g| g.uuid == uuid && g.placed)
                .map(|g| g.conn)
        });
        let local = conn.is_none()
            && self.player.is_some()
            && horizon_client::lan::playerdata::ints_uuid(self.acting_uuid()) == uuid;
        if conn.is_none() && !local {
            return false;
        }
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && !m.removed && m.alive())
        else {
            return false;
        };
        m.ai.target = Some(horizon_client::mobs::Who::Player);
        m.ai.last_hurt_by = Some(horizon_client::mobs::Who::Player);
        m.ai.last_hurt_by_time = m.age as i32;
        horizon_client::mobs::monster_brain::set_attack_target(
            m,
            Some(horizon_client::mobs::Who::Player),
        );
        if let (Some(conn), Some(host)) = (conn, self.lan.as_mut()) {
            host.mob_players.insert(id, conn);
        }
        true
    }
    /// Sends mob `id` after another mob `target` (a defender after an
    /// invader): it targets it as a mob targets one that hit it
    /// (`Mob.setTarget`), and a mob that thinks with a brain gets it as its
    /// attack target. The mob drops it once it dies. False when either is not
    /// there, or they are the same mob. API 72.
    pub(super) fn native_target_mob(&mut self, id: u32, target: u32) -> bool {
        use horizon_client::mobs::Who;
        if id == target
            || !self
                .mobs
                .iter()
                .any(|m| m.id == target && !m.removed && m.alive())
        {
            return false;
        }
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && !m.removed && m.alive())
        else {
            return false;
        };
        m.ai.target = Some(Who::Mob(target));
        m.ai.last_hurt_by = Some(Who::Mob(target));
        m.ai.last_hurt_by_time = m.age as i32;
        horizon_client::mobs::monster_brain::set_attack_target(m, Some(Who::Mob(target)));
        true
    }
    /// Puts `item` (`minecraft:iron_sword`, None for nothing) in mob `id`'s
    /// main hand, as `Mob.setItemSlot`: players see it, and a skeleton
    /// switches between its bow and melee goals by it
    /// (`AbstractSkeleton.reassessWeaponGoal`). False when the mob isn't
    /// there or the item doesn't exist. API 73.
    pub(super) fn native_set_held(&mut self, id: u32, item: Option<&str>) -> bool {
        if let Some(item) = item
            && self.res.items().get(item).is_none()
        {
            return false;
        }
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && !m.removed && m.alive())
        else {
            return false;
        };
        m.extra.set_main_hand(item.map(Into::into));
        true
    }
}

/// Creaking hearts in trees for native mods (API 69).
#[allow(dead_code)]
impl Game<'_> {
    /// The creaking that grows between logs `name`: a content mod's variant
    /// whose `creaking.heart_logs` names it (`invasion:spruce_creaking`),
    /// `minecraft:creaking` for pale oak, else None.
    pub(super) fn native_creaking_for_log(&self, name: &str) -> Option<String> {
        if self.block_in_tag("minecraft:pale_oak_logs", name) {
            return Some("minecraft:creaking".into());
        }
        let named = |spec: &str| match spec.strip_prefix('#') {
            Some(t) if t.contains(':') => self.block_in_tag(t, name),
            Some(t) => self.block_in_tag(&format!("minecraft:{t}"), name),
            None if spec.contains(':') => spec == name,
            None => name.strip_prefix("minecraft:") == Some(spec),
        };
        super::creaking::creaking_variants(self.res.mob_types())
            .into_iter()
            .find(|t| {
                t.custom
                    .as_ref()
                    .and_then(|c| c.creaking.as_ref())
                    .is_some_and(|c| c.heart_logs.iter().any(|s| named(s)))
            })
            .map(|t| t.id.to_string())
    }
    /// Plants a creaking heart in the trunk of a tree near `center` (within
    /// `radius`, at most 32), as a pale oak grows one: a log with logs of the
    /// same wood above and below, upright, turned into a heart that wakes at
    /// its creaking's hours and grows the creaking its wood names (a mod's
    /// variant, Grafted Creaking-style, or vanilla's in pale oak). With
    /// `creaking`, only a trunk whose wood grows that one (`invasion:spruce_creaking`).
    /// The heart's position and the creaking it grows, nearest first; None
    /// when no such trunk is loaded. API 69.
    pub(super) fn native_plant_creaking_heart(
        &mut self,
        center: [i32; 3],
        radius: i32,
        creaking: Option<&str>,
    ) -> Option<([i32; 3], String)> {
        let logs = self.native_find_blocks(center, radius, "#minecraft:logs", 4096);
        let name_at = |g: &Self, p: [i32; 3]| {
            g.res
                .facts()
                .get(g.world.block(p[0], p[1], p[2]))
                .name
                .clone()
        };
        let upright = |g: &Self, p: [i32; 3]| {
            g.native_block_at(p)
                .is_some_and(|b| b.properties.get("axis").is_none_or(|a| a == "y"))
        };
        for p in logs {
            let (below, above) = ([p[0], p[1] - 1, p[2]], [p[0], p[1] + 1, p[2]]);
            let wood = name_at(self, p);
            if name_at(self, below) != wood || name_at(self, above) != wood {
                continue;
            }
            if !(upright(self, p) && upright(self, below) && upright(self, above)) {
                continue;
            }
            // One heart to a tree: none within 2 blocks across (a dark oak's
            // trunk is 2 by 2) or 4 up or down.
            let hearted = (-2..=2).any(|dx| {
                (-2..=2).any(|dz| {
                    (-4..=4).any(|dy| {
                        name_at(self, [p[0] + dx, p[1] + dy, p[2] + dz]).as_ref()
                            == "minecraft:creaking_heart"
                    })
                })
            });
            if hearted {
                continue;
            }
            let Some(grows) = self.native_creaking_for_log(&wood) else {
                continue;
            };
            if creaking.is_some_and(|c| c != grows) {
                continue;
            }
            if self
                // Dormant, as `getStateForPlacement` makes it between its
                // logs: its own tick wakes it at its creaking's hours.
                .native_set_block_at(
                    p,
                    "minecraft:creaking_heart[axis=y,natural=true,creaking_heart_state=dormant]",
                )
                .is_ok()
            {
                return Some((p, grows));
            }
        }
        None
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
