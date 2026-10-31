//! Content mods' machines in the game (docs/mods.md, "Machines"; Weft's
//! `r.machine`): a declared machine block's block entity is a mod block
//! entity with the declared slots (`horizon_client::mod_machines`). Using it
//! opens its vanilla screen over those slots (game/lan_actions.rs), clicks
//! follow its groups' rules (lan/menu.rs), breaking it spills them
//! (game/spill.rs), and each tick hands it to native mods with the ticks it
//! missed (`native_machine_tick`, API 19). All of it runs where the world is
//! simulated.
use super::{Game, Resources};
use horizon_client::{
    block_entities::Pos,
    containers::{BlockEntity, ModBlockEntity},
    mod_machines::catch_up,
    world::World,
};
use horizon_mods::machine::Machine;
use horizon_world::nbt::Compound;
use std::ops::Range;

/// The machine the block at `key` is, if a content mod declared one.
pub(super) fn machine_at<'r>(res: &'r Resources, world: &World, key: Pos) -> Option<&'r Machine> {
    let state = world.block(key.0, key.1, key.2);
    res.machines().get(&res.facts().get(state).name)
}

/// A machine's slots and screen values made to match its declaration (a
/// world saved before the mod changed it keeps what fits).
fn fit(m: &mut ModBlockEntity, machine: &Machine) {
    m.items.resize(machine.slots(), None);
    m.data.resize(machine.screen.data_slots(), 0);
}

impl Game<'_> {
    /// `EntityBlock.newBlockEntity` of a mod's block after the native mods
    /// had their say (`entity`): a declared machine's gets its slots, under
    /// the native mod's id and compound when one named it, else the block's.
    pub(super) fn with_machine_slots(
        &self,
        block: &str,
        entity: Option<(String, Compound)>,
    ) -> Option<BlockEntity> {
        let machine = self.res.machines().get(block);
        if entity.is_none() && machine.is_none() {
            return None;
        }
        let (id, nbt) = entity.unwrap_or_else(|| (block.to_owned(), Compound::new()));
        let mut m = ModBlockEntity {
            id,
            nbt,
            ..Default::default()
        };
        if let Some(machine) = machine {
            fit(&mut m, machine);
        }
        Some(BlockEntity::Mod(Box::new(m)))
    }
    /// A player uses machine block `name` at `key`: its block entity (made
    /// if a command or an older world left none) becomes the open container,
    /// which a server shows its player as the machine's screen. False when
    /// the block is no machine.
    pub(super) fn open_machine(&mut self, key: Pos, name: &str) -> bool {
        let Some(machine) = self.res.machines().get(name) else {
            return false;
        };
        if !matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Mod(_))
        ) && let Some(entity) = self.with_machine_slots(name, None)
        {
            self.world.block_entities.insert(key, entity);
        }
        if let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&key) {
            fit(m, machine);
        }
        self.container = Some(key);
        self.post_block_game_event(
            "minecraft:container_open",
            [key.0, key.1, key.2],
            Some(horizon_client::mobs::Who::Player),
            None,
        );
        true
    }
    /// The menu type a machine block `name` at `key` opens as, its slots
    /// fitted to the declaration.
    pub(super) fn machine_menu(&mut self, key: Pos, name: &str) -> Option<&'static str> {
        let machine = self.res.machines().get(name)?;
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&key) else {
            return None;
        };
        fit(m, machine);
        Some(machine.screen.menu_type(machine.slots()))
    }
    /// A declared machine's tick, after the block entity's own
    /// (`native_block_entity_tick`): `native_machine_tick` once, or with the
    /// ticks it missed since its last tick (an unloaded chunk) in at most 20
    /// calls. It is out of the world during the calls, as in API 15's tick.
    pub(super) fn tick_machine(&mut self, pos: Pos) {
        let Some(machine) = machine_at(&self.res, &self.world, pos).cloned() else {
            return;
        };
        let block = self
            .res
            .facts()
            .get(self.world.block(pos.0, pos.1, pos.2))
            .name
            .clone();
        let now = self.world.game_time;
        let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos) else {
            return;
        };
        fit(m, &machine);
        let calls = catch_up(m.ticked.map_or(1, |t| now - t));
        m.ticked = Some(now);
        if calls.is_empty() {
            return;
        }
        // An empty id marks its place while the mod has it.
        let mut taken = std::mem::take(&mut **m);
        let before = taken.items.clone();
        for ticks in calls {
            self.native_machine_tick(
                [pos.0, pos.1, pos.2],
                &block,
                &mut taken.items,
                &mut taken.nbt,
                &mut taken.data,
                ticks,
            );
        }
        if let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos)
            && m.id.is_empty()
        {
            let changed = taken.items != before;
            **m = taken;
            fit(m, &machine);
            // BlockEntity.setChanged: the next save writes its chunk (with
            // the tick, so a reload catches up only on the time it was gone),
            // and comparators read changed slots again.
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            if changed {
                self.container_changed(pos);
            }
        }
    }
}

/// Helpers for native mods' machines, kept as stable as the hook points.
// Native mods use these; nothing in Horizon does.
#[allow(dead_code)]
impl Game<'_> {
    /// The slots of group `group` (`fuel`) of machine block `machine`
    /// (`ruby:grinder`), as its declaration numbers them; a machine without
    /// groups has one, `slots`.
    pub(super) fn native_machine_group(&self, machine: &str, group: &str) -> Option<Range<usize>> {
        self.res.machines().get(machine)?.group(group)
    }
    /// Machine block `machine`'s declaration.
    pub(super) fn native_machine(&self, machine: &str) -> Option<Machine> {
        self.res.machines().get(machine).cloned()
    }
}
