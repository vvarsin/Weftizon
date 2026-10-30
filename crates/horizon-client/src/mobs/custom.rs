//! Entities content mods declare (docs/mods.md#custom-entities): a vanilla
//! mob's type under a new id, its behaviour (the species) kept and its looks,
//! size, stats, sounds and spawns its own.
//!
//! `mod_content` writes the declarations into the content root
//! ([`FILE`], and a row per entity in `entity-types.json` with the declared
//! size and attributes); [`super::MobTypes::load`] makes each a copy of its
//! template's [`super::MobType`] with [`Custom`] attached. Bedrock models and
//! animations are read with `horizon_mods::bedrock` into the Java model space
//! the renderer draws.
use super::{Mob, Part, Polygon, anim, goals, keyframes};
use horizon_mods::{
    bedrock::{self, Loop},
    behaviour,
};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

/// In a content root: the declared entities ([`Entry`] list).
pub const FILE: &str = "horizon-mod-entities.json";
/// The layer key of a declared entity's own model.
pub const LAYER: &str = "custom#main";

/// One declared entity as `mod_content` resolved it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub template: String,
    /// Atlas names of its skins; empty keeps the template's.
    #[serde(default)]
    pub textures: Vec<String>,
    /// For each skin, the full-bright layer made from its glow mask.
    #[serde(default)]
    pub glows: Vec<Option<String>>,
    /// A full-bright layer over every skin, drawn as it is.
    pub emissive: Option<String>,
    /// The `.geo.json` in the root; none keeps the template's model.
    pub model: Option<String>,
    pub animations: Option<String>,
    #[serde(default = "one")]
    pub scale: f32,
    /// Sound event per event key (`ambient`, `hurt`, `death`, `step`).
    #[serde(default)]
    pub sounds: BTreeMap<String, String>,
    pub loot_table: Option<String>,
    /// Its AI goals and target selectors in place of the template's.
    #[serde(default)]
    pub goals: Option<Vec<behaviour::Goal>>,
    #[serde(default)]
    pub targets: Option<Vec<behaviour::Target>>,
}
fn one() -> f32 {
    1.0
}

/// A declared goal or target selector with what it names resolved: its
/// priority, the entities it acts on and the items that tempt it.
#[derive(Clone, Debug)]
pub struct Planned<T> {
    pub priority: i32,
    pub spec: T,
    /// It names `minecraft:player`.
    pub player: bool,
    pub kinds: goals::Kinds,
    pub items: Arc<[String]>,
}

/// Entity ids as what a goal matches: the player, vanilla species, mods'
/// entities. A vanilla id that is no mob is left out.
fn kinds(owner: &str, names: &[String]) -> (bool, goals::Kinds) {
    let mut player = false;
    let mut kinds = goals::Kinds::default();
    for name in names {
        match name.strip_prefix("minecraft:") {
            Some("player") => player = true,
            Some(path) => match super::species::ALL.iter().find(|d| d.name == path) {
                Some(d) => kinds.species.push(d.species),
                None => log::warn!("Horizon: {owner}: {name} is not a mob"),
            },
            None => kinds.declared.push(intern(name)),
        }
    }
    (player, kinds)
}

/// The declared goals and targets of entity `owner`, resolved; `item_tag`
/// lists the items of an item tag.
pub fn plan(
    owner: &str,
    goals: Option<&[behaviour::Goal]>,
    targets: Option<&[behaviour::Target]>,
    item_tag: &dyn Fn(&str) -> Vec<String>,
) -> (
    Option<Vec<Planned<behaviour::Goal>>>,
    Option<Vec<Planned<behaviour::Target>>>,
) {
    let none: Arc<[String]> = Arc::from(Vec::new());
    let goals = goals.map(|list| {
        list.iter()
            .enumerate()
            .map(|(i, g)| {
                let (player, kinds) = match g {
                    behaviour::Goal::Avoid { entities, .. } => kinds(owner, entities),
                    _ => Default::default(),
                };
                let items = match g {
                    behaviour::Goal::Tempt { items, .. } => {
                        let mut all = Vec::new();
                        for item in items {
                            match item.strip_prefix('#') {
                                Some(tag) => all.extend(item_tag(tag)),
                                None => all.push(item.clone()),
                            }
                        }
                        all.sort();
                        all.dedup();
                        Arc::from(all)
                    }
                    _ => none.clone(),
                };
                Planned {
                    priority: g.priority().unwrap_or(i as i32),
                    spec: g.clone(),
                    player,
                    kinds,
                    items,
                }
            })
            .collect()
    });
    let targets = targets.map(|list| {
        list.iter()
            .enumerate()
            .map(|(i, t)| {
                let (player, kinds) = match t {
                    behaviour::Target::NearestAttackable { entities, .. } => kinds(owner, entities),
                    behaviour::Target::HurtBy { .. } => Default::default(),
                };
                Planned {
                    priority: t.priority().unwrap_or(i as i32),
                    spec: t.clone(),
                    player,
                    kinds,
                    items: none.clone(),
                }
            })
            .collect()
    });
    (goals, targets)
}

/// `registerGoals` for a mod's entity: its declared goals and target
/// selectors in place of the template's (which the template registered
/// first, with the rest of its set-up).
pub fn register(m: &mut Mob) {
    use super::goals::{
        Avoid, AvoidEntity, FleeSun, Float, LeapAtTarget, LookAtPlayer, MeleeAttack, NearestTarget,
        Panic, Prey, RandomLookAround, RestrictSun, Stroll, Tempt,
    };
    use behaviour::{Goal as G, Target as T};
    let Some(c) = m.kind.custom.clone() else {
        return;
    };
    if let Some(list) = &c.goals {
        m.ai.goals.clear();
        for p in list {
            let g = &mut m.ai.goals;
            let at = p.priority;
            match &p.spec {
                G::Float { .. } => g.add(at, Float),
                G::Melee {
                    speed,
                    follow_unseen,
                    reach,
                    interval,
                    ..
                } => {
                    let mut goal =
                        MeleeAttack::new(*speed, *follow_unseen).interval(*interval as i32);
                    if let Some(r) = reach {
                        goal = goal.reach(*r);
                    }
                    g.add(at, goal);
                }
                G::LeapAtTarget { velocity_y, .. } => g.add(at, LeapAtTarget::new(*velocity_y)),
                G::Ranged {
                    projectile,
                    speed,
                    interval,
                    range,
                    ..
                } => {
                    let fire: super::witch::Fire = match projectile.as_str() {
                        "snowball" => super::golem::snowball_attack,
                        "llama_spit" => super::llama::spit,
                        _ => |m, who, power, cx| {
                            super::undead::perform_ranged_attack(m, who, cx, power)
                        },
                    };
                    let interval = *interval as i32;
                    g.add(
                        at,
                        super::witch::RangedAttack::new(*speed, interval, interval, *range, fire),
                    );
                }
                G::Wander {
                    speed,
                    interval,
                    avoid_water,
                    ..
                } => {
                    let stroll = if *avoid_water {
                        Stroll::avoiding_water(*speed)
                    } else {
                        Stroll::new(*speed)
                    };
                    g.add(at, stroll.interval(*interval as i32));
                }
                G::LookAtPlayer {
                    distance,
                    probability,
                    ..
                } => {
                    let mut look = LookAtPlayer::new(*distance);
                    look.probability = *probability;
                    g.add(at, look);
                }
                G::RandomLookAround { .. } => g.add(at, RandomLookAround::default()),
                G::Panic { speed, .. } => g.add(at, Panic::new(*speed)),
                G::Tempt { speed, scared, .. } => {
                    g.add(at, Tempt::new(*speed, p.items.clone(), *scared));
                }
                G::Avoid {
                    distance,
                    walk_speed,
                    sprint_speed,
                    ..
                } => {
                    if p.player {
                        g.add(
                            at,
                            AvoidEntity::new(Avoid::Player, *distance, *walk_speed, *sprint_speed),
                        );
                    }
                    if p.kinds != Default::default() {
                        g.add(
                            at,
                            AvoidEntity::new(
                                Avoid::Kinds(p.kinds.clone()),
                                *distance,
                                *walk_speed,
                                *sprint_speed,
                            ),
                        );
                    }
                }
                G::RestrictSun { .. } => g.add(at, RestrictSun),
                G::FleeSun { speed, .. } => g.add(at, FleeSun::new(*speed)),
            }
        }
    }
    if let Some(list) = &c.targets {
        m.ai.targets.clear();
        for p in list {
            let t = &mut m.ai.targets;
            match &p.spec {
                T::HurtBy { alert_others, .. } => {
                    let goal = super::goals::HurtByTarget::new();
                    t.add(p.priority, if *alert_others { goal.alert() } else { goal });
                }
                T::NearestAttackable {
                    must_see, interval, ..
                } => {
                    let interval = *interval as i32;
                    if p.player {
                        t.add(
                            p.priority,
                            NearestTarget::player(*must_see).interval(interval),
                        );
                    }
                    if p.kinds != Default::default() {
                        t.add(
                            p.priority,
                            NearestTarget::new(Prey::Kinds(p.kinds.clone()), *must_see)
                                .interval(interval),
                        );
                    }
                }
            }
        }
    }
}

/// A declared entity's own looks and sounds, on its [`super::MobType`].
#[derive(Clone, Debug, Default)]
pub struct Custom {
    /// Its id (`mymod:ghost`), one copy for the game's life.
    pub id: &'static str,
    pub textures: Vec<String>,
    pub glows: Vec<Option<String>>,
    pub emissive: Option<String>,
    /// Its model is the mod's (under [`LAYER`]), not the template's.
    pub own_model: bool,
    pub scale: f32,
    pub step: Option<&'static str>,
    /// The loot table its death drops; `None` is its template's.
    pub loot_table: Option<String>,
    /// Keyframe animations by name (`idle`, `walk`, `hurt`, `death`...).
    pub animations: HashMap<String, (keyframes::Animation, Loop)>,
    /// Declared goals and target selectors ([`register`]); `None` keeps the template's.
    pub goals: Option<Vec<Planned<behaviour::Goal>>>,
    pub targets: Option<Vec<Planned<behaviour::Target>>>,
}

/// One copy of each sound event name, kept for the game's life: mob sounds are `&'static str`.
pub fn intern(name: &str) -> &'static str {
    static NAMES: OnceLock<Mutex<std::collections::HashSet<&'static str>>> = OnceLock::new();
    let mut names = NAMES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(n) = names.get(name) {
        return n;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(leaked);
    leaked
}

/// A mob sound's event id: vanilla's are written without their namespace,
/// a mod's with it.
pub fn sound_id(name: &str) -> Cow<'_, str> {
    if name.contains(':') {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("minecraft:{name}"))
    }
}

/// The declarations in a content root, if it has any.
pub fn entries(root: &Path) -> Vec<Entry> {
    let Ok(bytes) = std::fs::read(root.join(FILE)) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        log::warn!("Horizon: {FILE}: {e}");
        Vec::new()
    })
}

/// A Bedrock part as the renderer's [`Part`].
fn part(p: &bedrock::Part) -> Part {
    Part {
        name: p.name.clone(),
        pose: p.pose,
        polygons: p
            .quads
            .iter()
            .map(|q| Polygon {
                normal: q.normal,
                vertices: q.vertices,
            })
            .collect(),
        children: p.children.iter().map(part).collect(),
    }
}

/// The model and animations an entry names, read from `root`. A file that
/// no longer reads (the root changed under the game) leaves the template's.
pub fn load_model(
    root: &Path,
    e: &Entry,
) -> Option<(Arc<Part>, HashMap<String, (keyframes::Animation, Loop)>)> {
    let path = e.model.as_ref()?;
    let geometry = std::fs::read(root.join(path))
        .map_err(|err| err.to_string())
        .and_then(|b| bedrock::read_geometry(&b));
    let geometry = match geometry {
        Ok(g) => g,
        Err(err) => {
            log::warn!("Horizon: {}: {path}: {err}", e.id);
            return None;
        }
    };
    for w in &geometry.warnings {
        log::warn!("Horizon: {}: {path}: {w}", e.id);
    }
    let mut animations = HashMap::new();
    if let Some(path) = &e.animations {
        match std::fs::read(root.join(path))
            .map_err(|err| err.to_string())
            .and_then(|b| bedrock::read_animations(&b, &geometry.renames))
        {
            Ok((list, warnings)) => {
                for w in warnings {
                    log::warn!("Horizon: {}: {path}: {w}", e.id);
                }
                for a in list {
                    animations.insert(a.name.clone(), (animation(&a), a.looping));
                }
            }
            Err(err) => log::warn!("Horizon: {}: {path}: {err}", e.id),
        }
    }
    Some((Arc::new(part(&geometry.root)), animations))
}

/// A Bedrock animation as vanilla's keyframe animation. It never loops by
/// itself: [`Custom::animate`] picks the time.
fn animation(a: &bedrock::Animation) -> keyframes::Animation {
    use keyframes::{Channel, Keyframe, Target};
    keyframes::Animation {
        length: a.length.max(f32::EPSILON),
        looping: false,
        bones: a
            .bones
            .iter()
            .map(|(bone, channels)| {
                let channels = channels
                    .iter()
                    .map(|c| Channel {
                        target: match c.target {
                            bedrock::Target::Position => Target::Position,
                            bedrock::Target::Rotation => Target::Rotation,
                            bedrock::Target::Scale => Target::Scale,
                        },
                        keyframes: c
                            .keyframes
                            .iter()
                            .map(|k| Keyframe {
                                time: k.time,
                                pre: k.pre,
                                post: k.post,
                                catmull: k.catmull,
                            })
                            .collect(),
                    })
                    .collect();
                (bone.clone(), channels)
            })
            .collect(),
    }
}

/// The custom payload in which a server tells its clients the animation a
/// mod plays on an entity: its id (varint), the animation's name (empty: none
/// any more), whether it repeats, and the ticks since it started (varint), so
/// a player who comes near later sees it at the same point.
pub const ANIMATION_CHANNEL: &str = "horizon:play_animation";
/// Characters of an animation's name at most.
pub const MAX_ANIMATION_NAME: usize = 64;

/// A named animation a mod plays on an entity (Weft's `play_animation`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Playing {
    pub name: String,
    pub repeat: bool,
    /// The entity's age when it started.
    pub started: u32,
}

impl Mob {
    /// Plays the animation `name` of a mod's entity in place of `idle` and
    /// `walk`: once, or over and over when `repeat`. An empty name goes back
    /// to idle and walk. Every player who sees the entity sees it.
    pub fn play_animation(&mut self, name: &str, repeat: bool) {
        self.extra.animation = (!name.is_empty()).then(|| Playing {
            name: name.chars().take(MAX_ANIMATION_NAME).collect(),
            repeat,
            started: self.age,
        });
    }
}

/// [`ANIMATION_CHANNEL`]'s payload for entity `id`.
pub fn animation_payload(id: i32, playing: Option<&Playing>, elapsed: u32) -> Vec<u8> {
    let mut w = horizon_protocol::buf::Writer::new();
    w.varint(id);
    w.string(playing.map_or("", |p| p.name.as_str()));
    w.bool(playing.is_some_and(|p| p.repeat));
    w.varint(elapsed.min(i32::MAX as u32) as i32);
    w.data
}

/// An [`ANIMATION_CHANNEL`] payload: the entity id, and the animation's
/// name, repeat and ticks since it started (none for an empty name).
pub fn read_animation_payload(data: &[u8]) -> Option<(i32, Option<(String, bool, u32)>)> {
    let mut r = horizon_protocol::buf::Reader::new(data);
    let id = r.varint().ok()?;
    let name = r.string(MAX_ANIMATION_NAME * 4).ok()?;
    let repeat = r.bool().ok()?;
    let elapsed = r.varint().ok()?.max(0) as u32;
    Some((id, (!name.is_empty()).then_some((name, repeat, elapsed))))
}

/// Each type of mods' entities alive among `mobs`, by id, with the ids of
/// its entities in their order: the batches of `native_entity_tick`.
pub fn tick_batches(mobs: &[Mob]) -> BTreeMap<&'static str, Vec<u32>> {
    let mut batches: BTreeMap<&'static str, Vec<u32>> = BTreeMap::new();
    for m in mobs {
        if let Some(c) = &m.kind.custom
            && m.alive()
            && !m.removed
        {
            batches.entry(c.id).or_default().push(m.id);
        }
    }
    batches
}

/// Takes the hits mods' entities took out of `mobs` (`Extra::hurts`), in
/// the mobs' order: the type, the entity, the damage and what it took.
pub fn take_hurts(mobs: &mut [Mob]) -> Vec<(&'static str, u32, super::Damage, f32)> {
    let mut hits = Vec::new();
    for m in mobs {
        if let Some(c) = &m.kind.custom
            && !m.extra.hurts.is_empty()
        {
            hits.extend(m.extra.hurts.drain(..).map(|(d, a)| (c.id, m.id, d, a)));
        }
    }
    hits
}

/// A client: the animation of an [`ANIMATION_CHANNEL`] payload on the entity
/// it names, timed from the client's own clock of the entity. False when the
/// payload doesn't read or the client has no such entity.
pub fn receive_animation(mobs: &mut [Mob], data: &[u8]) -> bool {
    let Some((id, playing)) = read_animation_payload(data) else {
        return false;
    };
    let Some(m) = mobs.iter_mut().find(|m| m.id == id as u32) else {
        return false;
    };
    m.extra.animation = playing.map(|(name, repeat, elapsed)| Playing {
        name,
        repeat,
        started: m.age.saturating_sub(elapsed),
    });
    true
}

impl Custom {
    /// Which of its skins this entity wears: the same for one entity id,
    /// so every client of a server draws it alike.
    pub fn skin_index(&self, m: &Mob) -> usize {
        if self.textures.len() <= 1 {
            return 0;
        }
        // A fixed mix of the id (SplitMix64's finaliser), not the world's random.
        let mut x = u64::from(m.id).wrapping_add(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= x >> 31;
        (x % self.textures.len() as u64) as usize
    }
    /// The skin this entity wears, if not the template's.
    pub fn texture(&self, m: &Mob) -> Option<&str> {
        self.textures.get(self.skin_index(m)).map(String::as_str)
    }
    /// The full-bright layers over its body: its skin's glow, then the emissive texture.
    pub fn bright_layers(&self, m: &Mob) -> impl Iterator<Item = &str> {
        let glow = self
            .glows
            .get(self.skin_index(m))
            .and_then(|g| g.as_deref());
        glow.into_iter().chain(self.emissive.as_deref())
    }
    /// Adds the keyframe animations over the template's pose: `idle` all
    /// the time, `walk` as much as it walks, or in their place the one a mod
    /// plays ([`Mob::play_animation`]) while it plays; `attack` from each
    /// swing, `hurt` after a hit and `death` while it dies.
    pub fn animate(&self, m: &Mob, s: &anim::State, p: &mut anim::Poses) {
        if self.animations.is_empty() {
            return;
        }
        let play = |name: &str, seconds: f32, scale: f32, p: &mut anim::Poses| {
            let Some((a, looping)) = self.animations.get(name) else {
                return;
            };
            let seconds = match looping {
                Loop::Repeat => seconds % a.length,
                Loop::Hold => seconds.min(a.length),
                Loop::Once if seconds > a.length => return,
                Loop::Once => seconds,
            };
            a.apply((seconds * 1000.0) as i64, scale, p);
        };
        let since = |tick: u32| (s.age - tick as f32).max(0.0) / 20.0;
        // `play_animation(id, name, repeat)`: over and over when `repeat`,
        // else once (or held on its last frame), then idle and walk again.
        let played = m.extra.animation.as_ref().and_then(|playing| {
            let (a, looping) = self.animations.get(&playing.name)?;
            let seconds = since(playing.started);
            let seconds = if playing.repeat {
                seconds % a.length
            } else if *looping == Loop::Hold {
                seconds.min(a.length)
            } else if seconds > a.length {
                return None;
            } else {
                seconds
            };
            Some((a, seconds))
        });
        match played {
            Some((a, seconds)) => a.apply((seconds * 1000.0) as i64, 1.0, p),
            None => {
                play("idle", s.age / 20.0, 1.0, p);
                if s.speed > 0.01 {
                    play("walk", s.age / 20.0, (s.speed * 2.0).min(1.0), p);
                }
            }
        }
        // Weft's attack keyframes: once from each swing (a melee hit).
        if let Some(start) = m.anim.swing_started
            && let Some((a, _)) = self.animations.get("attack")
            && since(start) <= a.length
        {
            a.apply((since(start) * 1000.0) as i64, 1.0, p);
        }
        if m.hurt_time > 0 {
            play(
                "hurt",
                (10 - m.hurt_time) as f32 / 20.0 + s.partial / 20.0,
                1.0,
                p,
            );
        }
        if m.death_time > 0 {
            play("death", (m.death_time as f32 + s.partial) / 20.0, 1.0, p);
        }
    }
}

#[cfg(test)]
#[path = "custom_tests.rs"]
mod tests;
