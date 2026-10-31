//! Immutable feature resource snapshots. Requested roots resolve named feature
//! and placed-feature holders and tags into bounded executable inline trees.
//! This is not a complete datapack registry loader or live-reload implementation.
use super::{
    DecodeError, Object,
    block_feature::FeatureBlockBindings,
    density::identifier,
    feature_graph::{FeatureDefinition, PlacedFeatureDefinition},
    required,
    tags::TagDefinitions,
};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

pub struct FeatureCatalog {
    features: BTreeMap<String, Box<RawValue>>,
    placed: BTreeMap<String, Box<RawValue>>,
    tags: BTreeMap<String, Vec<String>>,
    empty_tags: BTreeSet<String>,
}
struct Expansion {
    remaining: usize,
    active: Vec<(bool, String)>,
}
impl Expansion {
    fn visit(&mut self, depth: usize) -> Result<(), DecodeError> {
        if depth > 128 || self.remaining == 0 {
            return Err(DecodeError::Field("feature resource expansion limit"));
        }
        self.remaining -= 1;
        Ok(())
    }
}
fn resources(
    source: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Box<RawValue>>, DecodeError> {
    if source.len() > 8192 {
        return Err(DecodeError::Field("feature resource count"));
    }
    let mut result = BTreeMap::new();
    for (name, json) in source {
        if result
            .insert(identifier(name)?, serde_json::from_str(json)?)
            .is_some()
        {
            return Err(DecodeError::Field("duplicate feature resource"));
        }
    }
    Ok(result)
}
fn raw(json: String) -> Result<Box<RawValue>, DecodeError> {
    Ok(RawValue::from_string(json)?)
}
/// 1.20.1 vanilla features that 26.3 removed, as 1.20.1's data writes them, so a
/// pack of that version can still name them. Only read when no pack defines the
/// name. `seagrass_simple` waits on the liquid carving mask, which is empty
/// (see `PlacementModifier::LegacyLiquidCarvingMask`).
fn legacy_feature(placed: bool, name: &str) -> Option<&'static str> {
    Some(match (placed, name) {
        (false, "minecraft:seagrass_simple") => {
            r#"{"type":"minecraft:simple_block","config":{"to_place":{"type":"minecraft:simple_state_provider","state":{"Name":"minecraft:seagrass"}}}}"#
        }
        (true, "minecraft:seagrass_simple") => {
            r#"{"feature":"minecraft:seagrass_simple","placement":[{"type":"minecraft:carving_mask","step":"liquid"},{"type":"minecraft:rarity_filter","chance":10},{"type":"minecraft:block_predicate_filter","predicate":{"type":"minecraft:all_of","predicates":[{"type":"minecraft:matching_blocks","blocks":"minecraft:stone","offset":[0,-1,0]},{"type":"minecraft:matching_blocks","blocks":"minecraft:water"},{"type":"minecraft:matching_blocks","blocks":"minecraft:water","offset":[0,1,0]}]}},{"type":"minecraft:biome"}]}"#
        }
        _ => return None,
    })
}
fn entries(value: &RawValue) -> Result<Vec<Box<RawValue>>, DecodeError> {
    let values: Vec<Box<RawValue>> = serde_json::from_str(value.get())?;
    if values.len() > 8192 {
        return Err(DecodeError::Field("feature resource list"));
    }
    Ok(values)
}
impl FeatureCatalog {
    /// Stores source JSON without validating unused feature algorithms. Resolving
    /// a root validates its entire reachable tree, including unselected children.
    pub fn from_resources(
        features: &BTreeMap<String, String>,
        placed: &BTreeMap<String, String>,
        tags: &TagDefinitions,
    ) -> Result<Self, DecodeError> {
        let features = resources(features)?;
        let placed = resources(placed)?;
        let built = tags
            .build(|name, _| placed.contains_key(name).then(|| name.to_owned()))
            .map_err(|e| DecodeError::Registry(e.to_string()))?;
        Ok(Self {
            features,
            placed,
            empty_tags: built
                .tags
                .iter()
                .filter_map(|(name, values)| values.is_empty().then_some(name.clone()))
                .collect(),
            tags: built.tags,
        })
    }
    pub fn feature(&self, json: &str) -> Result<FeatureDefinition, DecodeError> {
        self.feature_with_optional_blocks(json, None)
    }
    pub fn feature_with_blocks(
        &self,
        json: &str,
        blocks: &FeatureBlockBindings<'_>,
    ) -> Result<FeatureDefinition, DecodeError> {
        self.feature_with_optional_blocks(json, Some(blocks))
    }
    fn feature_with_optional_blocks(
        &self,
        json: &str,
        blocks: Option<&FeatureBlockBindings<'_>>,
    ) -> Result<FeatureDefinition, DecodeError> {
        let value: Box<RawValue> = serde_json::from_str(json)?;
        let expanded = self.expand_feature(
            &value,
            0,
            &mut Expansion {
                remaining: 65536,
                active: vec![],
            },
        )?;
        FeatureDefinition::from_catalog_json(&expanded, &self.empty_tags, blocks)
    }
    pub fn placed_feature(&self, json: &str) -> Result<PlacedFeatureDefinition, DecodeError> {
        self.placed_with_optional_blocks(json, None)
    }
    pub fn placed_feature_with_blocks(
        &self,
        json: &str,
        blocks: &FeatureBlockBindings<'_>,
    ) -> Result<PlacedFeatureDefinition, DecodeError> {
        self.placed_with_optional_blocks(json, Some(blocks))
    }
    fn placed_with_optional_blocks(
        &self,
        json: &str,
        blocks: Option<&FeatureBlockBindings<'_>>,
    ) -> Result<PlacedFeatureDefinition, DecodeError> {
        let value: Box<RawValue> = serde_json::from_str(json)?;
        let expanded = self.expand_placed(
            &value,
            0,
            &mut Expansion {
                remaining: 65536,
                active: vec![],
            },
        )?;
        PlacedFeatureDefinition::from_catalog_json(&expanded, &self.empty_tags, blocks)
    }
    fn named(
        &self,
        name: &str,
        placed: bool,
        depth: usize,
        work: &mut Expansion,
    ) -> Result<String, DecodeError> {
        let name = identifier(name)?;
        let key = (placed, name.clone());
        if work.active.contains(&key) {
            return Err(DecodeError::Registry(format!(
                "cyclic feature reference {name}"
            )));
        }
        let table = if placed { &self.placed } else { &self.features };
        let legacy;
        let value = match table.get(&name) {
            Some(value) => value,
            None => {
                legacy = raw(legacy_feature(placed, &name)
                    .ok_or_else(|| {
                        DecodeError::Registry(format!("unknown feature reference {name}"))
                    })?
                    .to_owned())?;
                &legacy
            }
        };
        work.active.push(key);
        let result = if placed {
            self.expand_placed(value, depth + 1, work)
        } else {
            self.expand_feature(value, depth + 1, work)
        };
        work.active.pop();
        result
    }
    fn expand_placed(
        &self,
        value: &RawValue,
        depth: usize,
        work: &mut Expansion,
    ) -> Result<String, DecodeError> {
        work.visit(depth)?;
        if let Ok(name) = serde_json::from_str::<String>(value.get()) {
            return self.named(&name, true, depth, work);
        }
        let mut object: Object = serde_json::from_str(value.get())?;
        let feature = self.expand_feature(required(&object, "feature")?, depth + 1, work)?;
        object.insert("feature".into(), raw(feature)?);
        Ok(serde_json::to_string(&object)?)
    }
    fn expand_set(
        &self,
        value: &RawValue,
        depth: usize,
        work: &mut Expansion,
    ) -> Result<String, DecodeError> {
        let values = if let Ok(name) = serde_json::from_str::<String>(value.get()) {
            if let Some(tag) = name.strip_prefix('#') {
                let tag = identifier(tag)?;
                let names = self.tags.get(&tag).ok_or_else(|| {
                    DecodeError::Registry(format!("unknown placed-feature tag {tag}"))
                })?;
                // Native nonEmptyHolderSet validates direct lists, but permits
                // a named tag whose bound contents are empty.
                if names.is_empty() {
                    return Ok(serde_json::to_string(&format!("#{tag}"))?);
                }
                if names.len() > super::state_provider::MAX_TAG_BLOCKS {
                    return Err(DecodeError::Field("placed-feature tag size"));
                }
                names
                    .iter()
                    .map(|name| raw(serde_json::to_string(name)?))
                    .collect::<Result<Vec<_>, DecodeError>>()?
            } else {
                vec![value.to_owned()]
            }
        } else if value.get().trim_start().starts_with('[') {
            entries(value)?
        } else {
            vec![value.to_owned()]
        };
        let expanded = values
            .iter()
            .map(|v| raw(self.expand_placed(v, depth + 1, work)?))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::to_string(&expanded)?)
    }
    fn expand_feature(
        &self,
        value: &RawValue,
        depth: usize,
        work: &mut Expansion,
    ) -> Result<String, DecodeError> {
        work.visit(depth)?;
        if let Ok(name) = serde_json::from_str::<String>(value.get()) {
            return self.named(&name, false, depth, work);
        }
        let mut object: Object = serde_json::from_str(value.get())?;
        // 1.20.1 packs keep feature settings under `config`; the generator
        // reads the flattened form.
        if let Some(config) = object.remove("config") {
            let fields: Object = serde_json::from_str(config.get())?;
            for (key, value) in fields {
                object.insert(key, value);
            }
        }
        let kind = identifier(&serde_json::from_str::<String>(
            required(&object, "type")?.get(),
        )?)?;
        match kind.as_str() {
            "minecraft:single_block_pillar" => {
                if let Some(value) = super::optional(&object, "cap_feature") {
                    let expanded = self.expand_placed(value, depth + 1, work)?;
                    object.insert("cap_feature".into(), raw(expanded)?);
                }
            }
            "minecraft:vegetation_patch" | "minecraft:waterlogged_vegetation_patch" => {
                let expanded =
                    self.expand_placed(required(&object, "vegetation_feature")?, depth + 1, work)?;
                object.insert("vegetation_feature".into(), raw(expanded)?);
            }
            "minecraft:coral_tree"
            | "minecraft:coral_claw"
            | "minecraft:root_system"
            | "minecraft:random_patch" => {
                let expanded =
                    self.expand_placed(required(&object, "feature")?, depth + 1, work)?;
                object.insert("feature".into(), raw(expanded)?);
            }
            "minecraft:tree" => {
                // PaleMossDecorator places the registry's pale_moss_patch by key.
                let pale_moss = super::optional(&object, "decorators")
                    .map(|raw| serde_json::from_str::<Vec<Object>>(raw.get()))
                    .transpose()?
                    .unwrap_or_default()
                    .iter()
                    .any(|d| {
                        d.get("type").is_some_and(|t| {
                            serde_json::from_str::<String>(t.get())
                                .ok()
                                .and_then(|t| identifier(&t).ok())
                                .as_deref()
                                == Some("minecraft:pale_moss")
                        })
                    });
                const PATCH: &str = "minecraft:pale_moss_patch";
                if pale_moss && self.features.contains_key(PATCH) {
                    let expanded = self.named(PATCH, false, depth, work)?;
                    object.insert(super::tree::GROUND_FEATURE_KEY.into(), raw(expanded)?);
                }
            }
            "minecraft:sequence" | "minecraft:overlay" | "minecraft:simple_random_selector" => {
                let expanded = self.expand_set(required(&object, "features")?, depth + 1, work)?;
                object.insert("features".into(), raw(expanded)?);
            }
            "minecraft:random_boolean_selector" => {
                for name in ["feature_true", "feature_false"] {
                    let expanded = self.expand_placed(required(&object, name)?, depth + 1, work)?;
                    object.insert(name.into(), raw(expanded)?);
                }
            }
            "minecraft:random_selector" | "minecraft:weighted_random_selector" => {
                let key = if kind == "minecraft:random_selector" {
                    "feature"
                } else {
                    "data"
                };
                let values = entries(required(&object, "features")?)?;
                let mut expanded = Vec::with_capacity(values.len());
                for value in values {
                    let mut entry: Object = serde_json::from_str(value.get())?;
                    let child = self.expand_placed(required(&entry, key)?, depth + 1, work)?;
                    entry.insert(key.into(), raw(child)?);
                    expanded.push(entry);
                }
                object.insert("features".into(), raw(serde_json::to_string(&expanded)?)?);
                if kind == "minecraft:random_selector" {
                    let child =
                        self.expand_placed(required(&object, "default")?, depth + 1, work)?;
                    object.insert("default".into(), raw(child)?);
                }
            }
            _ => {}
        }
        Ok(serde_json::to_string(&object)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded(catalog: &FeatureCatalog, name: &str) -> serde_json::Value {
        let raw = raw(serde_json::to_string(name).unwrap()).unwrap();
        let mut work = Expansion {
            remaining: 65536,
            active: vec![],
        };
        serde_json::from_str(&catalog.expand_placed(&raw, 0, &mut work).unwrap()).unwrap()
    }

    #[test]
    fn removed_vanilla_features_resolve_to_their_1_20_1_definitions() {
        let tags = TagDefinitions::default();
        let empty =
            FeatureCatalog::from_resources(&BTreeMap::new(), &BTreeMap::new(), &tags).unwrap();
        let seagrass = expanded(&empty, "minecraft:seagrass_simple");
        assert_eq!(seagrass["feature"]["type"], "minecraft:simple_block");
        assert_eq!(
            seagrass["feature"]["to_place"]["state"]["Name"],
            "minecraft:seagrass"
        );
        assert_eq!(seagrass["placement"][0]["type"], "minecraft:carving_mask");
        assert_eq!(seagrass["placement"][0]["step"], "liquid");
        assert!(
            empty
                .placed_feature(r#""minecraft:seagrass_short""#)
                .is_err()
        );
        // A pack's own definition wins.
        let placed = BTreeMap::from([(
            "minecraft:seagrass_simple".to_owned(),
            r#"{"feature":{"type":"minecraft:no_op"},"placement":[]}"#.to_owned(),
        )]);
        let pack = FeatureCatalog::from_resources(&BTreeMap::new(), &placed, &tags).unwrap();
        let seagrass = expanded(&pack, "minecraft:seagrass_simple");
        assert_eq!(seagrass["feature"]["type"], "minecraft:no_op");
    }
}
