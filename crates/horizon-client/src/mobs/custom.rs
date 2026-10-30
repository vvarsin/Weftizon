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
use super::{Mob, Part, Polygon, anim, keyframes};
use horizon_mods::bedrock::{self, Loop};
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
}
fn one() -> f32 {
    1.0
}

/// A declared entity's own looks and sounds, on its [`super::MobType`].
#[derive(Clone, Debug, Default)]
pub struct Custom {
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
    /// the time, `walk` as much as it walks, `hurt` after a hit and `death`
    /// while it dies.
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
        play("idle", s.age / 20.0, 1.0, p);
        if s.speed > 0.01 {
            play("walk", s.age / 20.0, (s.speed * 2.0).min(1.0), p);
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
