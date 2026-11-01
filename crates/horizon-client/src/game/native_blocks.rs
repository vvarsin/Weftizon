//! Blocks for native mods (docs/native-mods.md, "Blocks", API 64): Weft's
//! block `on_use` and `on_scheduled` and the World API's blocks. The hook
//! points `native_use_block` and `native_block_tick` (in `native_hooks.rs`)
//! are called from here, and the helpers below read and change the level's
//! blocks by name and state, as Weft's `w.get`, `w.set`, `w.prop`,
//! `w.set_prop` and `w.schedule` do. They run where the world is simulated
//! (single player's server thread, a LAN host, a dedicated server), never on
//! a client.
use super::Game;
use horizon_core::position::BlockPos;
use horizon_world::{
    chunk::BlockState,
    registry::{BlockRegistry, BlockStateId},
};
use std::collections::BTreeMap;

/// A block as text, as Weft and the commands write it:
/// `ruby:lamp[lit=true,facing=north]`. No namespace is `minecraft`'s.
pub(super) fn parse_state(text: &str) -> Result<BlockState, String> {
    let (name, props) = match text.split_once('[') {
        Some((name, rest)) => (
            name,
            rest.strip_suffix(']')
                .ok_or_else(|| format!("{text:?}: no closing ]"))?,
        ),
        None => (text, ""),
    };
    let name = name.trim();
    if name.is_empty() || name.contains(']') {
        return Err(format!("{text:?} is not a block"));
    }
    let name = match name.contains(':') {
        true => name.to_owned(),
        false => format!("minecraft:{name}"),
    };
    let mut properties = BTreeMap::new();
    for pair in props.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("{text:?}: {pair:?} is not property=value"))?;
        properties.insert(key.trim().to_owned(), value.trim().to_owned());
    }
    Ok(BlockState { name, properties })
}

/// `state` with property `key` at `value`, its other properties kept.
fn with_property(
    registry: &BlockRegistry,
    state: BlockStateId,
    key: &str,
    value: &str,
) -> Option<BlockStateId> {
    let mut s = registry.state(state)?.clone();
    s.properties.get_mut(key).map(|v| *v = value.to_owned())?;
    registry.id(&s)
}

/// A content mod's block: any namespace but vanilla's.
pub(super) fn is_mod_block(name: &str) -> bool {
    !name.starts_with("minecraft:")
}

/// Calls into the hook points.
impl Game<'_> {
    /// `native_use_block` for the acting player's right click on the block at
    /// `pos`: true when a mod took it.
    pub(super) fn native_use_blocks(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let stack = self.held().cloned();
        let mut claimed = false;
        self.native_use_block(stack.as_ref(), pos, face, state, &mut claimed);
        claimed
    }
    /// `native_block_tick` for a scheduled tick of a mod's block that ran.
    pub(super) fn native_block_ticked(&mut self, (pos, state): (BlockPos, BlockStateId)) {
        self.native_block_tick([pos.x, pos.y, pos.z], state);
    }
}

/// Helpers for native mods' blocks, kept as stable as the hook points.
// Native mods use these; nothing in Horizon does.
#[allow(dead_code)]
impl Game<'_> {
    /// The block at `pos` in the level the hook runs in, with its
    /// properties; None outside the world or where no chunk is loaded.
    pub(super) fn native_block_at(&self, pos: [i32; 3]) -> Option<BlockState> {
        super::native_world_read::read_block(&self.world, self.pack.assets.registry, pos).ok()
    }
    /// Sets the block at `pos` to `block` (`ruby:lamp[lit=true]`; properties
    /// it doesn't name take the block's defaults), as `/setblock` does: the
    /// old block entity goes, the new block gets its own, and the neighbours
    /// and players hear of it. `Ok(false)` when that state was there already.
    pub(super) fn native_set_block_at(
        &mut self,
        pos: [i32; 3],
        block: &str,
    ) -> Result<bool, String> {
        let wanted = parse_state(block)?;
        let state = self
            .pack
            .assets
            .registry
            .resolve(&wanted)
            .map_err(|e| format!("{block:?}: {e}"))?;
        self.native_block_at(pos)
            .ok_or_else(|| format!("{pos:?} is not loaded"))?;
        self.place_state(pos, state, false)
            .map_err(|_| format!("{block:?} could not be set at {pos:?}"))
    }
    /// Sets property `key` of the block at `pos` to `value`, its other
    /// properties kept (Weft's `set_prop`); false when the block has no such
    /// property or value, or `pos` is not loaded.
    pub(super) fn native_set_block_property_at(
        &mut self,
        pos: [i32; 3],
        key: &str,
        value: &str,
    ) -> bool {
        if self.native_block_at(pos).is_none() {
            return false;
        }
        let current = self.world.block(pos[0], pos[1], pos[2]);
        let Some(state) = with_property(self.pack.assets.registry, current, key, value) else {
            return false;
        };
        state == current || self.place_state(pos, state, false).is_ok_and(|set| set)
    }
    /// Schedules a tick of the block at `pos` in `delay` ticks (Weft's
    /// `schedule`, vanilla's `Level.scheduleTick`); `native_block_tick` hears
    /// it when it comes due, if the same block is still there. One per block
    /// and position: an earlier one already waiting stays. False where no
    /// chunk is loaded.
    pub(super) fn native_schedule_block_tick(&mut self, pos: [i32; 3], delay: u32) -> bool {
        if self.native_block_at(pos).is_none() {
            return false;
        }
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let delay = delay.min(i32::MAX as u32) as i32;
        self.with_level(|b, level, ticks| b.schedule_tick(level, ticks, at, delay));
        true
    }
    /// Breaks the block at `pos` as a mob or an explosion does
    /// (`Level.destroyBlock(pos, drop)`): its break particles and sound, its
    /// drops without a tool when `drop`, a block entity's contents spilled,
    /// and the fluid it held left in its place. False when it was air or
    /// `pos` is not loaded. API 67.
    pub(super) fn native_break_block(&mut self, pos: [i32; 3], drop: bool) -> bool {
        if self.native_block_at(pos).is_none() {
            return false;
        }
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let broken = self.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, drop));
        if let Err(e) = self.process_level_events() {
            log::warn!("a native mod's block break at {pos:?}: {e}");
        }
        broken
    }
    /// How long the block at `pos` takes to break (its `destroy_time`:
    /// stone 1.5, obsidian 50); `None` for a block nothing breaks (bedrock,
    /// barriers: -1 in vanilla) or where no chunk is loaded. API 67.
    pub(super) fn native_block_hardness(&self, pos: [i32; 3]) -> Option<f32> {
        self.native_block_at(pos)?;
        let hardness = self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .hardness;
        (hardness >= 0.0).then_some(hardness)
    }
    /// Swings mob `id`'s arm (`LivingEntity.swing`), which players around
    /// see: a zombie hacking at a door, a mob mining its way. False when the
    /// mob isn't there. API 76.
    #[allow(dead_code)]
    pub(super) fn native_swing(&mut self, id: u32) -> bool {
        let Some(m) = self
            .mobs
            .iter_mut()
            .find(|m| m.id == id && !m.removed && m.alive())
        else {
            return false;
        };
        m.swing();
        true
    }
    /// Whether the block at `pos` can stop a mob (`BlockState.blocksMotion`):
    /// false for air, vines, grass, flowers, water and lava, cobwebs. `None`
    /// where no chunk is loaded. API 75.
    #[allow(dead_code)]
    pub(super) fn native_block_blocks_motion(&self, pos: [i32; 3]) -> Option<bool> {
        self.native_block_at(pos)?;
        // BlockState.blocksMotion: solid (legacySolid), but not a cobweb or
        // a bamboo sapling. Water and lava are not solid.
        let facts = self
            .res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]));
        Some(
            facts.legacy_solid
                && !matches!(
                    &*facts.name,
                    "minecraft:cobweb" | "minecraft:bamboo_sapling"
                ),
        )
    }
    /// Shows `breaker`'s crack on the block at `pos` to every player near
    /// it, as a zombie breaking a door does (`Level.destroyBlockProgress`):
    /// `progress` 0 to 9, anything else takes the crack away. One crack per
    /// breaker; pass the breaking mob's id. A crack not changed for 400
    /// ticks goes. API 67.
    pub(super) fn native_block_crack(&mut self, breaker: u32, pos: [i32; 3], progress: i32) {
        let event = horizon_client::mobs::MobEvent::DestroyProgress {
            mob: breaker,
            pos,
            progress,
        };
        if let Err(e) = self.handle_mob_events(vec![event]) {
            log::warn!("a native mod's block crack at {pos:?}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_text_reads_as_the_commands_write_it() {
        let lamp = parse_state("ruby:lamp[lit=true, facing=north]").unwrap();
        assert_eq!(lamp.name, "ruby:lamp");
        assert_eq!(lamp.properties["lit"], "true");
        assert_eq!(lamp.properties["facing"], "north");
        let stone = parse_state("stone").unwrap();
        assert_eq!(
            (stone.name.as_str(), stone.properties.len()),
            ("minecraft:stone", 0)
        );
        assert_eq!(parse_state("ruby:lamp[]").unwrap().properties.len(), 0);
        for bad in ["", "ruby:lamp[lit=true", "ruby:lamp[lit]", "[lit=true]"] {
            assert!(parse_state(bad).is_err(), "{bad:?}");
        }
        assert!(is_mod_block("ruby:lamp") && !is_mod_block("minecraft:stone"));
    }

    #[test]
    fn a_property_changes_alone() {
        let facts = br#"{"defaults":{"minecraft:air":0,"ruby:lamp":1},"states":[
            ["air",true,false,"","minecraft:air"],
            ["lamp",false,false,"",{"id":"ruby:lamp","properties":{"facing":"north","lit":"false"}}],
            ["lamp",false,false,"",{"id":"ruby:lamp","properties":{"facing":"north","lit":"true"}}],
            ["lamp",false,false,"",{"id":"ruby:lamp","properties":{"facing":"south","lit":"false"}}],
            ["lamp",false,false,"",{"id":"ruby:lamp","properties":{"facing":"south","lit":"true"}}]
        ]}"#;
        let registry = BlockRegistry::from_facts_json(&facts[..]).unwrap();
        let lit = |s| with_property(&registry, BlockStateId(s), "lit", "true");
        assert_eq!(lit(1), Some(BlockStateId(2)));
        assert_eq!(lit(3), Some(BlockStateId(4)));
        assert_eq!(lit(4), Some(BlockStateId(4)));
        assert_eq!(
            with_property(&registry, BlockStateId(1), "lit", "maybe"),
            None
        );
        assert_eq!(with_property(&registry, BlockStateId(1), "age", "1"), None);
        assert_eq!(
            with_property(&registry, BlockStateId(0), "lit", "true"),
            None
        );
    }
}
