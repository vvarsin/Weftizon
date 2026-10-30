//! Custom entities a content mod declares in `data/<id>/horizon/entity/<name>.json`
//! (docs/mods.md#custom-entities): a vanilla mob's behaviour under a new id,
//! with its own size, stats, skin, model, animations, sounds and spawns.
//!
//! Files are found by the entity's name, as Weft finds them:
//! `assets/<id>/textures/entity/<name>.png` (and `<name>_e.png`, the parts that
//! glow), `assets/<id>/models/entity/<name>.geo.json`,
//! `assets/<id>/animations/entity/<name>.animation.json` and
//! `assets/<id>/sounds/entity/<name>/<event>*.ogg`. The declaration names
//! others when they are called something else.
use serde::Deserialize;
use std::collections::BTreeMap;

/// `MobCategory` names a declaration may use.
pub const CATEGORIES: [&str; 8] = [
    "monster",
    "creature",
    "ambient",
    "axolotls",
    "underground_water_creature",
    "water_creature",
    "water_ambient",
    "misc",
];

/// The sounds the game plays for a mob, by event.
pub const SOUND_EVENTS: [&str; 4] = ["ambient", "hurt", "death", "step"];

fn one() -> u32 {
    1
}

/// A natural spawn: the entity joins these biomes' spawn lists, in its category.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    /// Biome ids, or biome tags as `#namespace:path`.
    pub biomes: Vec<String>,
    #[serde(default = "one")]
    pub weight: u32,
    #[serde(default = "one")]
    pub min_count: u32,
    #[serde(default = "one")]
    pub max_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EntityFile {
    template: String,
    width: Option<f64>,
    height: Option<f64>,
    eye_height: Option<f64>,
    health: Option<f64>,
    speed: Option<f64>,
    attack_damage: Option<f64>,
    attack_knockback: Option<f64>,
    armor: Option<f64>,
    follow_range: Option<f64>,
    knockback_resistance: Option<f64>,
    category: Option<String>,
    fire_immune: Option<bool>,
    goals: Option<Vec<crate::behaviour::Goal>>,
    targets: Option<Vec<crate::behaviour::Target>>,
    /// How much bigger the model is drawn than the template's.
    scale: Option<f32>,
    texture: Option<String>,
    #[serde(default)]
    variants: Vec<String>,
    /// A layer drawn full bright over the body (an Entity Texture Features `_e` texture).
    emissive: Option<String>,
    model: Option<String>,
    animations: Option<String>,
    #[serde(default)]
    sounds: BTreeMap<String, String>,
    loot_table: Option<String>,
    #[serde(default)]
    spawns: Vec<Spawn>,
}

/// Values that replace the template's; unset keeps it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub eye_height: Option<f64>,
    /// `minecraft:max_health`.
    pub health: Option<f64>,
    /// `minecraft:movement_speed`.
    pub speed: Option<f64>,
    pub attack_damage: Option<f64>,
    pub attack_knockback: Option<f64>,
    pub armor: Option<f64>,
    pub follow_range: Option<f64>,
    pub knockback_resistance: Option<f64>,
    pub category: Option<String>,
    pub fire_immune: Option<bool>,
}

/// A texture: one of the mod's (`<ns>:entity/<path>`, as the entity atlas
/// names mod textures) or vanilla's (`entity/<path>`).
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub atlas: String,
    /// The mod file it is, for the mod's own.
    pub file: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    /// `<mod id>:<name>`.
    pub id: String,
    /// The vanilla mob it copies, `minecraft:<path>`.
    pub template: String,
    pub stats: Stats,
    pub scale: Option<f32>,
    /// The skin; `None` keeps the template's.
    pub texture: Option<Texture>,
    /// More skins: each entity wears one of `texture` and these.
    pub variants: Vec<Texture>,
    /// `<texture>_e.png` files of the mod: Weft's glow masks (red × alpha), by skin.
    pub glow_masks: BTreeMap<String, String>,
    /// A full-bright layer drawn over the body as it is.
    pub emissive: Option<Texture>,
    /// The `.geo.json` mod file of its model; `None` keeps the template's model.
    pub model: Option<String>,
    /// The `.animation.json` mod file.
    pub animations: Option<String>,
    /// Sound event per event key (`ambient`, `hurt`, `death`, `step`).
    pub sounds: BTreeMap<String, String>,
    /// Sound files the mod has for an event it names no event for, by event
    /// key: the game makes `<ns>:entity.<name>.<event>` of them.
    pub sound_files: BTreeMap<String, Vec<String>>,
    pub loot_table: Option<String>,
    pub spawns: Vec<Spawn>,
    /// Its AI goals, in place of the template's; `None` keeps them.
    pub goals: Option<Vec<crate::behaviour::Goal>>,
    /// Its target selectors, in place of the template's; `None` keeps them.
    pub targets: Option<Vec<crate::behaviour::Target>>,
}

pub(crate) fn resource_id(s: &str) -> bool {
    let (ns, path) = s.split_once(':').unwrap_or(("minecraft", s));
    let ok = |p: &str, extra: &[u8]| {
        !p.is_empty()
            && p.bytes().all(|b| {
                b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b"_.-".contains(&b)
                    || extra.contains(&b)
            })
    };
    ok(ns, b"") && ok(path, b"/")
}

impl Entity {
    /// Reads and checks a declaration. `files` are the mod's content files
    /// (sorted); `file` reads one of them.
    pub(crate) fn read(
        id: String,
        decl: EntityFile,
        mod_id: &str,
        owned: &[String],
        files: &[String],
        file: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Entity, String> {
        let name = id
            .split_once(':')
            .map_or(id.as_str(), |(_, n)| n)
            .to_owned();
        if !decl.template.starts_with("minecraft:") || !resource_id(&decl.template) {
            return Err(format!(
                "template {:?} is not a vanilla mob (minecraft:...)",
                decl.template
            ));
        }
        let positive = |v: Option<f64>, what: &str, max: f64| match v {
            Some(x) if !(x.is_finite() && x > 0.0 && x <= max) => {
                Err(format!("{what} is above 0 and at most {max}"))
            }
            _ => Ok(()),
        };
        positive(decl.width, "width", 64.0)?;
        positive(decl.height, "height", 64.0)?;
        positive(decl.health, "health", 1024.0)?;
        positive(decl.speed, "speed", 1024.0)?;
        positive(decl.follow_range, "follow_range", 2048.0)?;
        if let Some(e) = decl.eye_height
            && !(e.is_finite() && (0.0..=64.0).contains(&e))
        {
            return Err("eye_height is 0 to 64".into());
        }
        for (v, what, max) in [
            (decl.attack_damage, "attack_damage", 2048.0),
            (decl.attack_knockback, "attack_knockback", 5.0),
            (decl.armor, "armor", 30.0),
            (decl.knockback_resistance, "knockback_resistance", 1.0),
        ] {
            if let Some(x) = v
                && !(x.is_finite() && (0.0..=max).contains(&x))
            {
                return Err(format!("{what} is 0 to {max}"));
            }
        }
        if let Some(c) = &decl.category
            && !CATEGORIES.contains(&c.as_str())
        {
            return Err(format!(
                "category {c:?} is none of {}",
                CATEGORIES.join(", ")
            ));
        }
        if let Some(s) = decl.scale
            && !(s.is_finite() && s > 0.0 && s <= 16.0)
        {
            return Err("scale is above 0 and at most 16".into());
        }
        // A name in a folder of the mod's assets: `ghost`, `mymod:ghost`, or vanilla's `minecraft:...`.
        let resolve = |name: &str,
                       folder: &str,
                       suffix: &str,
                       vanilla: bool|
         -> Result<(String, Option<String>), String> {
            if !resource_id(name) {
                return Err(format!("{name:?} is not a resource name"));
            }
            let (ns, path) = name.split_once(':').unwrap_or((mod_id, name));
            if ns == "minecraft" {
                return if vanilla {
                    Ok((format!("entity/{path}"), None))
                } else {
                    Err(format!(
                        "{name}: vanilla has no {folder} files; the mod brings its own"
                    ))
                };
            }
            if !owned.iter().any(|o| o == ns) {
                return Err(format!(
                    "{name}: names another mod's files; a mod reads its own namespaces"
                ));
            }
            let file = format!("assets/{ns}/{folder}/entity/{path}{suffix}");
            if files.binary_search(&file).is_err() {
                return Err(format!("{file} is not in the mod"));
            }
            Ok((format!("{ns}:entity/{path}"), Some(file)))
        };
        let texture_of = |name: &str| -> Result<Texture, String> {
            let (atlas, file) = resolve(name, "textures", ".png", true)?;
            Ok(Texture { atlas, file })
        };
        let own = |folder: &str, suffix: &str| {
            let file = format!("assets/{mod_id}/{folder}/entity/{name}{suffix}");
            files.binary_search(&file).is_ok().then_some(file)
        };
        let texture = match &decl.texture {
            Some(t) => Some(texture_of(t)?),
            None => own("textures", ".png").map(|file| Texture {
                atlas: format!("{mod_id}:entity/{name}"),
                file: Some(file),
            }),
        };
        if !decl.variants.is_empty() && texture.is_none() {
            return Err("variants need a texture to vary".into());
        }
        let variants = decl
            .variants
            .iter()
            .map(|v| texture_of(v))
            .collect::<Result<Vec<_>, _>>()?;
        let mut glow_masks = BTreeMap::new();
        for t in texture.iter().chain(&variants) {
            if let Some(file) = &t.file {
                let mask = format!("{}_e.png", file.trim_end_matches(".png"));
                if files.binary_search(&mask).is_ok() {
                    glow_masks.insert(t.atlas.clone(), mask);
                }
            }
        }
        let emissive = decl.emissive.as_deref().map(texture_of).transpose()?;
        let model = match &decl.model {
            Some(m) => resolve(m, "models", ".geo.json", false)?.1,
            None => own("models", ".geo.json"),
        };
        let animations = match &decl.animations {
            Some(a) => resolve(a, "animations", ".animation.json", false)?.1,
            None => own("animations", ".animation.json"),
        };
        // The files must read: a broken model refuses the mod, as a broken block does.
        let mut renames = BTreeMap::new();
        if let Some(m) = &model {
            let g = crate::bedrock::read_geometry(&file(m)?).map_err(|e| format!("{m}: {e}"))?;
            renames = g.renames;
        } else if animations.is_some() && decl.animations.is_some() {
            return Err("animations need a model (models/entity/<name>.geo.json) to move".into());
        }
        if let Some(a) = animations.as_ref().filter(|_| model.is_some()) {
            crate::bedrock::read_animations(&file(a)?, &renames)
                .map_err(|e| format!("{a}: {e}"))?;
        }
        let animations = animations.filter(|_| model.is_some());
        for (event, sound) in &decl.sounds {
            if !SOUND_EVENTS.contains(&event.as_str()) {
                return Err(format!(
                    "sound {event:?} is none of {}",
                    SOUND_EVENTS.join(", ")
                ));
            }
            if !resource_id(sound) {
                return Err(format!("sound {sound:?} is not a sound event name"));
            }
        }
        let mut sound_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let folder = format!("assets/{mod_id}/sounds/entity/{name}/");
        for f in files.iter().filter(|f| f.starts_with(&folder)) {
            let stem = &f[folder.len()..];
            let Some(stem) = stem.strip_suffix(".ogg") else {
                continue;
            };
            if stem.contains('/') {
                continue;
            }
            if let Some(event) = SOUND_EVENTS.iter().find(|e| stem.starts_with(**e))
                && !decl.sounds.contains_key(*event)
            {
                sound_files
                    .entry((*event).to_owned())
                    .or_default()
                    .push(format!("{mod_id}:entity/{name}/{stem}"));
            }
        }
        let mut sounds = decl.sounds.clone();
        for event in sound_files.keys() {
            sounds.insert(event.clone(), format!("{mod_id}:entity.{name}.{event}"));
        }
        crate::behaviour::check(
            &decl.template,
            decl.goals.as_deref(),
            decl.targets.as_deref(),
        )?;
        if let Some(t) = &decl.loot_table
            && !resource_id(t)
        {
            return Err(format!("loot_table {t:?} is not a resource name"));
        }
        for s in &decl.spawns {
            if s.biomes.is_empty() {
                return Err("a spawn names no biomes".into());
            }
            if let Some(b) = s
                .biomes
                .iter()
                .find(|b| !resource_id(b.trim_start_matches('#')))
            {
                return Err(format!("spawn biome {b:?} is not a biome or #tag"));
            }
            if s.weight == 0 || s.min_count == 0 || s.min_count > s.max_count || s.max_count > 64 {
                return Err(
                    "a spawn has weight 1 or more and 1 <= min_count <= max_count <= 64".into(),
                );
            }
        }
        Ok(Entity {
            id,
            template: decl.template,
            stats: Stats {
                width: decl.width,
                height: decl.height,
                eye_height: decl.eye_height,
                health: decl.health,
                speed: decl.speed,
                attack_damage: decl.attack_damage,
                attack_knockback: decl.attack_knockback,
                armor: decl.armor,
                follow_range: decl.follow_range,
                knockback_resistance: decl.knockback_resistance,
                category: decl.category,
                fire_immune: decl.fire_immune,
            },
            scale: decl.scale,
            texture,
            variants,
            glow_masks,
            emissive,
            model,
            animations,
            sounds,
            sound_files,
            loot_table: decl.loot_table,
            spawns: decl.spawns,
            goals: decl.goals,
            targets: decl.targets,
        })
    }
}
