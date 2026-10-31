//! The creaking heart block entity in the game: `mobs::creaking_heart`
//! ticks each heart (spawning, letting go of and bleeding for its creaking)
//! and this applies what it reports to the world, the mobs and the speakers.
use super::{Game, creatures::seen};
use horizon_client::{
    Result,
    containers::{BlockEntity, CreakingHeart},
    mobs::{
        Mob, MobType,
        creaking_heart::{self, HeartEvent, HeartHost},
    },
    particles::{ParticleOptions, ParticleType},
};
use horizon_core::{
    geometry::Vec3,
    position::BlockPos,
    random::{LegacyRandom, RandomSource},
};
use horizon_world::registry::BlockStateId;
use std::{collections::HashMap, sync::Arc};

type Pos = (i32, i32, i32);

/// Comparator input of a heart (`getAnalogOutputSignal`).
pub(super) fn heart_signal(world: &horizon_client::world::World, pos: BlockPos) -> Option<i32> {
    match world.block_entities.get(&(pos.x, pos.y, pos.z))? {
        BlockEntity::CreakingHeart(h) => Some(h.output_signal),
        _ => None,
    }
}

/// The game's mobs as a heart sees them, taken out of the game for the tick.
struct Host<'a> {
    mobs: &'a mut Vec<Mob>,
    added: Vec<Mob>,
    uuids: &'a mut HashMap<u32, [i32; 4]>,
    creaking: Option<Arc<MobType>>,
    /// Content mods' creaking variants, in id order.
    variants: Vec<Arc<MobType>>,
    next_entity: &'a mut u32,
    spawning: bool,
}

/// Content mods' creaking variants (entities with a `creaking` block), in
/// id order so a log two mods both name always grows the same one.
pub(super) fn creaking_variants(types: &HashMap<String, Arc<MobType>>) -> Vec<Arc<MobType>> {
    let mut v: Vec<(&String, &Arc<MobType>)> = types
        .iter()
        .filter(|(_, t)| t.custom.as_ref().is_some_and(|c| c.creaking.is_some()))
        .collect();
    v.sort_by(|a, b| a.0.cmp(b.0));
    v.into_iter().map(|(_, t)| t.clone()).collect()
}
impl HeartHost for Host<'_> {
    fn mob(&self, id: u32) -> Option<&Mob> {
        self.mobs
            .iter()
            .chain(self.added.iter())
            .find(|m| m.id == id && !m.removed)
    }
    fn mob_mut(&mut self, id: u32) -> Option<&mut Mob> {
        self.mobs
            .iter_mut()
            .chain(self.added.iter_mut())
            .find(|m| m.id == id && !m.removed)
    }
    fn mob_for_uuid(&self, uuid: [i32; 4]) -> Option<u32> {
        self.uuids
            .iter()
            .find(|(id, u)| **u == uuid && self.mob(**id).is_some())
            .map(|(id, _)| *id)
    }
    fn uuid_for(&mut self, id: u32) -> [i32; 4] {
        *self
            .uuids
            .entry(id)
            .or_insert_with(horizon_client::save::random_uuid)
    }
    fn creaking_type(&self) -> Option<Arc<MobType>> {
        self.creaking.clone()
    }
    fn create_mob(&mut self, kind: Arc<MobType>, at: Vec3) -> Mob {
        *self.next_entity += 1;
        // A new entity's random is its own (Mob::new seeds it).
        let mut m = Mob::new(*self.next_entity, kind, at, 0.0);
        let seed = i64::from(m.id);
        horizon_client::mobs::monster_brain::init(&mut m, seed);
        m
    }
    fn add_mob(&mut self, m: Mob) {
        self.added.push(m);
    }
    fn spawning_monsters(&self) -> bool {
        self.spawning
    }
    fn creaking_variants(&self) -> Vec<Arc<MobType>> {
        self.variants.clone()
    }
}

impl Game<'_> {
    fn heart_state_name(&self, state: BlockStateId) -> String {
        self.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get("creaking_heart_state").cloned())
            .unwrap_or_default()
    }
    /// Runs `f` on the heart at `key` with the game's mobs and level random,
    /// then applies what the heart reported. `others`: whether `f` reads the
    /// other mobs through the mob world (without, it holds none: building them
    /// costs a pass over every mob).
    fn with_heart(
        &mut self,
        key: Pos,
        heart: &mut CreakingHeart,
        others: bool,
        f: impl FnOnce(
            &mut CreakingHeart,
            &horizon_client::mobs::MobWorld,
            &mut LegacyRandom,
            &mut Host,
            &mut Vec<HeartEvent>,
        ),
    ) -> Result<()> {
        // A server has no player of its own: on the ticks a heart looks for
        // one (`getNearestPlayer`), it sees the nearest player in the level,
        // as mobs do (`lan_mobs`).
        let guest = (others && self.player.is_none())
            .then(|| {
                let at = Vec3::new(f64::from(key.0), f64::from(key.1), f64::from(key.2));
                self.lan_guest_views()
                    .into_iter()
                    .map(|(_, v)| v)
                    .filter(|v| v.alive)
                    .min_by(|a, b| {
                        at.distance_squared(a.position)
                            .total_cmp(&at.distance_squared(b.position))
                    })
            })
            .flatten();
        let mut mobs = std::mem::take(&mut self.mobs);
        let mut uuids = std::mem::take(&mut self.persist.uuids);
        let mut next_entity = self.next_entity;
        let mut random = self.world.random.clone();
        let spawning = self.spawning_monsters();
        let creaking = self.res.mob_types().get("minecraft:creaking").cloned();
        let variants = creaking_variants(self.res.mob_types());
        let player_uuid = self.level.player_uuid;
        let snapshot: Vec<_> = if others {
            mobs.iter().map(|m| seen(m, player_uuid)).collect()
        } else {
            Vec::new()
        };
        let mut events = Vec::new();
        let added = {
            let mut w = self.mob_world_base(&snapshot);
            if w.player.is_none()
                && let Some(v) = guest
            {
                w.nearest_player = (!v.spectator).then_some(v.position);
                w.player = Some(v);
            }
            let mut host = Host {
                mobs: &mut mobs,
                added: Vec::new(),
                uuids: &mut uuids,
                creaking,
                variants,
                next_entity: &mut next_entity,
                spawning,
            };
            f(heart, &w, &mut random, &mut host, &mut events);
            host.added
        };
        mobs.extend(added);
        self.mobs = mobs;
        self.persist.uuids = uuids;
        self.next_entity = next_entity;
        self.world.random = random;
        let mut mob_events = Vec::new();
        for e in events {
            match e {
                HeartEvent::SetBlock(at, state) => self.level_set(at, state)?,
                HeartEvent::Sound(name, at, volume, pitch) => {
                    self.audio
                        .play(&format!("minecraft:{name}"), Some(at), volume, pitch)
                }
                HeartEvent::Trail {
                    at,
                    target,
                    color,
                    duration,
                } => self.particles.add(
                    &ParticleOptions::trail(target, color, duration),
                    at,
                    [0.0; 3],
                ),
                HeartEvent::OutputChanged(at) => self.container_changed((at[0], at[1], at[2])),
                HeartEvent::Mob(event) => mob_events.push(event),
            }
        }
        self.handle_mob_events(mob_events)?;
        let _ = key;
        Ok(())
    }
    /// A creaking bound to the heart at `home` was hurt (`creakingHurt`).
    pub(super) fn creaking_hurt(&mut self, home: [i32; 3], _id: u32, by_player: bool) {
        if !by_player {
            return;
        }
        let key = (home[0], home[1], home[2]);
        let Some(BlockEntity::CreakingHeart(mut heart)) = self.world.block_entities.take(&key)
        else {
            return;
        };
        let _ = self.with_heart(key, &mut heart, true, |h, w, lr, host, out| {
            creaking_heart::creaking_hurt(h, home, w, lr, host, out);
        });
        self.world
            .block_entities
            .put(key, BlockEntity::CreakingHeart(heart));
    }
    /// A creaking variant that multiplies when hurt (Grafted Creaking's
    /// bamboo): another of `kind`, free of the heart, on a block with room
    /// within 5 of the heart at `home` (`SpawnUtil.trySpawnMob(.., 5, 16,
    /// 8, ON_TOP_OF_COLLIDER_NO_LEAVES)`), with the poof of its arrival.
    pub(super) fn creaking_multiply(&mut self, kind: &str, home: [i32; 3]) {
        for _ in 0..16 {
            let r = &mut self.world.random;
            let x = home[0] + r.next_int_between_inclusive(-5, 5).unwrap_or(0);
            let z = home[2] + r.next_int_between_inclusive(-5, 5).unwrap_or(0);
            if !self.world.loaded(x, z) {
                continue;
            }
            let found = {
                let facts = self.res.facts();
                let world = &self.world;
                let solid = |y: i32| {
                    let f = facts.get(world.block(x, y, z));
                    f.full_collision && !f.name.ends_with("leaves")
                };
                let air = |y: i32| facts.is_air(world.block(x, y, z));
                (home[1] - 8..=home[1] + 8)
                    .rev()
                    .find(|&y| solid(y - 1) && air(y) && air(y + 1) && air(y + 2))
            };
            let Some(y) = found else {
                continue;
            };
            let at = [f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5];
            if let Some(id) = self.spawn_mob(kind, at, false) {
                // `broadcastEntityEvent(creaking, 60)`: the poof.
                self.lan_entity_event(id as i32, 60, at);
                return;
            }
        }
    }
    /// `Creaking.tearDown`: the crumble particles.
    pub(super) fn creaking_tear_down(&mut self, center: [f64; 3], spread: [f64; 3]) {
        let registry = self.pack.assets.registry;
        let crumble = |this: &mut Self, name: &str, props: &[(&str, &str)], count: i32| {
            let Some(mut state) = registry
                .default_state(name)
                .and_then(|s| registry.state(s).cloned())
            else {
                return;
            };
            for (k, v) in props {
                state.properties.insert((*k).to_owned(), (*v).to_owned());
            }
            let Ok(id) = registry.resolve(&state) else {
                return;
            };
            let Some((sprite, tint)) = this.particle_sprite(id, center.map(|c| c.floor() as i32))
            else {
                return;
            };
            for _ in 0..count {
                let r = &mut this.world.random;
                let p = [
                    center[0] + r.next_gaussian() * spread[0],
                    center[1] + r.next_gaussian() * spread[1],
                    center[2] + r.next_gaussian() * spread[2],
                ];
                this.particles.add(
                    &ParticleOptions::block(ParticleType::BlockCrumble, sprite.clone(), tint),
                    p,
                    [0.0; 3],
                );
            }
        };
        crumble(self, "minecraft:pale_oak_wood", &[], 100);
        crumble(
            self,
            "minecraft:creaking_heart",
            &[("creaking_heart_state", "awake")],
            10,
        );
    }
    /// `CreakingHeartBlock.playerWillDestroy`: the heart's creaking twitches to
    /// death and a natural heart drops experience. `onExplosionHit` does the
    /// same for the creaking (`by_player` false), without the experience.
    pub(super) fn creaking_heart_broken(&mut self, pos: [i32; 3], by_player: bool) {
        let key = (pos[0], pos[1], pos[2]);
        if !matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::CreakingHeart(_))
        ) {
            return;
        }
        let Some(BlockEntity::CreakingHeart(mut heart)) = self.world.block_entities.take(&key)
        else {
            return;
        };
        let _ = self.with_heart(key, &mut heart, false, |h, _, _, host, out| {
            creaking_heart::remove_protector(h, host, true, out);
        });
        let state = self.world.block(key.0, key.1, key.2);
        let natural = self
            .pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get("natural").cloned())
            .is_some_and(|v| v == "true");
        if by_player && natural && !self.creative && !self.spectator() {
            let xp = 20 + self.world.random.next_int_bound(5).unwrap_or(0) as u32;
            self.spawn_orbs(
                [
                    f64::from(pos[0]) + 0.5,
                    f64::from(pos[1]) + 0.5,
                    f64::from(pos[2]) + 0.5,
                ],
                xp,
            );
        }
        self.world
            .block_entities
            .put(key, BlockEntity::CreakingHeart(heart));
    }
    /// `CreakingHeartBlockEntity.serverTick` of the heart at `key`. Its slot
    /// stays while uprooted, so that a heart whose block went away is still
    /// found here and lets go of its protector.
    pub(super) fn tick_creaking_heart(&mut self, key: Pos) -> Result<()> {
        let Some(BlockEntity::CreakingHeart(mut heart)) = self.world.block_entities.take(&key)
        else {
            return Ok(());
        };
        let state = self.world.block(key.0, key.1, key.2);
        if &*self.res.facts().get(state).name != "minecraft:creaking_heart" {
            // The block is gone: preRemoveSideEffects.
            self.world.block_entities.forget(&key);
            self.with_heart(key, &mut heart, false, |h, _, _, host, out| {
                creaking_heart::remove_protector(h, host, false, out);
            })?;
            return Ok(());
        }
        // getTicker: none once uprooted.
        if self.heart_state_name(state) != "uprooted" {
            let pos = [key.0, key.1, key.2];
            let others = creaking_heart::tick_updates(&heart);
            self.with_heart(key, &mut heart, others, |h, w, lr, host, out| {
                creaking_heart::server_tick(h, pos, w, lr, host, out);
            })?;
        }
        self.world
            .block_entities
            .put(key, BlockEntity::CreakingHeart(heart));
        Ok(())
    }
    /// `ServerLevel.isSpawningMonsters`.
    fn spawning_monsters(&self) -> bool {
        self.ui.difficulty != 0
            && self.ui.rules.bool("spawn_mobs")
            && self.ui.rules.bool("spawn_monsters")
    }
    /// Hearts that came with generated or loaded chunks get their block
    /// entity (`newBlockEntity`) the first time their chunk is seen.
    pub(super) fn scan_for_hearts(&mut self) {
        let Some(heart) = self
            .pack
            .assets
            .registry
            .default_state("minecraft:creaking_heart")
        else {
            return;
        };
        let keys: Vec<(i32, i32)> = self
            .world
            .chunks
            .iter()
            .filter(|(k, c)| c.lit && !self.creaking.scanned.contains(k))
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            self.creaking.scanned.insert(key);
            let Some(chunk) = self.world.chunks.get(&key).cloned() else {
                continue;
            };
            if self.creaking.hearts.is_empty() {
                self.creaking.hearts = self
                    .res
                    .facts()
                    .states
                    .iter()
                    .map(|f| &*f.name == "minecraft:creaking_heart")
                    .collect();
            }
            let hearts = &self.creaking.hearts;
            let _ = heart;
            let is_heart = |id: u32| hearts.get(id as usize).copied().unwrap_or(false);
            for (si, section) in chunk.sections.iter().enumerate() {
                // Only a section whose palette holds a heart state can have
                // one: the others are skipped without reading their 4,096 blocks.
                if !section.blocks.maybe_has(is_heart) {
                    continue;
                }
                for i in 0..4096 {
                    let id = section.get(i);
                    if !is_heart(id) {
                        continue;
                    }
                    let (lx, lz, ly) = ((i & 15) as i32, ((i >> 4) & 15) as i32, (i >> 8) as i32);
                    let pos = (
                        key.0 * 16 + lx,
                        self.world.bottom_y() + si as i32 * 16 + ly,
                        key.1 * 16 + lz,
                    );
                    self.world
                        .block_entities
                        .get_or_insert_with(pos, || BlockEntity::CreakingHeart(Box::default()));
                }
            }
        }
        let chunks = &self.world.chunks;
        self.creaking.scanned.retain(|k| chunks.contains_key(k));
    }
}

/// What the creaking code remembers between ticks.
#[derive(Default)]
pub(super) struct CreakingState {
    /// Chunks already scanned for heart blocks.
    scanned: horizon_client::fast_hash::FxHashSet<(i32, i32)>,
    /// Which block states are creaking hearts, by state id.
    hearts: Vec<bool>,
}

impl CreakingState {
    /// Exchange the chunks scanned for creaking hearts with another level's
    /// (game/levels.rs).
    pub(super) fn swap_level(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.scanned, &mut other.scanned);
    }
}
