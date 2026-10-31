//! Initial block-state providers, bound to immutable block and tag identities.
mod catalog;
mod noise;
pub use catalog::StateProviderCatalog;
pub(crate) mod properties;
use super::{
    DecodeError, Object,
    block_predicate::{BlockPredicate, PredicateBlockResolver},
    density::identifier,
    feature::FeatureWorld,
    optional,
    placement::{PlacementContext, PlacementQueryError},
    required,
    value_provider::IntProvider,
    weighted::WeightedList,
};
use crate::block_properties::{BlockPropertiesRegistry, BlockProperty, BoundBlockProperty};
use crate::registry::{BlockRegistry, BlockStateId, BlockStateJsonCodec};
use horizon_core::random::{InvalidBound, RandomSource};
use noise::NoiseProvider;
use properties::StateProperties;
use serde_json::value::RawValue;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
};
const DIRECTIONS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
type Rotations = Arc<Vec<[BlockStateId; 6]>>;

pub struct StateProviderBindings<'r> {
    physical_properties: Option<[BoundBlockProperty; 4]>,
    biomes: Option<Arc<super::biome_predicate::BiomePredicateBindings>>,
    fluids: super::fluid_predicate::FluidPredicateBindings,
    codec: BlockStateJsonCodec<'r>,
    blocks: BTreeMap<String, BlockStateId>,
    tags: BTreeMap<String, Arc<Vec<BlockStateId>>>,
    registry: &'r BlockRegistry,
    rotations: OnceLock<Result<Rotations, String>>,
    integers: BTreeSet<(String, String)>,
    properties: OnceLock<Result<Arc<StateProperties>, String>>,
    predicate_states: OnceLock<BTreeMap<BlockStateId, Arc<BTreeSet<BlockStateId>>>>,
}
impl<'r> StateProviderBindings<'r> {
    pub fn registry(&self) -> &'r BlockRegistry {
        self.registry
    }
    pub fn with_block_properties(
        mut self,
        properties: &BlockPropertiesRegistry<'r>,
    ) -> Result<Self, DecodeError> {
        if !std::ptr::eq(self.registry, properties.registry()) {
            return Err(DecodeError::Field(
                "block properties use a different registry",
            ));
        }
        self.physical_properties = Some([
            properties.bind(BlockProperty::Solid),
            properties.bind(BlockProperty::Replaceable),
            properties.bind(BlockProperty::Liquid),
            properties.bind(BlockProperty::SolidRender),
        ]);
        Ok(self)
    }
    pub fn with_biomes(
        mut self,
        biomes: Arc<super::biome_predicate::BiomePredicateBindings>,
    ) -> Self {
        self.biomes = Some(biomes);
        self
    }

    pub fn with_fluid_tags(
        mut self,
        tags: &BTreeMap<String, Vec<String>>,
    ) -> Result<Self, DecodeError> {
        self.fluids = self.fluids.with_resolved_tags(tags)?;
        Ok(self)
    }
    /// Resolved tag lists preserve holder order, including an empty named set.
    pub fn new(
        registry: &'r BlockRegistry,
        integers: BTreeSet<(String, String)>,
        tags: &BTreeMap<String, Vec<String>>,
    ) -> Result<Self, DecodeError> {
        let codec = BlockStateJsonCodec::new(registry, integers.clone())
            .map_err(|e| DecodeError::Registry(e.to_string()))?;
        let mut blocks = BTreeMap::new();
        for index in 0..registry.len() {
            let name = &registry.state(BlockStateId(index as u32)).unwrap().name;
            blocks
                .entry(name.clone())
                .or_insert_with(|| registry.default_state(name).unwrap());
        }
        let mut bound_tags = BTreeMap::new();
        for (name, entries) in tags {
            if entries.len() > MAX_TAG_BLOCKS {
                return Err(DecodeError::Field("provider tag size"));
            }
            let values = entries
                .iter()
                .map(|name| {
                    blocks
                        .get(&identifier(name)?)
                        .copied()
                        .ok_or_else(|| DecodeError::Registry(format!("unknown block {name}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if bound_tags
                .insert(super::tags::identifier(name)?, Arc::new(values))
                .is_some()
            {
                return Err(DecodeError::Field("duplicate provider block tag"));
            }
        }
        Ok(Self {
            codec,
            biomes: None,
            physical_properties: None,
            fluids: super::fluid_predicate::FluidPredicateBindings::new(registry)?,
            blocks,
            tags: bound_tags,
            registry,
            rotations: OnceLock::new(),
            integers,
            properties: OnceLock::new(),
            predicate_states: OnceLock::new(),
        })
    }
    fn state(&self, json: &str, full: bool) -> Result<BlockStateId, DecodeError> {
        decode_state(&self.codec, json, full)
    }
    fn rotations(&self) -> Result<Rotations, DecodeError> {
        self.rotations
            .get_or_init(|| rotation_table(self.registry))
            .clone()
            .map_err(DecodeError::Registry)
    }
    pub(crate) fn properties(&self) -> Result<Arc<StateProperties>, DecodeError> {
        self.properties
            .get_or_init(|| StateProperties::build(self.registry, &self.integers).map(Arc::new))
            .clone()
            .map_err(DecodeError::Registry)
    }
    fn block_set(&self, raw: &RawValue) -> Result<Arc<Vec<BlockStateId>>, DecodeError> {
        if let Ok(name) = serde_json::from_str::<String>(raw.get()) {
            if let Some(tag) = name.strip_prefix('#') {
                return self
                    .tags
                    .get(&super::tags::identifier(tag)?)
                    .cloned()
                    .ok_or_else(|| {
                        DecodeError::Registry(format!("unknown provider block tag {tag}"))
                    });
            }
            let state = self
                .blocks
                .get(&identifier(&name)?)
                .copied()
                .ok_or_else(|| DecodeError::Registry(format!("unknown block {name}")))?;
            return Ok(Arc::new(vec![state]));
        }
        let names: Vec<String> = serde_json::from_str(raw.get())?;
        if names.len() > 8192 {
            return Err(DecodeError::Field("provider block list size"));
        }
        Ok(Arc::new(
            names
                .iter()
                .map(|name| {
                    self.blocks
                        .get(&identifier(name)?)
                        .copied()
                        .ok_or_else(|| DecodeError::Registry(format!("unknown block {name}")))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}
impl PredicateBlockResolver for StateProviderBindings<'_> {
    fn predicate_state(&self, json: &str) -> Result<BlockStateId, DecodeError> {
        self.state(json, false)
    }
    fn predicate_property(
        &self,
        property: BlockProperty,
    ) -> Result<BoundBlockProperty, DecodeError> {
        super::block_predicate::bound_property(self.physical_properties.as_ref(), property)
    }
    fn predicate_biome_set(
        &self,
        json: &str,
    ) -> Result<Arc<BTreeSet<super::biome::BiomeId>>, DecodeError> {
        self.biomes
            .as_ref()
            .ok_or(DecodeError::Field("predicate biome registry"))?
            .resolve(json)
    }

    fn predicate_fluid_set(
        &self,
        json: &str,
    ) -> Result<super::fluid_predicate::BoundFluidSet, DecodeError> {
        self.fluids.resolve(json)
    }
    fn predicate_block_set(&self, json: &str) -> Result<Arc<BTreeSet<BlockStateId>>, DecodeError> {
        let raw: Box<RawValue> = serde_json::from_str(json)?;
        let defaults = self.block_set(&raw)?;
        let states = self.predicate_states.get_or_init(|| {
            let mut states = BTreeMap::<BlockStateId, BTreeSet<BlockStateId>>::new();
            for id in 0..self.registry.len() {
                let id = BlockStateId(id as u32);
                let default = self.blocks[&self.registry.state(id).unwrap().name];
                states.entry(default).or_default().insert(id);
            }
            states
                .into_iter()
                .map(|(id, values)| (id, Arc::new(values)))
                .collect()
        });
        if defaults.len() == 1 {
            return Ok(states[&defaults[0]].clone());
        }
        Ok(Arc::new(
            defaults
                .iter()
                .flat_map(|id| states[id].iter().copied())
                .collect(),
        ))
    }
}
#[derive(Clone, Debug)]
pub struct BlockStateProvider(Node);
#[derive(Clone, Debug)]
enum Node {
    RuleBased {
        fallback: Option<Box<BlockStateProvider>>,
        rules: Vec<(BlockPredicate, BlockStateProvider)>,
    },
    Noise(Box<NoiseProvider>),
    Simple(BlockStateId),
    Weighted(WeightedList<BlockStateId>),
    RandomBlock(Arc<Vec<BlockStateId>>),
    Rotated {
        source: Box<BlockStateProvider>,
        direction: Option<usize>,
        rotations: Rotations,
    },
    CopyProperties(Box<BlockStateProvider>, Arc<StateProperties>),
    RandomizedInt {
        source: Box<BlockStateProvider>,
        property: String,
        values: IntProvider,
        properties: Arc<StateProperties>,
    },
}
#[derive(Debug)]
pub enum StateProviderError<E> {
    Random(InvalidBound),
    World(E),
    Query(PlacementQueryError),
    InvalidState,
    InvalidPropertyValue,
    EmptyNoiseSelection,
}
impl<E: std::fmt::Display> std::fmt::Display for StateProviderError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Random(e) => e.fmt(f),
            Self::World(e) => e.fmt(f),
            Self::Query(e) => e.fmt(f),
            Self::InvalidState => f.write_str("unregistered provider source state"),
            Self::InvalidPropertyValue => f.write_str("invalid randomized property value"),
            Self::EmptyNoiseSelection => f.write_str("empty dual-noise state selection"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for StateProviderError<E> {}
impl<E> From<InvalidBound> for StateProviderError<E> {
    fn from(e: InvalidBound) -> Self {
        Self::Random(e)
    }
}
impl BlockStateProvider {
    /// Native DIRECT_CODEC is an exclusive choice: exactly one of a full state
    /// object and a typed provider must decode. Compact state strings are not
    /// direct providers; named provider holders are a separate registry layer.
    pub fn from_json(
        json: &str,
        bindings: &StateProviderBindings<'_>,
    ) -> Result<Self, DecodeError> {
        Self::decode(json, bindings, 0, &mut 65536, None)
    }
    fn decode_holder(
        json: &str,
        bindings: &StateProviderBindings<'_>,
        depth: usize,
        remaining: &mut usize,
        catalog: Option<&StateProviderCatalog>,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field(
                "state provider resource expansion limit",
            ));
        }
        if let Ok(name) = serde_json::from_str::<String>(json) {
            let name = identifier(&name)?;
            let resource = catalog
                .and_then(|c| c.lookup(&name))
                .ok_or_else(|| DecodeError::Registry(format!("unknown state provider {name}")))?;
            *remaining -= 1;
            Self::decode(resource, bindings, depth + 1, remaining, catalog)
        } else {
            Self::decode(json, bindings, depth, remaining, catalog)
        }
    }
    fn decode(
        json: &str,
        bindings: &StateProviderBindings<'_>,
        depth: usize,
        remaining: &mut usize,
        catalog: Option<&StateProviderCatalog>,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("state provider depth"));
        }
        match (
            bindings.state(json, true),
            Self::decode_typed(json, bindings, depth, remaining, catalog),
        ) {
            (Ok(state), Err(_)) => Ok(Self(Node::Simple(state))),
            (Err(_), Ok(provider)) => Ok(provider),
            _ => Err(DecodeError::Field("exclusive state or provider")),
        }
    }
    fn decode_typed(
        json: &str,
        bindings: &StateProviderBindings<'_>,
        depth: usize,
        remaining: &mut usize,
        catalog: Option<&StateProviderCatalog>,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("state provider depth"));
        }
        *remaining -= 1;
        let object: Object = serde_json::from_str(json)?;
        let kind = if let Some(raw) = object.get("type") {
            identifier(&serde_json::from_str::<String>(raw.get())?)?
        } else if object.contains_key("fallback") && object.contains_key("rules") {
            "minecraft:rule_based".to_owned()
        } else {
            return Err(DecodeError::Field("block state provider type"));
        };
        // 1.20.1's BlockStateProviderType names, same fields as 26.3's.
        let kind = match kind.as_str() {
            "minecraft:noise_provider" => "minecraft:noise".to_owned(),
            "minecraft:noise_threshold_provider" => "minecraft:noise_threshold".to_owned(),
            "minecraft:dual_noise_provider" => "minecraft:dual_noise".to_owned(),
            "minecraft:rotated_block_provider" => "minecraft:rotated".to_owned(),
            "minecraft:randomized_int_state_provider" => "minecraft:randomized_int".to_owned(),
            _ => kind,
        };
        Ok(Self(match kind.as_str() {
            "minecraft:rule_based" | "minecraft:rule_based_state_provider" => {
                let fallback = optional(&object, "fallback")
                    .map(|raw| {
                        Self::decode_holder(raw.get(), bindings, depth + 1, remaining, catalog)
                            .map(Box::new)
                    })
                    .transpose()?;
                let raw_rules: Vec<Box<RawValue>> =
                    serde_json::from_str(required(&object, "rules")?.get())?;
                if raw_rules.len() > 8192 {
                    return Err(DecodeError::Field("provider rule count"));
                }
                let mut rules = Vec::with_capacity(raw_rules.len());
                for raw in raw_rules {
                    if *remaining == 0 {
                        return Err(DecodeError::Field("provider node count"));
                    }
                    *remaining -= 1;
                    let rule: Object = serde_json::from_str(raw.get())?;
                    let predicate = BlockPredicate::decode_with_blocks(
                        required(&rule, "if_true")?.get(),
                        depth + 1,
                        remaining,
                        Some(bindings),
                    )?;
                    let provider = Self::decode_holder(
                        required(&rule, "then")?.get(),
                        bindings,
                        depth + 1,
                        remaining,
                        catalog,
                    )?;
                    rules.push((predicate, provider));
                }
                Node::RuleBased { fallback, rules }
            }
            "minecraft:noise" | "minecraft:noise_threshold" | "minecraft:dual_noise" => {
                Node::Noise(Box::new(NoiseProvider::decode(&object, bindings, &kind)?))
            }
            "minecraft:simple" | "minecraft:simple_state_provider" => {
                Node::Simple(bindings.state(required(&object, "state")?.get(), false)?)
            }
            "minecraft:weighted" | "minecraft:weighted_state_provider" => {
                Node::Weighted(WeightedList::decode(required(&object, "entries")?, |v| {
                    bindings.state(v.get(), false)
                })?)
            }
            "minecraft:random_block" | "minecraft:random_block_state_provider" => {
                Node::RandomBlock(bindings.block_set(required(&object, "blocks")?)?)
            }
            "minecraft:rotated" => {
                let direction = optional(&object, "direction")
                    .map(|v| {
                        let name: String = serde_json::from_str(v.get())?;
                        DIRECTIONS
                            .iter()
                            .position(|v| *v == name)
                            .ok_or(DecodeError::Field("provider direction"))
                    })
                    .transpose()?;
                Node::Rotated {
                    source: Box::new(Self::decode_holder(
                        required(&object, "state")?.get(),
                        bindings,
                        depth + 1,
                        remaining,
                        catalog,
                    )?),
                    direction,
                    rotations: bindings.rotations()?,
                }
            }
            "minecraft:copy_properties" => Node::CopyProperties(
                Box::new(Self::decode_holder(
                    required(&object, "source")?.get(),
                    bindings,
                    depth + 1,
                    remaining,
                    catalog,
                )?),
                bindings.properties()?,
            ),
            "minecraft:randomized_int" => Node::RandomizedInt {
                source: Box::new(Self::decode_holder(
                    required(&object, "source")?.get(),
                    bindings,
                    depth + 1,
                    remaining,
                    catalog,
                )?),
                property: serde_json::from_str(required(&object, "property")?.get())?,
                values: IntProvider::decode(required(&object, "values")?, depth + 1, remaining)?,
                properties: bindings.properties()?,
            },
            _ => return Err(DecodeError::Field("block state provider type")),
        }))
    }
    pub fn get_optional_state<W: FeatureWorld + PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        position: [i32; 3],
    ) -> Result<Option<BlockStateId>, StateProviderError<W::Error>> {
        Ok(match &self.0 {
            Node::RuleBased { fallback, rules } => {
                for (predicate, provider) in rules {
                    if predicate
                        .test(world, position)
                        .map_err(StateProviderError::Query)?
                    {
                        if let Some(state) = provider.get_optional_state(world, random, position)? {
                            return Ok(Some(state));
                        }
                    }
                }
                match fallback {
                    Some(provider) => provider.get_optional_state(world, random, position)?,
                    None => None,
                }
            }
            Node::Noise(provider) => Some(provider.sample(random, position)?),
            Node::Simple(state) => Some(*state),
            Node::Weighted(states) => Some(*states.sample(random)?),
            Node::RandomBlock(states) => {
                if states.is_empty() {
                    None
                } else {
                    Some(states[random.next_int_bound(states.len() as i32)? as usize])
                }
            }
            Node::Rotated {
                source,
                direction,
                rotations,
            } => {
                // Native orientation selection precedes the nested state draw.
                let direction = match direction {
                    Some(value) => *value,
                    None => random.next_int_bound(6)? as usize,
                };
                let state = source.get_state(world, random, position)?;
                Some(
                    rotations
                        .get(state.0 as usize)
                        .ok_or(StateProviderError::InvalidState)?[direction],
                )
            }
            Node::CopyProperties(source, properties) => {
                let target = source.get_state(world, random, position)?;
                let source = world.block(position).map_err(StateProviderError::World)?;
                Some(
                    properties
                        .copy(target, source)
                        .ok_or(StateProviderError::InvalidState)?,
                )
            }
            Node::RandomizedInt {
                source,
                property,
                values,
                properties,
            } => {
                let state = source.get_state(world, random, position)?;
                if properties
                    .has_integer(state, property)
                    .ok_or(StateProviderError::InvalidState)?
                {
                    let value = values.sample(random)?;
                    Some(
                        properties
                            .set_integer(state, property, value)
                            .ok_or(StateProviderError::InvalidPropertyValue)?,
                    )
                } else {
                    Some(state)
                }
            }
        })
    }
    pub fn get_state<W: FeatureWorld + PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        position: [i32; 3],
    ) -> Result<BlockStateId, StateProviderError<W::Error>> {
        match self.get_optional_state(world, random, position)? {
            Some(state) => Ok(state),
            None => world.block(position).map_err(StateProviderError::World),
        }
    }
}

/// 1.20.1's DataVersion: the version the legacy data-pack aliases read.
const LEGACY_DATA_VERSION: i32 = 3465;

/// A block state, or, when the pinned registry has no such state, the same state
/// as 1.20.1 data writes it, brought to 26.3 by the DataFixer's block-state fixes
/// (`grass` became `short_grass`, `chain` `iron_chain`). The 26.3 reading wins.
pub(crate) fn decode_state(
    codec: &BlockStateJsonCodec<'_>,
    json: &str,
    full: bool,
) -> Result<BlockStateId, DecodeError> {
    codec.decode(json, full).or_else(|error| {
        legacy_state(json, full)
            .and_then(|fixed| codec.decode(&fixed, full).ok())
            .ok_or_else(|| DecodeError::Registry(error.to_string()))
    })
}

fn legacy_state(json: &str, full: bool) -> Option<String> {
    use crate::datafix::dynamic::{as_str, empty_map, get, map, set, string};
    use serde_json::Value;
    let value: Value = serde_json::from_str(json).ok()?;
    let (name, properties) = match &value {
        Value::String(name) if !full => (name.as_str(), None),
        Value::Object(o) => (
            o.get("id").or_else(|| o.get("Name"))?.as_str()?,
            o.get("properties")
                .or_else(|| o.get("Properties"))
                .and_then(Value::as_object),
        ),
        _ => return None,
    };
    let mut tag = empty_map();
    set(&mut tag, "Name", string(name));
    if let Some(properties) = properties {
        let mut values = empty_map();
        for (key, value) in properties {
            if let Some(value) = value.as_str() {
                set(&mut values, key, string(value));
            }
        }
        set(&mut tag, "Properties", values);
    }
    crate::datafix::update(
        crate::datafix::schema::Ty::BlockState,
        &mut tag,
        LEGACY_DATA_VERSION,
        crate::datafix::CURRENT,
    );
    // The 26.x fixes also rename the fields to `id` and `properties`.
    let field = |current, old| get(&tag, current).or_else(|| get(&tag, old));
    let mut fixed = serde_json::Map::new();
    fixed.insert("id".into(), Value::String(as_str(field("id", "Name"))?));
    if let Some(values) = field("properties", "Properties").and_then(map) {
        let mut properties = serde_json::Map::new();
        for (key, value) in values {
            properties.insert(key.to_string().ok()?, Value::String(as_str(Some(value))?));
        }
        fixed.insert("properties".into(), Value::Object(properties));
    }
    Some(Value::Object(fixed).to_string())
}

/// Blocks a bound block tag may list (8,192 before content mods, vanilla's
/// tags fit well within it): content mods' blocks join the shared tags
/// (`mineable/pickaxe` takes every stone block of a pack).
pub(crate) const MAX_TAG_BLOCKS: usize = 1 << 20;

/// Bind the pinned registry's full-axis/full-facing/horizontal-facing property
/// families. A five-way facing or two-axis property is a different property and
/// is intentionally left unchanged. Unrelated properties retain their values.
fn rotation_table(registry: &BlockRegistry) -> Result<Rotations, String> {
    let mut values = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for index in 0..registry.len() {
        let state = registry.state(BlockStateId(index as u32)).unwrap();
        for name in ["axis", "facing"] {
            if let Some(value) = state.properties.get(name) {
                values
                    .entry((state.name.clone(), name.into()))
                    .or_default()
                    .insert(value.clone());
            }
        }
    }
    let axis = ["x", "y", "z"].map(str::to_owned).into_iter().collect();
    let facing = DIRECTIONS.map(str::to_owned).into_iter().collect();
    let horizontal = DIRECTIONS[2..].iter().map(|s| (*s).to_owned()).collect();
    let mut result = Vec::with_capacity(registry.len());
    for index in 0..registry.len() {
        let state = registry.state(BlockStateId(index as u32)).unwrap();
        let full_axis = values.get(&(state.name.clone(), "axis".into())) == Some(&axis);
        let directions = values.get(&(state.name.clone(), "facing".into()));
        let mut rotated = [BlockStateId(index as u32); 6];
        for (direction, name) in DIRECTIONS.iter().enumerate() {
            let mut value = state.clone();
            if full_axis {
                value.properties.insert(
                    "axis".into(),
                    ["y", "y", "z", "z", "x", "x"][direction].into(),
                );
            }
            if directions == Some(&facing) || (direction >= 2 && directions == Some(&horizontal)) {
                value.properties.insert("facing".into(), (*name).into());
            }
            rotated[direction] = registry
                .id(&value)
                .ok_or_else(|| "incomplete rotation state registry".to_owned())?;
        }
        result.push(rotated);
    }
    Ok(Arc::new(result))
}

/// A registry of `blocks`, each with every combination of its property values
/// (the first combination is the default), for the worldgen unit tests.
#[cfg(test)]
pub(crate) fn test_registry(blocks: &[(&str, &[(&str, &[&str])])]) -> BlockRegistry {
    use serde_json::{Map, Value, json};
    let mut rows = Vec::new();
    let mut defaults = Map::new();
    for (name, properties) in blocks {
        defaults.insert((*name).into(), rows.len().into());
        let mut states = vec![Map::new()];
        for (key, values) in *properties {
            states = (states.iter())
                .flat_map(|state| {
                    values.iter().map(move |value| {
                        let mut state = state.clone();
                        state.insert((*key).into(), Value::from(*value));
                        state
                    })
                })
                .collect();
        }
        for state in states {
            rows.push(
                json!([name, name.ends_with("air"), false, "", {"id": name, "properties": state}]),
            );
        }
    }
    let facts = json!({"states": rows, "defaults": defaults});
    BlockRegistry::from_facts_json(facts.to_string().as_bytes()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_of_1_20_1_data_decode_through_the_datafixer() {
        let registry = test_registry(&[
            ("minecraft:stone", &[]),
            ("minecraft:short_grass", &[]),
            ("minecraft:iron_chain", &[("axis", &["y", "x", "z"])]),
        ]);
        let bindings =
            StateProviderBindings::new(&registry, BTreeSet::new(), &BTreeMap::new()).unwrap();
        let state = |json: &str, full| decode_state(&bindings.codec, json, full);
        let current = |json: &str| bindings.codec.decode(json, true).unwrap();
        let grass = current(r#"{"id":"minecraft:short_grass"}"#);
        // 1.20.3 renamed grass, 1.21.9 chain; properties carry over.
        assert_eq!(state(r#"{"Name":"minecraft:grass"}"#, true).unwrap(), grass);
        assert_eq!(state(r#""minecraft:grass""#, false).unwrap(), grass);
        assert_eq!(
            state(
                r#"{"Name":"minecraft:chain","Properties":{"axis":"x"}}"#,
                true
            )
            .unwrap(),
            current(r#"{"id":"minecraft:iron_chain","properties":{"axis":"x"}}"#)
        );
        assert!(state(r#"{"Name":"minecraft:not_a_block"}"#, true).is_err());
        // A bare name is not a full state, whatever its version.
        assert!(state(r#""minecraft:grass""#, true).is_err());
        assert!(state(r#""minecraft:stone""#, true).is_err());
    }

    #[test]
    fn provider_types_of_1_20_1_decode_under_their_26_3_names() {
        let registry = test_registry(&[
            ("minecraft:stone", &[]),
            ("minecraft:short_grass", &[]),
            ("minecraft:iron_chain", &[("axis", &["y", "x", "z"])]),
        ]);
        let bindings =
            StateProviderBindings::new(&registry, BTreeSet::new(), &BTreeMap::new()).unwrap();
        for json in [
            r#"{"type":"minecraft:weighted_state_provider","entries":[{"weight":1,"data":{"Name":"minecraft:grass"}}]}"#,
            r#"{"type":"minecraft:randomized_int_state_provider","property":"age","values":{"type":"minecraft:uniform","value":{"min_inclusive":12,"max_inclusive":25}},"source":{"type":"minecraft:simple_state_provider","state":{"Name":"minecraft:stone"}}}"#,
            r#"{"type":"minecraft:rotated_block_provider","state":{"Name":"minecraft:chain","Properties":{"axis":"y"}}}"#,
        ] {
            assert!(
                BlockStateProvider::from_json(json, &bindings).is_ok(),
                "{json}"
            );
        }
        let unknown = r#"{"type":"minecraft:randomized_int_state_provider","property":"age","values":1,"source":{"Name":"minecraft:not_a_block"}}"#;
        assert!(BlockStateProvider::from_json(unknown, &bindings).is_err());
    }
}
