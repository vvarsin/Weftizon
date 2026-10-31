//! Content mods' machines at run time (docs/mods.md, "Machines"; Weft's
//! `r.machine`): the blocks whose `machine` declaration `mod_content` wrote
//! into the content root. Such a block's block entity is a mod block entity
//! with slots (`ModBlockEntity::items`), which opens as the declared vanilla
//! screen, takes items by its groups' rules, is saved, spills when broken
//! and ticks for native mods with the ticks it missed (`catch_up`).
use horizon_mods::machine::Machine;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

/// The file in the content root.
pub const FILE: &str = "horizon-mod-machines.json";

/// One declared machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub block: String,
    pub machine: Machine,
}

/// The declared machines by block.
#[derive(Clone, Debug, Default)]
pub struct Machines {
    by_block: HashMap<String, Machine>,
}

impl Machines {
    /// The content root's machines; none without the file (no content mod
    /// declares one).
    pub fn load(root: &Path) -> Self {
        let Ok(bytes) = std::fs::read(root.join(FILE)) else {
            return Self::default();
        };
        let entries: Vec<Entry> = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("Horizon: {FILE}: {e}");
            Vec::new()
        });
        Self::new(entries)
    }
    pub fn new(entries: Vec<Entry>) -> Self {
        Self {
            by_block: entries.into_iter().map(|e| (e.block, e.machine)).collect(),
        }
    }
    /// Block `block`'s machine.
    pub fn get(&self, block: &str) -> Option<&Machine> {
        self.by_block.get(block)
    }
    pub fn is_empty(&self) -> bool {
        self.by_block.is_empty()
    }
}

/// Whether a click may put `item` into slot `slot` of `machine`: not into
/// an output, and only what its group accepts (`#tags` through `in_tag`).
pub fn accepts(machine: &Machine, slot: usize, item: &str, in_tag: impl Fn(&str) -> bool) -> bool {
    let Some(group) = machine.group_of(slot) else {
        return true;
    };
    if group.output {
        return false;
    }
    group.accepts.is_empty()
        || group.accepts.iter().any(|a| match a.strip_prefix('#') {
            Some(tag) => in_tag(tag),
            None => a == item,
        })
}

/// The most items slot `slot` of `machine` holds, when its group says.
pub fn limit(machine: &Machine, slot: usize) -> Option<u32> {
    machine.group_of(slot).and_then(|g| g.limit)
}

/// The order items moved in without a slot (a shift-click, a hopper, a
/// dropper) try its slots: the groups with an accepts rule first (Weft's),
/// so coal finds the fuel before an input that takes anything, then the
/// rest in order.
pub fn fill_order(machine: &Machine) -> Vec<usize> {
    let (ruled, plain): (Vec<usize>, Vec<usize>) = (0..machine.slots())
        .partition(|&s| machine.group_of(s).is_some_and(|g| !g.accepts.is_empty()));
    ruled.into_iter().chain(plain).collect()
}

/// Whether a hopper below may pull from slot `slot`: only its outputs when
/// it has any (as a furnace gives up only its result), else any slot.
pub fn hopper_takes(machine: &Machine, slot: usize) -> bool {
    !machine.groups.iter().any(|g| g.output) || machine.group_of(slot).is_some_and(|g| g.output)
}

/// The most calls a machine's missed ticks are handed over in (Weft's).
pub const CATCH_UP_CALLS: u64 = 20;

/// The ticks a machine is owed now, `elapsed` game ticks after its last
/// tick, as the calls to make: one tick almost always; after a gap (its
/// chunk was unloaded) the whole gap, in at most `CATCH_UP_CALLS` calls of
/// near-equal ticks, larger first. Nothing when it already ticked this game
/// tick (or the clock went back).
pub fn catch_up(elapsed: i64) -> Vec<u32> {
    if elapsed <= 0 {
        return Vec::new();
    }
    let total = elapsed as u64;
    let calls = total.min(CATCH_UP_CALLS);
    let (each, rest) = (total / calls, total % calls);
    (0..calls)
        .map(|i| u32::try_from(each + u64::from(i < rest)).unwrap_or(u32::MAX))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missed_ticks_come_in_at_most_twenty_calls() {
        assert_eq!(catch_up(1), [1]);
        assert!(catch_up(0).is_empty() && catch_up(-5).is_empty());
        assert_eq!(catch_up(3), [1, 1, 1]);
        let gap = catch_up(1_000_003);
        assert_eq!(gap.len(), 20);
        assert_eq!(gap.iter().map(|&t| u64::from(t)).sum::<u64>(), 1_000_003);
        assert_eq!((gap[0], gap[19]), (50_001, 50_000));
    }

    #[test]
    fn clicks_follow_the_groups_rules() {
        let m: Machine = serde_json::from_str(
            r##"{"screen": "furnace", "groups": [{"name": "input", "limit": 16},
                {"name": "fuel", "accepts": ["#minecraft:coals", "minecraft:blaze_rod"]},
                {"name": "output", "output": true}]}"##,
        )
        .unwrap();
        let coals = |tag: &str| tag == "minecraft:coals";
        assert!(accepts(&m, 0, "minecraft:iron_ore", coals));
        assert!(accepts(&m, 1, "minecraft:charcoal", coals));
        assert!(accepts(&m, 1, "minecraft:blaze_rod", |_| false));
        assert!(!accepts(&m, 1, "minecraft:stone", |_| false));
        assert!(!accepts(&m, 2, "minecraft:iron_ingot", coals));
        assert_eq!((limit(&m, 0), limit(&m, 1)), (Some(16), None));
        // Moved in without a slot: the fuel first, then the input; nothing
        // goes to the output (`accepts` refuses it). Hoppers pull results only.
        assert_eq!(fill_order(&m), [1, 0, 2]);
        assert_eq!(
            (0..3).map(|s| hopper_takes(&m, s)).collect::<Vec<_>>(),
            [false, false, true]
        );
        let crate_: Machine = serde_json::from_str(r#"{"screen": "hopper"}"#).unwrap();
        assert_eq!(fill_order(&crate_), [0, 1, 2, 3, 4]);
        assert!(hopper_takes(&crate_, 3) && accepts(&crate_, 3, "minecraft:stone", |_| false));
        let machines = Machines::new(vec![Entry {
            block: "ruby:grinder".into(),
            machine: m,
        }]);
        assert!(machines.get("ruby:grinder").is_some() && machines.get("ruby:ore").is_none());
    }
}
