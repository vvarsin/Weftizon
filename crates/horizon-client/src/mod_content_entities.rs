//! The entities content mods declare, put in the content root
//! (docs/mods.md#custom-entities): a row per entity in `entity-types.json`
//! (its template's, with the declared size and attributes), the
//! declarations for [`crate::mobs::custom`], glow layers made from Weft's
//! `_e` masks, the sound events of sound files found by name, and the
//! natural spawns added to the biomes they name.
use super::{ContentMod, Overlay, out_path};
use crate::{Result, mobs::custom::Entry};
use horizon_mods::entity::Entity;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

/// The glow layer made from a skin's mask: next to the skin, under this suffix.
const GLOW: &str = "_horizon_glow";

/// Writes the mods' entities into the root; returns warnings for the log.
pub(super) fn build(
    base: &Path,
    mods: &[ContentMod],
    overlay: &Overlay<'_>,
) -> Result<Vec<String>> {
    let mut sorted: Vec<&ContentMod> = mods
        .iter()
        .filter(|m| !m.content.entities.is_empty())
        .collect();
    if sorted.is_empty() {
        return Ok(Vec::new());
    }
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut warnings = Vec::new();
    let types_text = fs::read_to_string(base.join("entity-types.json"))?;
    let types: serde_json::Map<String, Value> = serde_json::from_str(&types_text)?;
    let mut rows = Vec::new();
    let mut entries = Vec::new();
    let mut sound_events: BTreeMap<&str, Vec<(String, Vec<String>)>> = BTreeMap::new();
    let mut spawns: Vec<(&Entity, String)> = Vec::new();
    for m in &sorted {
        let mut entities: Vec<&Entity> = m.content.entities.iter().collect();
        entities.sort_by(|a, b| a.id.cmp(&b.id));
        let mut reader = m.content.reader()?;
        for e in entities {
            let path = e.template.trim_start_matches("minecraft:");
            if !crate::mobs::species::ALL.iter().any(|d| d.name == path) {
                return Err(
                    format!("{}: {}: template {} is not a mob", m.id, e.id, e.template).into(),
                );
            }
            let Some(Value::Object(template)) = types.get(&e.template) else {
                return Err(
                    format!("{}: {}: template {} is not a mob", m.id, e.id, e.template).into(),
                );
            };
            rows.push((e.id.clone(), row(template.clone(), e)));
            let category = e
                .stats
                .category
                .clone()
                .or_else(|| template.get("category")?.as_str().map(str::to_owned))
                .unwrap_or_else(|| "creature".into());
            if !e.spawns.is_empty() {
                spawns.push((e, category));
            }
            let mut textures: Vec<String> = Vec::new();
            let mut glows = Vec::new();
            for t in e.texture.iter().chain(&e.variants) {
                textures.push(t.atlas.clone());
                let glow = match (e.glow_masks.get(&t.atlas), &t.file) {
                    (Some(mask), Some(file)) => {
                        let skin = reader.read(file)?;
                        let mask_bytes = reader.read(mask)?;
                        match glow_layer(&skin, &mask_bytes) {
                            Ok(png) => {
                                let out = format!("{}{GLOW}.png", file.trim_end_matches(".png"));
                                overlay.write(&out, &png)?;
                                Some(format!("{}{GLOW}", t.atlas))
                            }
                            Err(why) => {
                                warnings.push(format!("{}: {mask}: {why}; it doesn't glow", e.id));
                                None
                            }
                        }
                    }
                    _ => None,
                };
                glows.push(glow);
            }
            let (ns, name) = e.id.split_once(':').unwrap_or((&m.id, &e.id));
            for (event, files) in &e.sound_files {
                sound_events
                    .entry(
                        m.content
                            .namespaces
                            .iter()
                            .find(|n| *n == ns)
                            .map_or(&m.id, |n| n),
                    )
                    .or_default()
                    .push((format!("entity.{name}.{event}"), files.clone()));
            }
            // EntityType.getDefaultLootTable when the mod has that table, else the template's.
            let own_table = format!("data/{ns}/loot_table/entities/{name}.json");
            let loot_table = e.loot_table.clone().or_else(|| {
                if m.content.files.binary_search(&own_table).is_ok() {
                    Some(format!("{ns}:entities/{name}"))
                } else {
                    Some(format!("minecraft:entities/{path}"))
                }
            });
            entries.push(Entry {
                id: e.id.clone(),
                template: e.template.clone(),
                textures,
                glows,
                emissive: e.emissive.as_ref().map(|t| t.atlas.clone()),
                model: e.model.clone(),
                animations: e.animations.clone(),
                scale: e.scale.unwrap_or(1.0),
                sounds: e.sounds.clone(),
                loot_table,
                goals: e.goals.clone(),
                targets: e.targets.clone(),
                components: e.components.clone(),
                solid: e.solid,
                kinematic: e.kinematic,
                carry_height: e.carry_height,
            });
        }
    }
    overlay.write(
        crate::mobs::custom::FILE,
        &serde_json::to_vec_pretty(&entries)?,
    )?;
    // The vanilla rows stay as they are, byte for byte: their attributes are
    // read exactly (`mobs::exact_attributes`).
    let end = types_text
        .rfind('}')
        .ok_or("entity-types.json is not an object")?;
    let mut text = types_text[..end].trim_end().to_owned();
    for (id, row) in &rows {
        text.push_str(&format!(
            ",{}:{}",
            serde_json::to_string(id)?,
            serde_json::to_string(row)?
        ));
    }
    text.push('}');
    overlay.write("entity-types.json", text.as_bytes())?;
    for (namespace, events) in sound_events {
        let mod_file = format!("assets/{namespace}/sounds.json");
        let owner = sorted
            .iter()
            .find(|m| m.content.namespaces.iter().any(|n| n == namespace));
        let mut doc = match owner.filter(|m| m.content.files.binary_search(&mod_file).is_ok()) {
            Some(m) => serde_json::from_slice::<Value>(&m.content.file(&mod_file)?)
                .map_err(|e| format!("{mod_file}: {e}"))?,
            None => Value::Object(Default::default()),
        };
        let Value::Object(map) = &mut doc else {
            return Err(format!("{mod_file} is not an object").into());
        };
        // The mod's own entries for an event win.
        for (event, files) in events {
            map.entry(event)
                .or_insert_with(|| serde_json::json!({ "sounds": files }));
        }
        overlay.write(&mod_file, &serde_json::to_vec_pretty(&doc)?)?;
    }
    warnings.extend(add_spawns(base, overlay, &spawns)?);
    Ok(warnings)
}

/// The template's `entity-types.json` row with the declaration applied.
fn row(mut t: serde_json::Map<String, Value>, e: &Entity) -> serde_json::Map<String, Value> {
    let s = &e.stats;
    // A box taller than the template's keeps the eyes at the same share of it.
    let eye_height = s.eye_height.or_else(|| {
        let (h, old_h) = (s.height?, t.get("height")?.as_f64()?);
        Some(t.get("eye_height")?.as_f64()? * h / old_h)
    });
    for (key, v) in [
        ("width", s.width),
        ("height", s.height),
        ("eye_height", eye_height),
    ] {
        if let Some(v) = v {
            t.insert(key.into(), v.into());
        }
    }
    if let Some(c) = &s.category {
        t.insert("category".into(), c.clone().into());
    }
    if let Some(f) = s.fire_immune {
        t.insert("fire_immune".into(), f.into());
    }
    if let Some(Value::Object(attributes)) = t.get_mut("attributes") {
        for (key, v) in [
            ("max_health", s.health),
            ("movement_speed", s.speed),
            ("attack_damage", s.attack_damage),
            ("attack_knockback", s.attack_knockback),
            ("armor", s.armor),
            ("follow_range", s.follow_range),
            ("knockback_resistance", s.knockback_resistance),
        ] {
            if let Some(v) = v {
                attributes.insert(format!("minecraft:{key}"), v.into());
            }
        }
    }
    t
}

/// Weft's glow mask (`<skin>_e.png`: red × alpha is how much a pixel glows)
/// as a full-bright layer: the skin's colours, as opaque as they glow.
fn glow_layer(skin: &[u8], mask: &[u8]) -> std::result::Result<Vec<u8>, String> {
    let (w, h, mut pixels) =
        crate::hud::decode_bytes(skin).map_err(|e| format!("the skin: {e}"))?;
    let (mw, mh, mask) = crate::hud::decode_bytes(mask).map_err(|e| e.to_string())?;
    if (mw, mh) != (w, h) {
        return Err(format!("{mw}x{mh}, not the skin's {w}x{h}"));
    }
    for (p, m) in pixels
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(mask.as_chunks::<4>().0)
    {
        let strength = u32::from(m[0]) * u32::from(m[3]) / 255;
        p[3] = (u32::from(p[3]) * strength / 255) as u8;
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// The biome ids a spawn names: ids as they are, `#tags` from the
/// worldgen loader's tag bundle (mods' tag additions merged in already).
fn biomes(names: &[String], tags: &serde_json::Map<String, Value>) -> BTreeSet<String> {
    fn expand(
        name: &str,
        tags: &serde_json::Map<String, Value>,
        out: &mut BTreeSet<String>,
        depth: usize,
    ) {
        let Some(tag) = name.strip_prefix('#') else {
            out.insert(name.to_owned());
            return;
        };
        if depth > 32 {
            return;
        }
        let doc: Option<Value> = tags
            .get(tag)
            .and_then(Value::as_str)
            .and_then(|t| serde_json::from_str(t).ok());
        for v in doc
            .as_ref()
            .and_then(|d| d["values"].as_array())
            .into_iter()
            .flatten()
        {
            let id = v.as_str().or_else(|| v["id"].as_str());
            if let Some(id) = id {
                expand(id, tags, out, depth + 1);
            }
        }
    }
    let mut out = BTreeSet::new();
    for n in names {
        expand(n, tags, &mut out, 0);
    }
    out
}

/// `natural_mob_spawns` of each named biome, with the entities added to
/// their categories' lists.
fn add_spawns(
    base: &Path,
    overlay: &Overlay<'_>,
    spawns: &[(&Entity, String)],
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    if spawns.is_empty() {
        return Ok(warnings);
    }
    let tags: serde_json::Map<String, Value> = match fs::read(out_path(overlay, super::BIOME_TAGS))
    {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(_) => Default::default(),
    };
    let mut changed: BTreeMap<String, Value> = BTreeMap::new();
    for (e, category) in spawns {
        for spawn in &e.spawns {
            for biome in biomes(&spawn.biomes, &tags) {
                let Some(path) = biome.strip_prefix("minecraft:") else {
                    warnings.push(format!(
                        "{}: spawns in {biome}: only vanilla biomes have spawn lists yet",
                        e.id
                    ));
                    continue;
                };
                let file = format!("data/minecraft/worldgen/biome/{path}.json");
                if !changed.contains_key(&file) {
                    let Ok(bytes) = fs::read(base.join(&file)) else {
                        warnings.push(format!("{}: spawns in {biome}, which is no biome", e.id));
                        continue;
                    };
                    changed.insert(file.clone(), serde_json::from_slice(&bytes)?);
                }
                let doc = changed.get_mut(&file).ok_or("biome")?;
                let natural = &mut doc["attributes"]["minecraft:gameplay/natural_mob_spawns"];
                // A modifier ({argument, modifier: overlay}) or the plain value.
                let value = if natural["argument"].is_object() {
                    &mut natural["argument"]
                } else {
                    if natural.is_null() {
                        *natural =
                            serde_json::json!({ "spawn_costs": {}, "spawns_by_category": {} });
                    }
                    natural
                };
                let list = &mut value["spawns_by_category"][category.as_str()];
                if list.is_null() {
                    *list = Value::Array(Vec::new());
                }
                let count = if spawn.min_count == spawn.max_count {
                    Value::from(spawn.min_count)
                } else {
                    serde_json::json!({ "type": "minecraft:uniform",
                        "min_inclusive": spawn.min_count, "max_inclusive": spawn.max_count })
                };
                if let Value::Array(list) = list {
                    list.push(
                        serde_json::json!({ "type": e.id, "count": count, "weight": spawn.weight }),
                    );
                }
            }
        }
    }
    for (file, doc) in &changed {
        overlay.write(file, &serde_json::to_vec(doc)?)?;
    }
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glow_masks_make_the_glowing_pixels_opaque() {
        let png = |pixels: &[u8]| {
            let mut out = Vec::new();
            let mut e = png::Encoder::new(&mut out, 2, 1);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().unwrap();
            w.write_image_data(pixels).unwrap();
            w.finish().unwrap();
            out
        };
        let skin = png(&[10, 20, 30, 255, 40, 50, 60, 255]);
        let mask = png(&[255, 0, 0, 255, 0, 0, 0, 255]);
        let glow = glow_layer(&skin, &mask).unwrap();
        let (w, h, pixels) = crate::hud::decode_bytes(&glow).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(pixels, [10, 20, 30, 255, 40, 50, 60, 0]);
        let small = png(&[0; 8]);
        let mut e = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut e, 1, 1);
            enc.set_color(png::ColorType::Rgba);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[0; 4]).unwrap();
        }
        assert!(
            glow_layer(&small, &e)
                .unwrap_err()
                .contains("not the skin's")
        );
    }

    #[test]
    fn spawn_biomes_expand_tags() {
        let tags: serde_json::Map<String, Value> = serde_json::from_str(
            r##"{"minecraft:is_forest": "{\"values\": [\"minecraft:forest\", \"#minecraft:is_birch\"]}",
                "minecraft:is_birch": "{\"values\": [{\"id\": \"minecraft:birch_forest\", \"required\": false}]}"}"##,
        )
        .unwrap();
        let got = biomes(
            &["#minecraft:is_forest".into(), "minecraft:plains".into()],
            &tags,
        );
        assert_eq!(
            got.into_iter().collect::<Vec<_>>(),
            [
                "minecraft:birch_forest",
                "minecraft:forest",
                "minecraft:plains"
            ]
        );
    }
}
