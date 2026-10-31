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

fn state_value(
    v: &Value,
    bindings: &FeatureBlockBindings<'_>,
) -> Result<BlockStateId, DecodeError> {
    match v {
        Value::String(name) => bindings
            .codec
            .decode(name, false)
            .map_err(|e| DecodeError::Registry(e.to_string())),
        Value::Object(o) => {
            if let Some(name) = o.get("Name").and_then(Value::as_str) {
                let mut copy = v.clone();
                copy["id"] = Value::String(name.to_owned());
                copy.as_object_mut().unwrap().remove("Name");
                if let Some(properties) = o.get("Properties").and_then(Value::as_object) {
                    copy["properties"] = Value::Object(properties.clone());
                    copy.as_object_mut().unwrap().remove("Properties");
                }
                let text = copy.to_string();
                bindings
                    .codec
                    .decode(&text, true)
                    .map_err(|e| DecodeError::Registry(e.to_string()))
            } else if o.get("id").and_then(Value::as_str).is_some() {
                bindings
                    .codec
                    .decode(&v.to_string(), true)
                    .map_err(|e| DecodeError::Registry(e.to_string()))
            } else if let Some(fallback) = o.get("fallback") {
                state_value(fallback, bindings)
            } else if let Some(kind) = o.get("type").and_then(Value::as_str) {
                match kind {
                    "minecraft:simple_state_provider" => state_value(
                        o.get("state")
                            .ok_or_else(|| DecodeError::Field("simple provider state"))?,
                        bindings,
                    ),
                    "minecraft:weighted_state_provider"
                    | "minecraft:random_block_state_provider" => {
                        let entries = o
                            .get("entries")
                            .and_then(Value::as_array)
                            .ok_or_else(|| DecodeError::Field("weighted provider entries"))?;
                        let first = entries
                            .first()
                            .ok_or_else(|| DecodeError::Field("empty weighted provider"))?;
                        let data = first
                            .get("data")
                            .ok_or_else(|| DecodeError::Field("weighted provider data"))?;
                        state_value(data, bindings)
                    }
                    "minecraft:rule_based_state_provider" => {
                        let fallback = o
                            .get("fallback")
                            .ok_or_else(|| DecodeError::Field("rule provider fallback"))?;
                        state_value(fallback, bindings)
                    }
                    _ => bindings
                        .codec
                        .decode(&v.to_string(), true)
                        .map_err(|e| DecodeError::Registry(e.to_string())),
                }
            } else {
                bindings
                    .codec
                    .decode(&v.to_string(), true)
                    .map_err(|e| DecodeError::Registry(e.to_string()))
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

fn nested_to_place(raw: &RawValue, bindings: &FeatureBlockBindings<'_>) -> Result<BlockStateId, DecodeError> {
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
            .ok_or_else(|| DecodeError::Field("mod blob material"))?;
        let material = block_state(material_raw, bindings)?;
        let supports = supports_from(&object, bindings)?;
        let width_raw = super::optional(&object, "blobWidth")
            .or_else(|| super::optional(&object, "radius"))
            .ok_or_else(|| DecodeError::Field("mod blob width"))?;
        let height_raw = super::optional(&object, "blobHeight")
            .or_else(|| super::optional(&object, "height"))
            .ok_or_else(|| DecodeError::Field("mod blob height"))?;
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
                if self.supports.iter().any(|s| *s == state) {
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
        let mut remaining = 1_048_576;
        for _ in 0..tries.max(0).min(1024) {
            if remaining == 0 {
                return Err(FeatureExecutionError::WorkLimit);
            }
            remaining -= 1;
            let dx = sample_int(&self.xz_spread, random).map_err(FeatureExecutionError::Random)?;
            let dz = sample_int(&self.xz_spread, random).map_err(FeatureExecutionError::Random)?;
            let dy = sample_int(&self.y_spread, random).map_err(FeatureExecutionError::Random)?;
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
