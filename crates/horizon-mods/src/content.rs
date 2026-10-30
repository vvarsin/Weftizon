//! A content mod's `assets/` and `data/` (API 0.2, docs/modpacks.md): the
//! files it adds to the game's resource tree, and the blocks, items and
//! entities it declares in `data/<id>/horizon/block/`, `item/` and `entity/`.
//!
//! Content is data, not code: nothing here runs in the sandbox. What a mod
//! may add is fenced instead. Its own namespaces (its id and the ones its
//! manifest lists), plus additions to vanilla tags and to the convention
//! tags (`c`, which NeoForge and Fabric mods share). Paths are plain, files
//! and totals are capped, vanilla ids cannot be redefined, and no two mods
//! share a namespace ([`clash`]).
use crate::manifest::{Capability, MANIFEST, Manifest};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
};

/// Content files per mod, bytes per file and bytes per mod.
pub const MAX_FILES: usize = 65_536;
pub const MAX_FILE: u64 = 16 << 20;
pub const MAX_TOTAL: u64 = 512 << 20;

/// Checks a content path: `assets/` or `data/`, then `/`-separated parts of
/// `[a-z0-9_.-]`, none of them `.` or `..` (vanilla's resource-location
/// characters, so the path means the same on every OS).
pub fn check_path(name: &str) -> Result<(), String> {
    let bad = || format!("{name:?} is not a plain assets/ or data/ path");
    let mut parts = name.split('/');
    if !matches!(parts.next(), Some("assets" | "data")) {
        return Err(bad());
    }
    let mut count = 0;
    for part in parts {
        count += 1;
        let chars_ok = part
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b));
        if part.is_empty() || part == "." || part == ".." || !chars_ok {
            return Err(bad());
        }
    }
    // assets/<namespace>/<file> at least.
    if count < 2 {
        return Err(bad());
    }
    Ok(())
}

/// A vanilla identifier, `minecraft:path`.
fn vanilla_id(id: &str) -> bool {
    id.strip_prefix("minecraft:").is_some_and(|path| {
        !path.is_empty()
            && path
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_./-".contains(&b))
    })
}

/// What a declared block changes from its template; unset keeps the
/// template's value.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BlockProperties {
    /// Destroy time in seconds of hand mining (-1: unbreakable).
    pub hardness: Option<f32>,
    pub explosion_resistance: Option<f32>,
    /// Light emitted, 0 to 15.
    pub light: Option<u8>,
    pub requires_tool: Option<bool>,
    pub friction: Option<f32>,
    /// A `MapColor` id, 0 to 61.
    pub map_color: Option<u8>,
    /// A vanilla block whose sounds this block uses.
    pub sounds: Option<String>,
}

/// Per-state outline and collision boxes, selected by one state property.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BlockShapes {
    pub property: String,
    pub values: BTreeMap<String, Vec<[f64; 6]>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockFile {
    template: String,
    #[serde(default)]
    properties: BlockProperties,
    shapes: Option<BlockShapes>,
    #[serde(default)]
    default_state: BTreeMap<String, String>,
    #[serde(default = "yes")]
    item: bool,
}
fn yes() -> bool {
    true
}

/// A block the mod adds: a vanilla template's behaviour, states and shapes
/// under a new id, with `properties` applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// `<mod id>:<path>`.
    pub id: String,
    /// The vanilla block it copies, `minecraft:<path>`.
    pub template: String,
    pub properties: BlockProperties,
    pub shapes: Option<BlockShapes>,
    /// State properties overlaid on the template default.
    pub default_state: BTreeMap<String, String>,
    /// Also add a block item with the same id.
    pub item: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemFile {
    template: String,
    max_stack_size: Option<u8>,
    rarity: Option<Rarity>,
    #[serde(default)]
    components: BTreeMap<String, serde_json::Value>,
}

/// An item the mod adds: a vanilla template item's components under a new
/// id, with these changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    /// The vanilla item it copies, `minecraft:<path>`.
    pub template: String,
    pub max_stack_size: Option<u8>,
    pub rarity: Option<Rarity>,
    /// A 26.3 data component patch over the template's: `!minecraft:<type>` removes one.
    pub components: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug)]
enum Source {
    Dir(PathBuf),
    Zip(PathBuf),
}

/// A mod's content: its files and declarations.
#[derive(Debug)]
pub struct Content {
    source: Source,
    /// Every content file (`assets/...`, `data/...`), sorted.
    pub files: Vec<String>,
    pub blocks: Vec<Block>,
    pub items: Vec<Item>,
    pub entities: Vec<crate::entity::Entity>,
    /// The namespaces it owns: the mod id, then the manifest's `namespaces`.
    pub namespaces: Vec<String>,
}

/// Reads a mod's content files ([`Content::reader`]).
pub enum FileReader<'a> {
    Dir(&'a Path),
    Zip(Box<zip::ZipArchive<std::fs::File>>),
}
impl FileReader<'_> {
    /// One content file's bytes.
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, String> {
        check_path(name)?;
        let reader: Box<dyn Read + '_> = match self {
            Self::Dir(root) => {
                Box::new(std::fs::File::open(root.join(name)).map_err(|e| format!("{name}: {e}"))?)
            }
            Self::Zip(zip) => Box::new(zip.by_name(name).map_err(|e| format!("{name}: {e}"))?),
        };
        let mut out = Vec::new();
        reader
            .take(MAX_FILE + 1)
            .read_to_end(&mut out)
            .map_err(|e| format!("{name}: {e}"))?;
        if out.len() as u64 > MAX_FILE {
            return Err(format!("{name} is over {} MiB", MAX_FILE >> 20));
        }
        Ok(out)
    }
}

/// Why two content mods cannot be on together: a namespace both own (one's
/// id or listed namespace is the other's), which would mix their files.
/// `a` and `b` are (mod id, owned namespaces).
pub fn clash(a: (&str, &[String]), b: (&str, &[String])) -> Option<String> {
    let shared = a.1.iter().find(|ns| b.1.contains(ns))?;
    Some(format!(
        "mods {} and {} both use the namespace {shared}; turn one of them off",
        a.0, b.0
    ))
}

impl Content {
    /// The content of the mod at `path` (a `.hmod` or a mod folder).
    pub fn read(path: &Path, manifest: &Manifest) -> Result<Content, String> {
        let (source, sized) = if path.is_dir() {
            let mut out = Vec::new();
            for top in ["assets", "data"] {
                walk(path, &path.join(top), &mut out)?;
            }
            (Source::Dir(path.to_owned()), out)
        } else {
            let mut zip = open_zip(path)?;
            let mut out = Vec::new();
            for i in 0..zip.len() {
                let entry = zip.by_index_raw(i).map_err(|e| e.to_string())?;
                let name = entry.name();
                if (name.starts_with("assets/") || name.starts_with("data/")) && !entry.is_dir() {
                    out.push((name.to_owned(), entry.size()));
                }
            }
            (Source::Zip(path.to_owned()), out)
        };
        let mut files = Vec::with_capacity(sized.len());
        let mut total = 0u64;
        for (name, size) in sized {
            check_path(&name)?;
            if size > MAX_FILE {
                return Err(format!("{name} is over {} MiB", MAX_FILE >> 20));
            }
            total += size;
            files.push(name);
        }
        if files.len() > MAX_FILES {
            return Err(format!("more than {MAX_FILES} content files"));
        }
        if total > MAX_TOTAL {
            return Err(format!("content is over {} MiB", MAX_TOTAL >> 20));
        }
        files.sort();
        files.dedup();
        let mut content = Content {
            source,
            files,
            blocks: Vec::new(),
            items: Vec::new(),
            entities: Vec::new(),
            namespaces: manifest.owned_namespaces().map(str::to_owned).collect(),
        };
        content.check(manifest)?;
        Ok(content)
    }

    /// Where the files are read from: the mod's folder or its `.hmod`.
    pub fn source(&self) -> &Path {
        match &self.source {
            Source::Dir(path) | Source::Zip(path) => path,
        }
    }

    /// No files: a mod with code only.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// One content file's bytes.
    pub fn file(&self, name: &str) -> Result<Vec<u8>, String> {
        self.reader()?.read(name)
    }

    /// A reader of its content files. A packaged mod's archive is opened
    /// once for all of them (opening it reads its whole index).
    pub fn reader(&self) -> Result<FileReader<'_>, String> {
        Ok(match &self.source {
            Source::Dir(root) => FileReader::Dir(root),
            Source::Zip(path) => FileReader::Zip(Box::new(open_zip(path)?)),
        })
    }

    /// Where content may go, and the declarations.
    fn check(&mut self, manifest: &Manifest) -> Result<(), String> {
        let id = manifest.id.as_str();
        if !self.files.is_empty() && !manifest.capabilities.contains(&Capability::Registries) {
            return Err(format!(
                "has assets/ or data/ but not capability registries in {MANIFEST}"
            ));
        }
        let mut blocks = Vec::new();
        let mut items = Vec::new();
        let mut entities = Vec::new();
        for name in &self.files {
            let mut parts = name.splitn(3, '/');
            let (top, namespace, rest) = (
                parts.next().unwrap_or(""),
                parts.next().unwrap_or(""),
                parts.next().unwrap_or(""),
            );
            let own = namespace == id;
            let owned = own || manifest.namespaces.iter().any(|n| n == namespace);
            // Vanilla's own files stay vanilla's; a mod may only add to its
            // tags, and to the convention tags mods share.
            let shared = matches!(namespace, "minecraft" | "c");
            let allowed = owned || (top == "data" && shared && rest.starts_with("tags/"));
            if !allowed {
                return Err(format!(
                    "{name}: a mod adds files in its own namespaces ({top}/{id}/ and those listed in namespaces) or to vanilla and convention (c) tags"
                ));
            }
            if !own && top == "data" && rest.starts_with("horizon/") {
                return Err(format!(
                    "{name}: blocks, items and entities are declared in data/{id}/horizon/"
                ));
            }
            let Some(decl) = (own && top == "data")
                .then(|| rest.strip_prefix("horizon/"))
                .flatten()
            else {
                continue;
            };
            let parse = |kind: &str| -> Option<String> {
                let path = decl
                    .strip_prefix(kind)?
                    .strip_prefix('/')?
                    .strip_suffix(".json")?;
                Some(format!("{id}:{path}"))
            };
            if let Some(block) = parse("block") {
                blocks.push((block, name.clone()));
            } else if let Some(item) = parse("item") {
                items.push((item, name.clone()));
            } else if let Some(entity) = parse("entity") {
                entities.push((entity, name.clone()));
            } else {
                return Err(format!(
                    "{name}: data/{id}/horizon/ holds block/*.json, item/*.json and entity/*.json only"
                ));
            }
        }
        let mut ids = BTreeSet::new();
        for (block, name) in blocks {
            let file: BlockFile =
                serde_json::from_slice(&self.file(&name)?).map_err(|e| format!("{name}: {e}"))?;
            check_block(&file).map_err(|e| format!("{name}: {e}"))?;
            if file.item {
                ids.insert(block.clone());
            }
            self.blocks.push(Block {
                id: block,
                template: file.template,
                properties: file.properties,
                shapes: file.shapes,
                default_state: file.default_state,
                item: file.item,
            });
        }
        for (item, name) in items {
            let file: ItemFile =
                serde_json::from_slice(&self.file(&name)?).map_err(|e| format!("{name}: {e}"))?;
            if !vanilla_id(&file.template) {
                return Err(format!(
                    "{name}: template {:?} is not a vanilla item (minecraft:...)",
                    file.template
                ));
            }
            if file.max_stack_size.is_some_and(|n| !(1..=99).contains(&n)) {
                return Err(format!("{name}: max_stack_size is 1 to 99"));
            }
            check_components(&file.components).map_err(|e| format!("{name}: {e}"))?;
            if !ids.insert(item.clone()) {
                return Err(format!("{name}: {item} is also a block's item"));
            }
            self.items.push(Item {
                id: item,
                template: file.template,
                max_stack_size: file.max_stack_size,
                rarity: file.rarity,
                components: file.components,
            });
        }
        for (entity, name) in entities {
            if entity.contains('/') {
                return Err(format!(
                    "{name}: an entity's name is one file name, not a folder path"
                ));
            }
            let file: crate::entity::EntityFile =
                serde_json::from_slice(&self.file(&name)?).map_err(|e| format!("{name}: {e}"))?;
            let mut read = |n: &str| self.file(n);
            let decl = crate::entity::Entity::read(
                entity,
                file,
                id,
                &self.namespaces,
                &self.files,
                &mut read,
            )
            .map_err(|e| format!("{name}: {e}"))?;
            self.entities.push(decl);
        }
        // A declared entity's spawn egg: an item copying a vanilla egg, with
        // its own entity id and name. A mod can give it its own item model.
        for e in &self.entities {
            let Some(egg) = &e.spawn_egg else {
                continue;
            };
            let (ns, path) = e.id.split_once(':').unwrap_or((id, &e.id));
            let item_id = format!("{ns}:{path}_spawn_egg");
            if !ids.insert(item_id.clone()) {
                return Err(format!("spawn egg item {item_id} is already an item"));
            }
            let mut components = BTreeMap::new();
            components.insert(
                "minecraft:entity_data".into(),
                serde_json::json!({ "id": e.id }),
            );
            components.insert(
                "minecraft:item_name".into(),
                serde_json::json!({ "translate": format!("item.{ns}.{path}_spawn_egg") }),
            );
            if let Some(model) = &egg.item_model {
                components.insert(
                    "minecraft:item_model".into(),
                    serde_json::Value::String(model.clone()),
                );
            }
            self.items.push(Item {
                id: item_id,
                template: "minecraft:zombie_spawn_egg".into(),
                max_stack_size: None,
                rarity: None,
                components,
            });
        }
        Ok(())
    }
}

/// Vanilla component types only, removals as `{}`, none the item file has its own key for.
fn check_components(components: &BTreeMap<String, serde_json::Value>) -> Result<(), String> {
    for (key, value) in components {
        let (removed, kind) = match key.strip_prefix('!') {
            Some(kind) => (true, kind),
            None => (false, key.as_str()),
        };
        if !vanilla_id(kind) {
            return Err(format!(
                "component {key:?} is not a vanilla type (minecraft:...)"
            ));
        }
        if let Some(own @ ("max_stack_size" | "rarity")) = kind.strip_prefix("minecraft:") {
            return Err(format!("component {key:?}: set {own} instead"));
        }
        if removed && !value.as_object().is_some_and(|o| o.is_empty()) {
            return Err(format!("component {key:?}: a removal's value is {{}}"));
        }
    }
    Ok(())
}

fn check_block(file: &BlockFile) -> Result<(), String> {
    if !vanilla_id(&file.template) {
        return Err(format!(
            "template {:?} is not a vanilla block (minecraft:...)",
            file.template
        ));
    }
    if let Some(shapes) = &file.shapes {
        if shapes.property.is_empty() || shapes.values.is_empty() {
            return Err("shapes needs a property and values".into());
        }
        for boxes in shapes.values.values() {
            if boxes.len() > 16
                || boxes
                    .iter()
                    .any(|b| b.iter().any(|n| !n.is_finite()) || (0..3).any(|i| b[i] > b[i + 3]))
            {
                return Err("shape needs at most 16 finite boxes with ordered bounds".into());
            }
        }
    }
    let p = &file.properties;
    let finite = |v: Option<f32>| v.is_none_or(f32::is_finite);
    if !finite(p.hardness) || !finite(p.explosion_resistance) || !finite(p.friction) {
        return Err("hardness, explosion_resistance and friction are numbers".into());
    }
    if p.hardness.is_some_and(|h| h < 0.0 && h != -1.0) {
        return Err("hardness is 0 or more, or -1 (unbreakable)".into());
    }
    if p.explosion_resistance.is_some_and(|r| r < 0.0) {
        return Err("explosion_resistance is 0 or more".into());
    }
    if p.friction.is_some_and(|f| !(0.0..=2.0).contains(&f)) {
        return Err("friction is 0 to 2".into());
    }
    if p.light.is_some_and(|l| l > 15) {
        return Err("light is 0 to 15".into());
    }
    if p.map_color.is_some_and(|c| c > 61) {
        return Err("map_color is a MapColor id, 0 to 61".into());
    }
    if p.sounds.as_deref().is_some_and(|s| !vanilla_id(s)) {
        return Err("sounds names a vanilla block (minecraft:...)".into());
    }
    Ok(())
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    zip::ZipArchive::new(file).map_err(|e| e.to_string())
}

/// Files under `dir`, relative to `root` with `/` separators. Symbolic links
/// are not followed: a mod folder holds its own files.
fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, u64)>) -> Result<(), String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in read.flatten() {
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.is_dir() {
            walk(root, &path, out)?;
        } else if meta.is_file() {
            let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
            let parts: Option<Vec<&str>> = rel.iter().map(|p| p.to_str()).collect();
            let name = parts.ok_or_else(|| format!("{} is not UTF-8", rel.display()))?;
            out.push((name.join("/"), meta.len()));
        }
        if out.len() > MAX_FILES {
            return Err(format!("more than {MAX_FILES} content files"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Package;

    const MANIFEST_TEXT: &str = r#"
id = "ruby"
name = "Ruby"
version = "0.1.0"
api = "0.2"
side = "both"
capabilities = ["registries"]
"#;
    const BLOCK: &[u8] = br#"{"template": "minecraft:iron_block",
        "properties": {"hardness": 4.0, "light": 3, "sounds": "minecraft:amethyst_block"}}"#;
    const ITEM: &[u8] = br#"{"template": "minecraft:diamond", "rarity": "uncommon"}"#;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("horizon-content-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    fn hmod(name: &str, manifest: &str, files: &[(&str, &[u8])]) -> Result<Content, String> {
        let dir = scratch(name);
        let path = dir.join("m.hmod");
        std::fs::write(&path, Package::write_files(manifest, None, files)?).unwrap();
        let package = Package::read(&path);
        let content = package.and_then(|p| Content::read(&path, &p.manifest));
        std::fs::remove_dir_all(&dir).unwrap();
        content
    }

    const GHOST_GEO: &[u8] = br#"{"format_version": "1.12.0", "minecraft:geometry": [{
        "description": {"texture_width": 64, "texture_height": 32},
        "bones": [{"name": "body", "pivot": [0, 0, 0], "cubes": [{"origin": [-4, 0, -2], "size": [8, 16, 4], "uv": [0, 0]}]},
                  {"name": "head", "parent": "body", "pivot": [0, 16, 0]}]}]}"#;
    const GHOST_ANIM: &[u8] = br#"{"animations": {"animation.ghost.idle": {"loop": true,
        "bones": {"head": {"rotation": {"0": [0, 0, 0], "1": [0, 20, 0]}}}}}}"#;

    /// The files a declaration doesn't name are found by the entity's name.
    #[test]
    fn entities_find_their_files_by_name() {
        let decl = br##"{"template": "minecraft:zombie", "health": 30, "width": 0.8,
            "category": "monster", "sounds": {"ambient": "minecraft:entity.ghast.ambient"},
            "variants": ["ghost_blue"],
            "spawns": [{"biomes": ["#minecraft:is_overworld", "minecraft:plains"], "weight": 5, "max_count": 2}]}"##;
        let content = hmod(
            "entity",
            MANIFEST_TEXT,
            &[
                ("data/ruby/horizon/entity/ghost.json", decl),
                ("assets/ruby/textures/entity/ghost.png", b"png"),
                ("assets/ruby/textures/entity/ghost_e.png", b"png"),
                ("assets/ruby/textures/entity/ghost_blue.png", b"png"),
                ("assets/ruby/models/entity/ghost.geo.json", GHOST_GEO),
                (
                    "assets/ruby/animations/entity/ghost.animation.json",
                    GHOST_ANIM,
                ),
                ("assets/ruby/sounds/entity/ghost/hurt1.ogg", b"ogg"),
                ("assets/ruby/sounds/entity/ghost/hurt2.ogg", b"ogg"),
                ("assets/ruby/sounds/entity/ghost/death.ogg", b"ogg"),
                ("assets/ruby/sounds/entity/ghost/ambient.ogg", b"ogg"),
            ],
        )
        .unwrap();
        let [e] = &content.entities[..] else {
            panic!("{:?}", content.entities)
        };
        assert_eq!(
            (e.id.as_str(), e.template.as_str()),
            ("ruby:ghost", "minecraft:zombie")
        );
        assert_eq!((e.stats.health, e.stats.width), (Some(30.0), Some(0.8)));
        assert_eq!(e.texture.as_ref().unwrap().atlas, "ruby:entity/ghost");
        assert_eq!(e.variants[0].atlas, "ruby:entity/ghost_blue");
        assert_eq!(
            e.glow_masks.get("ruby:entity/ghost").map(String::as_str),
            Some("assets/ruby/textures/entity/ghost_e.png")
        );
        assert_eq!(
            e.model.as_deref(),
            Some("assets/ruby/models/entity/ghost.geo.json")
        );
        assert!(e.animations.is_some());
        // The declared ambient sound wins over the file; hurt and death come from the files.
        assert_eq!(e.sounds["ambient"], "minecraft:entity.ghast.ambient");
        assert_eq!(e.sounds["hurt"], "ruby:entity.ghost.hurt");
        assert_eq!(
            e.sound_files["hurt"],
            ["ruby:entity/ghost/hurt1", "ruby:entity/ghost/hurt2"]
        );
        assert!(!e.sound_files.contains_key("ambient"));
        assert_eq!(e.spawns[0].weight, 5);
        assert_eq!((e.spawns[0].min_count, e.spawns[0].max_count), (1, 2));
        // A declaration alone keeps the template's looks.
        let bare = hmod(
            "entity-bare",
            MANIFEST_TEXT,
            &[(
                "data/ruby/horizon/entity/brute.json",
                br#"{"template": "minecraft:husk"}"#,
            )],
        )
        .unwrap();
        let b = &bare.entities[0];
        assert!(b.texture.is_none() && b.model.is_none() && b.sounds.is_empty());
        assert!(b.goals.is_none() && b.targets.is_none());
    }

    /// Goals and targets read with their defaults, in the declared order.
    #[test]
    fn entities_declare_goals_and_targets() {
        use crate::behaviour::{Goal, Target};
        let decl = br##"{"template": "minecraft:cow", "attack_damage": 4, "attack_knockback": 1.5,
            "components": {"ruby:mana": 10, "ruby:owner": "", "ruby:path": [[1, 2], [3]],
                           "ruby:stats": {"speed": 0.5, "tame": false}},
            "goals": [
                {"goal": "float"},
                {"goal": "melee", "priority": 2, "reach": 1.5},
                {"goal": "ranged", "projectile": "snowball", "range": 10},
                {"goal": "wander", "speed": 0.8, "avoid_water": false},
                {"goal": "tempt", "items": ["minecraft:wheat", "#minecraft:cow_food"]},
                {"goal": "avoid", "entities": ["minecraft:wolf", "ruby:ghost"]},
                {"goal": "look_at_player"}
            ],
            "targets": [
                {"target": "hurt_by", "alert_others": true},
                {"target": "nearest_attackable", "entities": ["minecraft:player"], "must_see": false}
            ],
            "spawn_egg": {"colors": [10, 20]},
            "solid": true, "kinematic": true, "carry_height": 0.5}"##;
        let content = hmod(
            "entity-goals",
            MANIFEST_TEXT,
            &[("data/ruby/horizon/entity/bull.json", decl)],
        )
        .unwrap();
        let e = &content.entities[0];
        assert_eq!(e.stats.attack_knockback, Some(1.5));
        assert_eq!(e.components.len(), 4);
        assert_eq!(e.components["ruby:mana"], 10);
        let egg = content
            .items
            .iter()
            .find(|i| i.id == "ruby:bull_spawn_egg")
            .unwrap();
        assert_eq!(egg.template, "minecraft:zombie_spawn_egg");
        assert_eq!(
            egg.components["minecraft:entity_data"],
            serde_json::json!({ "id": "ruby:bull" })
        );
        assert_eq!(
            (e.solid, e.kinematic, e.carry_height),
            (true, true, Some(0.5))
        );
        let goals = e.goals.as_ref().unwrap();
        assert_eq!(goals.len(), 7);
        assert_eq!(goals[0], Goal::Float { priority: None });
        assert_eq!(
            goals[1],
            Goal::Melee {
                priority: Some(2),
                speed: 1.0,
                follow_unseen: false,
                reach: Some(1.5),
                interval: 20,
            }
        );
        assert!(matches!(&goals[2], Goal::Ranged { interval: 40, range, .. } if *range == 10.0));
        assert!(matches!(
            goals[3],
            Goal::Wander {
                interval: 120,
                avoid_water: false,
                ..
            }
        ));
        assert_eq!(
            e.targets.as_ref().unwrap()[1],
            Target::NearestAttackable {
                priority: None,
                entities: vec!["minecraft:player".into()],
                must_see: false,
                interval: 10,
            }
        );
        // The declaration round-trips through the content root's file.
        let json = serde_json::to_string(goals).unwrap();
        assert_eq!(&serde_json::from_str::<Vec<Goal>>(&json).unwrap(), goals);
    }

    #[test]
    fn broken_entities_are_refused() {
        let decl = "data/ruby/horizon/entity/ghost.json";
        let refused = |name: &str, json: &str, extra: &[(&str, &[u8])]| {
            let mut files: Vec<(&str, &[u8])> = vec![(decl, json.as_bytes())];
            files.extend_from_slice(extra);
            let e = hmod(name, MANIFEST_TEXT, &files).unwrap_err();
            assert!(e.contains("ghost.json"), "{name}: {e}");
            e
        };
        refused("e-template", r#"{"template": "ruby:zombie"}"#, &[]);
        refused("e-key", r#"{"template": "minecraft:zombie", "hp": 3}"#, &[]);
        refused(
            "e-health",
            r#"{"template": "minecraft:zombie", "health": -1}"#,
            &[],
        );
        refused(
            "e-category",
            r#"{"template": "minecraft:zombie", "category": "boss"}"#,
            &[],
        );
        let e = refused(
            "e-texture",
            r#"{"template": "minecraft:zombie", "texture": "nope"}"#,
            &[],
        );
        assert!(e.contains("assets/ruby/textures/entity/nope.png"), "{e}");
        refused(
            "e-other",
            r#"{"template": "minecraft:zombie", "texture": "gems:nope"}"#,
            &[],
        );
        refused(
            "e-sound",
            r#"{"template": "minecraft:zombie", "sounds": {"roar": "a"}}"#,
            &[],
        );
        refused(
            "e-spawn",
            r#"{"template": "minecraft:zombie", "spawns": [{"biomes": ["minecraft:plains"], "min_count": 3, "max_count": 2}]}"#,
            &[],
        );
        refused(
            "e-model",
            r#"{"template": "minecraft:zombie"}"#,
            &[("assets/ruby/models/entity/ghost.geo.json", b"{}")],
        );
        refused(
            "e-anim",
            r#"{"template": "minecraft:zombie"}"#,
            &[
                ("assets/ruby/models/entity/ghost.geo.json", GHOST_GEO),
                (
                    "assets/ruby/animations/entity/ghost.animation.json",
                    b"{\"animations\": {\"a\": {\"loop\": 2}}}",
                ),
            ],
        );
        refused(
            "e-anim-alone",
            r#"{"template": "minecraft:zombie", "animations": "ghost"}"#,
            &[(
                "assets/ruby/animations/entity/ghost.animation.json",
                GHOST_ANIM,
            )],
        );
        let e = refused(
            "e-goal",
            r#"{"template": "minecraft:zombie", "goals": [{"goal": "dance"}]}"#,
            &[],
        );
        assert!(e.contains("dance"), "{e}");
        let e = refused(
            "e-goal-key",
            r#"{"template": "minecraft:zombie", "goals": [{"goal": "melee", "sped": 1}]}"#,
            &[],
        );
        assert!(e.contains("sped"), "{e}");
        let e = refused(
            "e-goal-brain",
            r#"{"template": "minecraft:warden", "goals": [{"goal": "float"}]}"#,
            &[],
        );
        assert!(e.contains("brain"), "{e}");
        refused(
            "e-goal-projectile",
            r#"{"template": "minecraft:zombie", "goals": [{"goal": "ranged", "projectile": "egg"}]}"#,
            &[],
        );
        refused(
            "e-goal-speed",
            r#"{"template": "minecraft:zombie", "goals": [{"goal": "wander", "speed": 0}]}"#,
            &[],
        );
        refused(
            "e-target-entity",
            r#"{"template": "minecraft:zombie", "targets": [{"target": "nearest_attackable", "entities": ["player"]}]}"#,
            &[],
        );
        refused(
            "e-knockback",
            r#"{"template": "minecraft:zombie", "attack_knockback": 9}"#,
            &[],
        );
        let e = refused(
            "e-component-name",
            r#"{"template": "minecraft:zombie", "components": {"mana": 1}}"#,
            &[],
        );
        assert!(e.contains("mymod:mana"), "{e}");
        refused(
            "e-component-null",
            r#"{"template": "minecraft:zombie", "components": {"ruby:mana": null}}"#,
            &[],
        );
        let e = refused(
            "e-component-list",
            r#"{"template": "minecraft:zombie", "components": {"ruby:bag": [1, "two"]}}"#,
            &[],
        );
        assert!(e.contains("one kind"), "{e}");
        refused(
            "e-spawn-egg-model",
            r#"{"template": "minecraft:zombie", "spawn_egg": {"item_model": "Nope"}}"#,
            &[],
        );
        refused(
            "e-carry-height",
            r#"{"template": "minecraft:zombie", "carry_height": -1}"#,
            &[],
        );
        let e = hmod(
            "e-spawn-egg-dup",
            MANIFEST_TEXT,
            &[
                (
                    "data/ruby/horizon/entity/ghost.json",
                    br#"{"template": "minecraft:zombie", "spawn_egg": {}}"#,
                ),
                (
                    "data/ruby/horizon/item/ghost_spawn_egg.json",
                    br#"{"template": "minecraft:zombie_spawn_egg"}"#,
                ),
            ],
        )
        .unwrap_err();
        assert!(e.contains("ghost_spawn_egg"), "{e}");
        let nested = hmod(
            "e-nested",
            MANIFEST_TEXT,
            &[(
                "data/ruby/horizon/entity/a/b.json",
                br#"{"template": "minecraft:zombie"}"#,
            )],
        )
        .unwrap_err();
        assert!(nested.contains("one file name"), "{nested}");
    }

    #[test]
    fn content_only_mod_round_trips() {
        let files: &[(&str, &[u8])] = &[
            ("data/ruby/horizon/block/ruby_block.json", BLOCK),
            ("data/ruby/horizon/item/ruby.json", ITEM),
            ("data/ruby/recipe/ruby_block.json", b"{}"),
            ("data/minecraft/tags/block/mineable/pickaxe.json", b"{}"),
            ("data/c/tags/item/gems/ruby.json", b"{}"),
            ("assets/ruby/textures/block/ruby_block.png", b"png"),
        ];
        let content = hmod("ok", MANIFEST_TEXT, files).unwrap();
        assert_eq!(content.files.len(), 6);
        assert_eq!(
            content.blocks,
            [Block {
                id: "ruby:ruby_block".into(),
                template: "minecraft:iron_block".into(),
                shapes: None,
                default_state: BTreeMap::new(),
                properties: BlockProperties {
                    hardness: Some(4.0),
                    light: Some(3),
                    sounds: Some("minecraft:amethyst_block".into()),
                    ..BlockProperties::default()
                },
                item: true,
            }]
        );
        assert_eq!(content.items[0].id, "ruby:ruby");
        assert_eq!(content.items[0].rarity, Some(Rarity::Uncommon));
        assert!(content.items[0].components.is_empty());
    }

    #[test]
    fn items_patch_their_template_components() {
        let sword = br#"{"template": "minecraft:iron_sword", "components": {
            "minecraft:max_damage": 900,
            "minecraft:attribute_modifiers": [{"type": "minecraft:attack_damage",
                "id": "minecraft:base_attack_damage", "amount": 7.0,
                "operation": "add_value", "slot": "mainhand"}],
            "!minecraft:repairable": {}}}"#;
        let files: &[(&str, &[u8])] = &[("data/ruby/horizon/item/ruby_sword.json", sword)];
        let content = hmod("components", MANIFEST_TEXT, files).unwrap();
        let components = &content.items[0].components;
        assert_eq!(
            components.keys().collect::<Vec<_>>(),
            [
                "!minecraft:repairable",
                "minecraft:attribute_modifiers",
                "minecraft:max_damage"
            ]
        );
        assert_eq!(components["minecraft:max_damage"], 900);
    }

    #[test]
    fn folder_mod_reads_the_same() {
        let dir = scratch("folder");
        std::fs::write(dir.join(MANIFEST), MANIFEST_TEXT).unwrap();
        let block = dir.join("data/ruby/horizon/block");
        std::fs::create_dir_all(&block).unwrap();
        std::fs::write(block.join("ruby_block.json"), BLOCK).unwrap();
        let package = Package::read(&dir).unwrap();
        assert!(package.wasm.is_none());
        let content = Content::read(&dir, &package.manifest).unwrap();
        assert_eq!(content.files, ["data/ruby/horizon/block/ruby_block.json"]);
        assert_eq!(content.file(&content.files[0]).unwrap(), BLOCK);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_content_outside_its_fence() {
        let refused = |name: &str, files: &[(&str, &[u8])]| {
            let e = hmod(name, MANIFEST_TEXT, files).unwrap_err();
            assert!(!e.is_empty());
            e
        };
        let e = refused("other-ns", &[("data/other/horizon/block/a.json", BLOCK)]);
        assert!(e.contains("own namespaces"), "{e}");
        refused(
            "vanilla-asset",
            &[("assets/minecraft/textures/block/stone.png", b"x")],
        );
        refused(
            "vanilla-recipe",
            &[("data/minecraft/recipe/stone.json", b"{}")],
        );
        refused("convention-recipe", &[("data/c/recipe/stone.json", b"{}")]);
        refused("convention-asset", &[("assets/c/lang/en_us.json", b"{}")]);
        let e = refused(
            "modded-template",
            &[(
                "data/ruby/horizon/block/a.json",
                br#"{"template": "ruby:other"}"#,
            )],
        );
        assert!(e.contains("not a vanilla block"), "{e}");
        refused(
            "light",
            &[(
                "data/ruby/horizon/block/a.json",
                br#"{"template": "minecraft:stone", "properties": {"light": 16}}"#,
            )],
        );
        refused(
            "unknown-key",
            &[(
                "data/ruby/horizon/block/a.json",
                br#"{"template": "minecraft:stone", "glow": true}"#,
            )],
        );
        refused("shape-reversed", &[("data/ruby/horizon/block/a.json", br#"{"template":"minecraft:stone","shapes":{"property":"axis","values":{"x":[[1,0,0,0,1,1]]}}}"#)]);
        refused("shape-malformed", &[("data/ruby/horizon/block/a.json", br#"{"template":"minecraft:stone","shapes":{"property":"axis","values":{"x":[[0,0,0,1,1]]}}}"#)]);
        refused(
            "shape-no-values",
            &[(
                "data/ruby/horizon/block/a.json",
                br#"{"template":"minecraft:stone","shapes":{"property":"axis","values":{}}}"#,
            )],
        );
        let e = refused(
            "clash",
            &[
                ("data/ruby/horizon/block/a.json", BLOCK),
                ("data/ruby/horizon/item/a.json", ITEM),
            ],
        );
        assert!(e.contains("also a block's item"), "{e}");
        refused("other-kind", &[("data/ruby/horizon/entity/a.json", b"{}")]);
        refused(
            "stack",
            &[(
                "data/ruby/horizon/item/a.json",
                br#"{"template": "minecraft:stick", "max_stack_size": 100}"#,
            )],
        );
        for (case, components, why) in [
            (
                "modded-component",
                r#"{"ruby:mana": 3}"#,
                "not a vanilla type",
            ),
            (
                "bare-component",
                r#"{"max_damage": 3}"#,
                "not a vanilla type",
            ),
            (
                "own-key",
                r#"{"minecraft:rarity": "epic"}"#,
                "set rarity instead",
            ),
            (
                "own-removal",
                r#"{"!minecraft:max_stack_size": {}}"#,
                "set max_stack_size",
            ),
            (
                "removal-value",
                r#"{"!minecraft:food": true}"#,
                "a removal's value is {}",
            ),
        ] {
            let item = format!(r#"{{"template": "minecraft:stick", "components": {components}}}"#);
            let e = refused(case, &[("data/ruby/horizon/item/a.json", item.as_bytes())]);
            assert!(e.contains(why), "{case}: {e}");
        }
    }

    #[test]
    fn listed_namespaces_hold_content() {
        let manifest = format!("{MANIFEST_TEXT}namespaces = [\"kaisyn\"]\n");
        let files: &[(&str, &[u8])] = &[
            ("data/kaisyn/worldgen/template_pool/a.json", b"{}"),
            ("data/kaisyn/structure/a.nbt", b"nbt"),
            ("assets/kaisyn/lang/en_us.json", b"{}"),
        ];
        let content = hmod("listed", &manifest, files).unwrap();
        assert_eq!(content.files.len(), 3);
        assert_eq!(content.namespaces, ["ruby", "kaisyn"]);
        // Declarations stay in the id's namespace.
        let decl: &[(&str, &[u8])] = &[("data/kaisyn/horizon/block/a.json", BLOCK)];
        let e = hmod("listed-decl", &manifest, decl).unwrap_err();
        assert!(e.contains("declared in data/ruby/horizon/"), "{e}");
        // A namespace it does not list is still outside.
        let other: &[(&str, &[u8])] = &[("data/other/structure/a.nbt", b"nbt")];
        assert!(hmod("listed-other", &manifest, other).is_err());
    }

    #[test]
    fn mods_sharing_a_namespace_clash() {
        let t_and_t = ["t_and_t".to_owned(), "kaisyn".to_owned()];
        let kaisyn = ["kaisyn".to_owned()];
        let ruby = ["ruby".to_owned(), "gems".to_owned()];
        assert_eq!(
            clash(("t_and_t", &t_and_t), ("kaisyn", &kaisyn)).as_deref(),
            Some("mods t_and_t and kaisyn both use the namespace kaisyn; turn one of them off")
        );
        assert_eq!(clash(("t_and_t", &t_and_t), ("ruby", &ruby)), None);
    }

    #[test]
    fn content_needs_the_registries_capability() {
        let code_only = MANIFEST_TEXT
            .replace("api = \"0.2\"", "api = \"0.1\"")
            .replace("capabilities = [\"registries\"]", "capabilities = []");
        // No code and no registries: nothing to load.
        let e = hmod("empty", &code_only, &[]).unwrap_err();
        assert!(e.contains("mod.wasm: missing"), "{e}");
        // Registries on API 0.1.
        let old = MANIFEST_TEXT.replace("api = \"0.2\"", "api = \"0.1\"");
        assert!(Manifest::parse(&old).unwrap_err().contains("needs api"));
        // Content without the capability, in a mod with code.
        let dir = scratch("no-cap");
        let path = dir.join("m.hmod");
        let files: &[(&str, &[u8])] = &[("assets/ruby/lang/en_us.json", b"{}")];
        std::fs::write(
            &path,
            Package::write_files(&code_only, Some(b"\0asm"), files).unwrap(),
        )
        .unwrap();
        let package = Package::read(&path).unwrap();
        let e = Content::read(&path, &package.manifest).err().unwrap();
        assert!(e.contains("not capability registries"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn paths_are_plain() {
        assert!(check_path("assets/ruby/textures/block/ruby_block.png").is_ok());
        assert!(check_path("data/ruby/loot_table/blocks/a.json").is_ok());
        for bad in [
            "assets/ruby",
            "assets/../x/y",
            "assets/ruby//a",
            "assets/Ruby/a",
            "assets/ruby/a b",
            "/assets/ruby/a",
            "other/ruby/a",
            "assets\\ruby\\a",
        ] {
            assert!(check_path(bad).is_err(), "{bad}");
        }
    }
}
