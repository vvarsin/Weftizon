//! Content-mod feature algorithms: clean-room support for feature types a
//! mod declares natively. They are approximations of TheAfterdark's generators,
//! built from the declared block providers and ranges.
use super::{
    DecodeError, Object,
    block_feature::FeatureBlockBindings,
    feature::{FeatureError, FeatureWorld},
    feature_graph::FeatureExecutionError,
    required,
    value_provider::{FloatProvider, IntProvider},
};
use crate::registry::BlockStateId;
use horizon_core::random::{InvalidBound, RandomSource};
use serde_json::{Value, value::RawValue};

fn value(raw: &RawValue) -> Result<Value, DecodeError> {
    serde_json::from_str(raw.get()).map_err(DecodeError::from)
}

fn decode_state(
    json: &str,
    full: bool,
    bindings: &FeatureBlockBindings<'_>,
) -> Result<BlockStateId, DecodeError> {
    super::state_provider::decode_state(&bindings.codec, json, full)
}

fn state_value(
    v: &Value,
    bindings: &FeatureBlockBindings<'_>,
) -> Result<BlockStateId, DecodeError> {
    match v {
        // The codec reads JSON: a bare name goes in quoted.
        Value::String(_) => decode_state(&v.to_string(), false, bindings),
        Value::Object(o) => {
            if o.get("Name").and_then(Value::as_str).is_some()
                || o.get("id").and_then(Value::as_str).is_some()
            {
                decode_state(&v.to_string(), true, bindings)
            } else if let Some(fallback) = o.get("fallback") {
                state_value(fallback, bindings)
            } else if let Some(kind) = o.get("type").and_then(Value::as_str) {
                match kind {
                    "minecraft:simple_state_provider" => state_value(
                        o.get("state")
                            .ok_or(DecodeError::Field("simple provider state"))?,
                        bindings,
                    ),
                    "minecraft:weighted_state_provider"
                    | "minecraft:random_block_state_provider" => {
                        let entries = o
                            .get("entries")
                            .and_then(Value::as_array)
                            .ok_or(DecodeError::Field("weighted provider entries"))?;
                        let first = entries
                            .first()
                            .ok_or(DecodeError::Field("empty weighted provider"))?;
                        let data = first
                            .get("data")
                            .ok_or(DecodeError::Field("weighted provider data"))?;
                        state_value(data, bindings)
                    }
                    "minecraft:rule_based_state_provider" => {
                        let fallback = o
                            .get("fallback")
                            .ok_or(DecodeError::Field("rule provider fallback"))?;
                        state_value(fallback, bindings)
                    }
                    _ => decode_state(&v.to_string(), true, bindings),
                }
            } else {
                decode_state(&v.to_string(), true, bindings)
            }
        }
        _ => Err(DecodeError::Field("block state")),
    }
}

fn block_state(
    raw: &RawValue,
    bindings: &FeatureBlockBindings<'_>,
) -> Result<BlockStateId, DecodeError> {
    let v = value(raw)?;
    state_value(&v, bindings)
}

fn int_provider(raw: &RawValue) -> Result<IntProvider, DecodeError> {
    IntProvider::decode(raw, 0, &mut 65536)
}

fn float_provider(raw: &RawValue) -> Result<FloatProvider, DecodeError> {
    FloatProvider::from_json(raw.get())
}

fn set<W: FeatureWorld + super::placement::PlacementContext>(
    world: &mut W,
    p: [i32; 3],
    state: BlockStateId,
) -> Result<(), FeatureExecutionError<W::Error>> {
    let _ = world
        .set_block(p, state, 2)
        .map_err(|e| FeatureExecutionError::Basic(FeatureError::World(e)))?;
    Ok(())
}

fn block<W: FeatureWorld + super::placement::PlacementContext>(
    world: &mut W,
    p: [i32; 3],
) -> Result<BlockStateId, FeatureExecutionError<W::Error>> {
    world
        .block(p)
        .map_err(|e| FeatureExecutionError::Basic(FeatureError::World(e)))
}

fn sample_int(provider: &IntProvider, random: &mut impl RandomSource) -> Result<i32, InvalidBound> {
    provider.sample(random)
}

/// A random patch's offset on one axis: `nextInt(bound) - nextInt(bound)`,
/// with `bound` its spread plus one, so within `[-spread, spread]`.
fn spread_offset(random: &mut impl RandomSource, bound: i32) -> Result<i32, InvalidBound> {
    Ok(random.next_int_bound(bound)? - random.next_int_bound(bound)?)
}

fn nested_to_place(
    raw: &RawValue,
    bindings: &FeatureBlockBindings<'_>,
) -> Result<BlockStateId, DecodeError> {
    let mut value = value(raw)?;
    for _ in 0..8 {
        if let Some(to_place) = value.get("to_place") {
            return state_value(to_place, bindings);
        }
        if let Some(config) = value.get("config") {
            value = config.clone();
            continue;
        }
        if let Some(feature) = value.get("feature") {
            value = feature.clone();
            continue;
        }
        break;
    }
    Err(DecodeError::Field("nested feature to_place"))
}

fn supports_from(
    object: &Object,
    bindings: &FeatureBlockBindings<'_>,
) -> Result<Vec<BlockStateId>, DecodeError> {
    let mut out = Vec::new();
    for name in ["blockOn", "blockOn2"] {
        if let Some(raw) = super::optional(object, name) {
            out.push(block_state(raw, bindings)?);
        }
    }
    if let Some(raw) = super::optional(object, "validBlocks") {
        let list: Vec<Value> = serde_json::from_str(raw.get()).map_err(DecodeError::from)?;
        for item in list {
            out.push(state_value(&item, bindings)?);
        }
    }
    Ok(out)
}

/// A randomized crystal/blob on a support block.
#[derive(Clone, Debug)]
pub struct ModBlobFeature {
    material: BlockStateId,
    supports: Vec<BlockStateId>,
    width: IntProvider,
    height: IntProvider,
    mass: IntProvider,
}

impl ModBlobFeature {
    pub fn decode(
        json: &str,
        bindings: &FeatureBlockBindings<'_>,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("mod blob depth or work"));
        }
        *remaining -= 1;
        let object: Object = serde_json::from_str(json)?;
        let material_raw = super::optional(&object, "blobMaterial")
            .or_else(|| super::optional(&object, "stemMaterial"))
            .or_else(|| super::optional(&object, "material"))
            .ok_or(DecodeError::Field("mod blob material"))?;
        let material = block_state(material_raw, bindings)?;
        let supports = supports_from(&object, bindings)?;
        let width_raw = super::optional(&object, "blobWidth")
            .or_else(|| super::optional(&object, "radius"))
            .ok_or(DecodeError::Field("mod blob width"))?;
        let height_raw = super::optional(&object, "blobHeight")
            .or_else(|| super::optional(&object, "height"))
            .ok_or(DecodeError::Field("mod blob height"))?;
        let mass = super::optional(&object, "blobMass")
            .or_else(|| super::optional(&object, "blob_mass"))
            .map(int_provider)
            .transpose()?
            .unwrap_or_else(|| {
                IntProvider::decode(
                    &RawValue::from_string(
                        r#"{"type":"minecraft:uniform","value":{"min_inclusive":64,"max_inclusive":64}}"#.to_owned(),
                    )
                    .unwrap(),
                    0,
                    &mut 65536,
                ).unwrap()
            });
        Ok(Self {
            material,
            supports,
            width: int_provider(width_raw)?,
            height: int_provider(height_raw)?,
            mass,
        })
    }

    pub fn place<W: FeatureWorld + super::placement::PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: [i32; 3],
    ) -> Result<bool, FeatureExecutionError<W::Error>> {
        let width = sample_int(&self.width, random).map_err(FeatureExecutionError::Random)?;
        let height = sample_int(&self.height, random).map_err(FeatureExecutionError::Random)?;
        let mass = sample_int(&self.mass, random).map_err(FeatureExecutionError::Random)?;
        if width <= 0 || height <= 0 || mass <= 0 {
            return Ok(false);
        }
        let mut base = origin;
        if !self.supports.is_empty() {
            let mut found = false;
            for _ in 0..16 {
                let below = [base[0], base[1].wrapping_sub(1), base[2]];
                let state = block(world, below)?;
                if self.supports.contains(&state) {
                    found = true;
                    break;
                }
                base = below;
            }
            if !found {
                return Ok(false);
            }
        }
        let mut remaining = 1_048_576;
        for _ in 0..mass.min(1024) {
            if remaining == 0 {
                return Err(FeatureExecutionError::WorkLimit);
            }
            remaining -= 1;
            let dx = random
                .next_int_bound(width)
                .map_err(FeatureExecutionError::Random)?;
            let dz = random
                .next_int_bound(width)
                .map_err(FeatureExecutionError::Random)?;
            let dy = random
                .next_int_bound(height)
                .map_err(FeatureExecutionError::Random)?;
            let p = [
                base[0].wrapping_add(dx).wrapping_sub(width / 2),
                base[1].wrapping_add(dy),
                base[2].wrapping_add(dz).wrapping_sub(width / 2),
            ];
            set(world, p, self.material)?;
        }
        Ok(true)
    }
}

/// A vertical cave pillar with a sampled radius and height.
#[derive(Clone, Debug)]
pub struct ModPillarFeature {
    material: BlockStateId,
    radius: IntProvider,
    height_scale: FloatProvider,
    search_range: i32,
}

impl ModPillarFeature {
    pub fn decode(
        json: &str,
        material: &str,
        bindings: &FeatureBlockBindings<'_>,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("mod pillar depth or work"));
        }
        *remaining -= 1;
        let object: Object = serde_json::from_str(json)?;
        let material = bindings
            .registry()
            .default_state(material)
            .ok_or_else(|| DecodeError::Registry(format!("unknown pillar block {material}")))?;
        let radius_raw = required(&object, "column_radius")?;
        let height_raw = required(&object, "height_scale")?;
        let search_range = super::optional(&object, "floor_to_ceiling_search_range")
            .map(|raw| {
                serde_json::from_str::<i32>(raw.get())
                    .map_err(|_| DecodeError::Field("pillar search range"))
            })
            .transpose()?
            .unwrap_or(24);
        Ok(Self {
            material,
            radius: int_provider(radius_raw)?,
            height_scale: float_provider(height_raw)?,
            search_range: search_range.clamp(1, 64),
        })
    }

    pub fn place<W: FeatureWorld + super::placement::PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: [i32; 3],
    ) -> Result<bool, FeatureExecutionError<W::Error>> {
        let radius = sample_int(&self.radius, random).map_err(FeatureExecutionError::Random)?;
        if radius <= 0 {
            return Ok(false);
        }
        let scale = self.height_scale.sample(random);
        let height = ((self.search_range as f32) * scale).floor() as i32;
        if height <= 0 {
            return Ok(false);
        }
        let mut remaining = 1_048_576;
        for y in 0..height {
            let r = radius.max(1);
            for dx in -r..=r {
                for dz in -r..=r {
                    if dx * dx + dz * dz > r * r {
                        continue;
                    }
                    if remaining == 0 {
                        return Err(FeatureExecutionError::WorkLimit);
                    }
                    remaining -= 1;
                    let p = [
                        origin[0].wrapping_add(dx),
                        origin[1].wrapping_add(y),
                        origin[2].wrapping_add(dz),
                    ];
                    set(world, p, self.material)?;
                }
            }
        }
        Ok(true)
    }
}

/// A small pond of a mod's `to_place` blocks with a rim.
#[derive(Clone, Debug)]
pub struct ModPondFeature {
    to_place: BlockStateId,
    slab: BlockStateId,
}

impl ModPondFeature {
    pub fn decode(
        json: &str,
        bindings: &FeatureBlockBindings<'_>,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("mod pond depth or work"));
        }
        *remaining -= 1;
        let object: Object = serde_json::from_str(json)?;
        let to_place = if let Some(raw) = super::optional(&object, "to_place") {
            block_state(raw, bindings)?
        } else if let Some(raw) = super::optional(&object, "feature") {
            nested_to_place(raw, bindings)?
        } else {
            return Err(DecodeError::Field("mod patch to_place"));
        };
        let slab = block_state(required(&object, "slab_to_place")?, bindings)?;
        Ok(Self { to_place, slab })
    }

    pub fn place<W: FeatureWorld + super::placement::PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: [i32; 3],
    ) -> Result<bool, FeatureExecutionError<W::Error>> {
        let radius = random
            .next_int_bound(5)
            .map_err(FeatureExecutionError::Random)?
            + 2;
        let mut remaining = 1_048_576;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if remaining == 0 {
                    return Err(FeatureExecutionError::WorkLimit);
                }
                remaining -= 1;
                let edge = dx == -radius || dx == radius || dz == -radius || dz == radius;
                let p = [origin[0] + dx, origin[1], origin[2] + dz];
                set(world, p, if edge { self.slab } else { self.to_place })?;
            }
        }
        Ok(true)
    }
}

/// A small water fall basin for TheAfterdark's `catching_fall`.
#[derive(Clone, Debug)]
pub struct ModFallFeature {
    water: BlockStateId,
    basin: BlockStateId,
}

impl ModFallFeature {
    pub fn decode(
        json: &str,
        bindings: &FeatureBlockBindings<'_>,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("mod fall depth or work"));
        }
        *remaining -= 1;
        let object: Object = serde_json::from_str(json)?;
        let water = block_state(required(&object, "state")?, bindings)?;
        let basin_name = super::optional(&object, "basinMaterial")
            .map(|raw| serde_json::from_str::<String>(raw.get()))
            .transpose()?
            .unwrap_or_else(|| "minecraft:amethyst_block".to_owned());
        let basin = bindings
            .registry()
            .default_state(&basin_name)
            .ok_or_else(|| DecodeError::Registry(format!("unknown basin material {basin_name}")))?;
        Ok(Self { water, basin })
    }

    pub fn place<W: FeatureWorld + super::placement::PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: [i32; 3],
    ) -> Result<bool, FeatureExecutionError<W::Error>> {
        set(world, origin, self.water)?;
        let radius = random
            .next_int_bound(3)
            .map_err(FeatureExecutionError::Random)?
            + 1;
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx == -radius || dx == radius || dz == -radius || dz == radius {
                    let p = [origin[0] + dx, origin[1] - 1, origin[2] + dz];
                    set(world, p, self.basin)?;
                }
            }
        }
        Ok(true)
    }
}

/// A random patch of mod blocks (waterloggable patches use this approximation).
#[derive(Clone, Debug)]
pub struct ModPatchFeature {
    tries: IntProvider,
    xz_spread: IntProvider,
    y_spread: IntProvider,
    to_place: BlockStateId,
    air: BlockStateId,
}

impl ModPatchFeature {
    pub fn decode(
        json: &str,
        bindings: &FeatureBlockBindings<'_>,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("mod patch depth or work"));
        }
        *remaining -= 1;
        let object: Object = serde_json::from_str(json)?;
        let to_place = block_state(required(&object, "to_place")?, bindings)?;
        Ok(Self {
            tries: int_provider(required(&object, "tries")?)?,
            xz_spread: int_provider(required(&object, "xz_spread")?)?,
            y_spread: int_provider(required(&object, "y_spread")?)?,
            to_place,
            air: bindings.air[0],
        })
    }

    pub fn place<W: FeatureWorld + super::placement::PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: [i32; 3],
    ) -> Result<bool, FeatureExecutionError<W::Error>> {
        let tries = sample_int(&self.tries, random).map_err(FeatureExecutionError::Random)?;
        // 1.20.1 RandomPatchFeature: the spreads are read once; each try
        // lands at nextInt(spread + 1) - nextInt(spread + 1) on x, y, then z.
        let xz = sample_int(&self.xz_spread, random)
            .map_err(FeatureExecutionError::Random)?
            .wrapping_add(1);
        let y = sample_int(&self.y_spread, random)
            .map_err(FeatureExecutionError::Random)?
            .wrapping_add(1);
        let mut remaining = 1_048_576;
        for _ in 0..tries.clamp(0, 1024) {
            if remaining == 0 {
                return Err(FeatureExecutionError::WorkLimit);
            }
            remaining -= 1;
            let dx = spread_offset(random, xz).map_err(FeatureExecutionError::Random)?;
            let dy = spread_offset(random, y).map_err(FeatureExecutionError::Random)?;
            let dz = spread_offset(random, xz).map_err(FeatureExecutionError::Random)?;
            let p = [
                origin[0].wrapping_add(dx),
                origin[1].wrapping_add(dy),
                origin[2].wrapping_add(dz),
            ];
            if block(world, p)? == self.air {
                set(world, p, self.to_place)?;
            }
        }
        Ok(true)
    }
}

/// A small cave plant block patch for the cave_kelp/pickle/seagrass types.
#[derive(Clone, Debug)]
pub struct ModPlantFeature {
    block: BlockStateId,
}

impl ModPlantFeature {
    pub fn decode(
        json: &str,
        block: &str,
        bindings: &FeatureBlockBindings<'_>,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Self, DecodeError> {
        if depth > 128 || *remaining == 0 {
            return Err(DecodeError::Field("mod plant depth or work"));
        }
        *remaining -= 1;
        let _object: Object = serde_json::from_str(json)?;
        let block = bindings
            .registry()
            .default_state(block)
            .ok_or_else(|| DecodeError::Registry(format!("unknown plant block {block}")))?;
        Ok(Self { block })
    }

    pub fn place<W: FeatureWorld + super::placement::PlacementContext>(
        &self,
        world: &mut W,
        random: &mut impl RandomSource,
        origin: [i32; 3],
    ) -> Result<bool, FeatureExecutionError<W::Error>> {
        for _ in 0..3 {
            let dx = random
                .next_int_bound(3)
                .map_err(FeatureExecutionError::Random)?
                - 1;
            let dz = random
                .next_int_bound(3)
                .map_err(FeatureExecutionError::Random)?
                - 1;
            let p = [origin[0] + dx, origin[1], origin[2] + dz];
            set(world, p, self.block)?;
        }
        Ok(true)
    }
}

/// The material a TheAfterdark pillar type names by its type.
pub fn pillar_material(kind: &str) -> Result<&'static str, DecodeError> {
    let material = match kind {
        "the_afterdark:large_calcite_pillar" => "minecraft:calcite",
        "the_afterdark:large_deepslate_pillar" => "minecraft:deepslate",
        "the_afterdark:large_dark_prismarine_pillar" => "minecraft:dark_prismarine",
        "the_afterdark:large_prismarine_pillar" => "minecraft:prismarine",
        "the_afterdark:large_packed_ice_pillar" => "minecraft:packed_ice",
        "the_afterdark:large_blue_ice_pillar" => "minecraft:blue_ice",
        "the_afterdark:large_ice_pillar" => "minecraft:ice",
        "the_afterdark:large_obsidian_pillar" => "minecraft:obsidian",
        "the_afterdark:large_slime_pillar" => "minecraft:slime_block",
        "the_afterdark:large_basalt_pillar" => "minecraft:smooth_basalt",
        "the_afterdark:diamond_ore_pillar" => "minecraft:diamond_ore",
        "the_afterdark:emerald_ore_pillar" => "minecraft:emerald_ore",
        "the_afterdark:lapis_ore_pillar" => "minecraft:lapis_ore",
        "the_afterdark:redstone_ore_pillar" => "minecraft:redstone_ore",
        "the_afterdark:raw_copper_pillar" => "minecraft:raw_copper_block",
        "the_afterdark:raw_gold_pillar" => "minecraft:raw_gold_block",
        "the_afterdark:raw_iron_pillar" => "minecraft:raw_iron_block",
        _ => {
            return Err(DecodeError::Registry(format!(
                "unknown mod pillar type {kind}"
            )));
        }
    };
    Ok(material)
}

#[cfg(test)]
mod tests {
    use super::super::{
        anchor::WorldGenerationContext,
        feature_catalog::FeatureCatalog,
        feature_graph::FeatureDefinition,
        placement::{PlacementContext, PlacementQueryError},
        state_provider::test_registry,
        tags::TagDefinitions,
    };
    use super::*;
    use crate::heightmap::HeightmapType;
    use horizon_core::random::LegacyRandom;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn blob_materials_and_supports_read_bare_names_and_legacy_states() {
        let registry = test_registry(&[
            ("minecraft:air", &[]),
            ("minecraft:cave_air", &[]),
            ("minecraft:void_air", &[]),
            ("minecraft:smooth_basalt", &[]),
            ("minecraft:deepslate", &[]),
            ("minecraft:amethyst_block", &[]),
            ("minecraft:short_grass", &[]),
        ]);
        let bindings = FeatureBlockBindings::new(&registry, BTreeSet::new()).unwrap();
        let catalog = FeatureCatalog::from_resources(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &TagDefinitions::default(),
        )
        .unwrap();
        let state = |name: &str| registry.default_state(name).unwrap();
        // TheAfterdark's amethyst_spiral, as its 1.20.1 data writes it.
        let spiral = r#"{"type":"the_afterdark:spiral","config":{"validBlocks":["minecraft:smooth_basalt","minecraft:deepslate"],"stemMaterial":{"Name":"minecraft:amethyst_block"},"leafMaterial":{"Name":"minecraft:air"},"blobMass":{"type":"minecraft:uniform","value":{"min_inclusive":26,"max_inclusive":38}},"blobWidth":{"type":"minecraft:uniform","value":{"min_inclusive":12,"max_inclusive":18}},"blobHeight":{"type":"minecraft:uniform","value":{"min_inclusive":2,"max_inclusive":3}}}}"#;
        let FeatureDefinition::ModBlob(blob) =
            catalog.feature_with_blocks(spiral, &bindings).unwrap()
        else {
            panic!("spiral is a blob");
        };
        assert_eq!(blob.material, state("minecraft:amethyst_block"));
        assert_eq!(
            blob.supports,
            [
                state("minecraft:smooth_basalt"),
                state("minecraft:deepslate")
            ]
        );
        let grass = state_value(&serde_json::json!("minecraft:grass"), &bindings).unwrap();
        assert_eq!(grass, state("minecraft:short_grass"));
        assert!(state_value(&serde_json::json!("minecraft:not_a_block"), &bindings).is_err());
    }

    /// Air everywhere, remembering the blocks set.
    struct Recorder {
        air: BlockStateId,
        set: BTreeMap<[i32; 3], BlockStateId>,
    }
    impl FeatureWorld for Recorder {
        type Error = std::convert::Infallible;
        fn block(&mut self, p: [i32; 3]) -> Result<BlockStateId, Self::Error> {
            Ok(self.set.get(&p).copied().unwrap_or(self.air))
        }
        fn set_block(
            &mut self,
            p: [i32; 3],
            state: BlockStateId,
            _: i32,
        ) -> Result<bool, Self::Error> {
            self.set.insert(p, state);
            Ok(true)
        }
    }
    impl PlacementContext for Recorder {
        fn world_seed(&mut self) -> Result<i64, PlacementQueryError> {
            Ok(0)
        }
        fn block_state(&mut self, p: [i32; 3]) -> Result<BlockStateId, PlacementQueryError> {
            Ok(self.set.get(&p).copied().unwrap_or(self.air))
        }
        fn generation(&self) -> WorldGenerationContext {
            WorldGenerationContext::new(-64, 384, 63)
        }
        fn min_y(&mut self) -> Result<i32, PlacementQueryError> {
            Ok(-64)
        }
        fn height(&mut self, _: HeightmapType, _: i32, _: i32) -> Result<i32, PlacementQueryError> {
            Err(PlacementQueryError::Unavailable)
        }
    }

    #[test]
    fn waterloggable_patch_spreads_its_tries_as_random_patch_does() {
        let registry = test_registry(&[
            ("minecraft:air", &[]),
            ("minecraft:cave_air", &[]),
            ("minecraft:void_air", &[]),
            ("minecraft:short_grass", &[]),
        ]);
        let bindings = FeatureBlockBindings::new(&registry, BTreeSet::new()).unwrap();
        let grass = registry.default_state("minecraft:short_grass").unwrap();
        let config =
            r#"{"tries":96,"xz_spread":7,"y_spread":3,"to_place":"minecraft:short_grass"}"#;
        let patch = ModPatchFeature::decode(config, &bindings, 0, &mut 16).unwrap();
        let mut world = Recorder {
            air: bindings.air[0],
            set: BTreeMap::new(),
        };
        let origin = [100, 10, -50];
        let mut random = LegacyRandom::new(42);
        assert!(patch.place(&mut world, &mut random, origin).unwrap());

        // Each try at nextInt(8) - nextInt(8), nextInt(4) - nextInt(4),
        // nextInt(8) - nextInt(8) from the origin, in that order; a try on a
        // block already placed changes nothing.
        let mut expected_random = LegacyRandom::new(42);
        let mut expected = BTreeSet::new();
        for _ in 0..96 {
            let mut offset = |bound| {
                expected_random.next_int_bound(bound).unwrap()
                    - expected_random.next_int_bound(bound).unwrap()
            };
            let (dx, dy, dz) = (offset(8), offset(4), offset(8));
            expected.insert([origin[0] + dx, origin[1] + dy, origin[2] + dz]);
        }
        assert_eq!(random.next_long(), expected_random.next_long());
        let placed: BTreeSet<[i32; 3]> = world.set.keys().copied().collect();
        assert_eq!(placed, expected);
        assert!(world.set.values().all(|&s| s == grass));
        // A patch, not one block: the tries spread within the spreads.
        assert!(placed.len() > 40, "{} blocks", placed.len());
        assert!(placed.iter().all(|p| {
            (p[0] - origin[0]).abs() <= 7
                && (p[1] - origin[1]).abs() <= 3
                && (p[2] - origin[2]).abs() <= 7
        }));
        assert!(placed.iter().any(|p| p[1] != origin[1]));
    }
}
