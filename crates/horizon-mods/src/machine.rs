//! Weft's `r.machine` as data (docs/mods.md, "Machines"): a block
//! declaration's `machine` gives the block a block entity holding slots in
//! named groups, which opens as one of vanilla's container screens, is saved
//! with the world, spills when broken, and is ticked for native mods
//! (`native_machine_tick`, docs/native-mods.md).
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// The blocks a machine may copy: their use opens a menu and they keep a
/// block entity, and nothing else of theirs reads it.
pub const TEMPLATES: [&str; 4] = [
    "minecraft:barrel",
    "minecraft:furnace",
    "minecraft:blast_furnace",
    "minecraft:smoker",
];

/// The vanilla screen a machine opens as: how it looks and how many slots it
/// shows. Players need nothing installed to use it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    /// A chest's rows of nine (9 to 54 slots).
    Chest,
    /// A dispenser's three by three.
    Dispenser,
    /// A hopper's row of five.
    Hopper,
    /// A furnace's input, fuel and result, with its flame and arrow.
    Furnace,
    BlastFurnace,
    Smoker,
}

impl Screen {
    /// Whether it shows `slots` slots.
    pub fn fits(self, slots: usize) -> bool {
        match self {
            Screen::Chest => slots.is_multiple_of(9) && (9..=54).contains(&slots),
            _ => slots == self.default_slots(),
        }
    }
    /// Its slots when the machine names no groups.
    pub fn default_slots(self) -> usize {
        match self {
            Screen::Chest => 27,
            Screen::Dispenser => 9,
            Screen::Hopper => 5,
            Screen::Furnace | Screen::BlastFurnace | Screen::Smoker => 3,
        }
    }
    /// Its `minecraft:menu` type with `slots` slots.
    pub fn menu_type(self, slots: usize) -> &'static str {
        match self {
            Screen::Chest => match slots / 9 {
                1 => "minecraft:generic_9x1",
                2 => "minecraft:generic_9x2",
                4 => "minecraft:generic_9x4",
                5 => "minecraft:generic_9x5",
                6 => "minecraft:generic_9x6",
                _ => "minecraft:generic_9x3",
            },
            Screen::Dispenser => "minecraft:generic_3x3",
            Screen::Hopper => "minecraft:hopper",
            Screen::Furnace => "minecraft:furnace",
            Screen::BlastFurnace => "minecraft:blast_furnace",
            Screen::Smoker => "minecraft:smoker",
        }
    }
    /// The values its screen draws (`ContainerData`): a furnace's lit time
    /// left, lit total, cooking progress and cooking total.
    pub fn data_slots(self) -> usize {
        match self {
            Screen::Furnace | Screen::BlastFurnace | Screen::Smoker => 4,
            _ => 0,
        }
    }
}

/// A named run of a machine's slots, and what may go in them.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    /// How many slots; a group's follow the group before it.
    #[serde(default = "one")]
    pub slots: usize,
    /// Items only come out (results): no click puts any in.
    #[serde(default)]
    pub output: bool,
    /// Only these items and `#tags` go in by hand; empty: any item.
    #[serde(default)]
    pub accepts: Vec<String>,
    /// At most this many items in each of its slots.
    #[serde(default)]
    pub limit: Option<u32>,
}
fn one() -> usize {
    1
}

/// A block's `machine`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Machine {
    pub screen: Screen,
    /// Its slots in order; none: one group `slots` the screen's size.
    #[serde(default)]
    pub groups: Vec<Group>,
}

impl Machine {
    /// How many slots it has.
    pub fn slots(&self) -> usize {
        if self.groups.is_empty() {
            self.screen.default_slots()
        } else {
            self.groups.iter().map(|g| g.slots).sum()
        }
    }
    /// The slots of group `name` (`slots` when it names none).
    pub fn group(&self, name: &str) -> Option<Range<usize>> {
        if self.groups.is_empty() {
            return (name == "slots").then(|| 0..self.slots());
        }
        let mut start = 0;
        for g in &self.groups {
            if g.name == name {
                return Some(start..start + g.slots);
            }
            start += g.slots;
        }
        None
    }
    /// The group slot `slot` is in.
    pub fn group_of(&self, slot: usize) -> Option<&Group> {
        let mut start = 0;
        for g in &self.groups {
            if (start..start + g.slots).contains(&slot) {
                return Some(g);
            }
            start += g.slots;
        }
        None
    }
    /// What the game could not use, for a block copying `template`.
    pub fn check(&self, template: &str) -> Result<(), String> {
        if !TEMPLATES.contains(&template) {
            return Err(format!(
                "a machine copies one of {}, not {template}",
                TEMPLATES.join(", ")
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        for g in &self.groups {
            let plain = !g.name.is_empty()
                && g.name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            if !plain {
                return Err(format!("group name {:?} is not [a-z0-9_]", g.name));
            }
            if !names.insert(g.name.as_str()) {
                return Err(format!("group {:?} is named twice", g.name));
            }
            if g.slots == 0 {
                return Err(format!("group {:?} has no slots", g.name));
            }
            if g.output && !g.accepts.is_empty() {
                return Err(format!(
                    "group {:?} is an output: nothing goes in to accept",
                    g.name
                ));
            }
            if let Some(bad) = g
                .accepts
                .iter()
                .find(|a| !crate::content::resource_id(a.strip_prefix('#').unwrap_or(a)))
            {
                return Err(format!(
                    "group {:?} accepts {bad:?}: an item id or #tag (<namespace>:<path>)",
                    g.name
                ));
            }
            if g.limit.is_some_and(|l| !(1..=99).contains(&l)) {
                return Err(format!("group {:?}'s limit is 1 to 99", g.name));
            }
        }
        let slots = self.slots();
        if !self.screen.fits(slots) {
            return Err(match self.screen {
                Screen::Chest => {
                    format!("a chest screen shows 9 to 54 slots, rows of 9, not {slots}")
                }
                s => format!(
                    "a {s:?} screen shows {} slots, not {slots}",
                    s.default_slots()
                ),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(json: &str) -> Machine {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn groups_number_the_slots_in_order() {
        let m = machine(
            r##"{"screen": "furnace", "groups": [
                {"name": "input"}, {"name": "fuel", "accepts": ["#minecraft:coals"]},
                {"name": "output", "output": true}]}"##,
        );
        m.check("minecraft:furnace").unwrap();
        assert_eq!(m.slots(), 3);
        assert_eq!(m.group("fuel"), Some(1..2));
        assert_eq!(m.group_of(2).map(|g| g.output), Some(true));
        assert_eq!(m.group("nope"), None);
        let crate_ = machine(r#"{"screen": "chest"}"#);
        assert_eq!((crate_.slots(), crate_.group("slots")), (27, Some(0..27)));
        assert_eq!(crate_.screen.menu_type(27), "minecraft:generic_9x3");
        assert_eq!(Screen::Chest.menu_type(54), "minecraft:generic_9x6");
        assert_eq!(Screen::Furnace.data_slots(), 4);
    }

    #[test]
    fn what_the_game_cant_use_is_refused() {
        let refused = |json: &str, template: &str| machine(json).check(template).unwrap_err();
        assert!(refused(r#"{"screen": "chest"}"#, "minecraft:chest").contains("copies one of"));
        assert!(
            refused(
                r#"{"screen": "chest", "groups": [{"name": "a", "slots": 10}]}"#,
                "minecraft:barrel"
            )
            .contains("rows of 9")
        );
        assert!(
            refused(
                r#"{"screen": "furnace", "groups": [{"name": "a"}]}"#,
                "minecraft:furnace"
            )
            .contains("3 slots")
        );
        assert!(
            refused(
                r#"{"screen": "hopper", "groups": [{"name": "a", "slots": 4}, {"name": "a"}]}"#,
                "minecraft:barrel"
            )
            .contains("named twice")
        );
        assert!(refused(
            r#"{"screen": "dispenser", "groups": [{"name": "out", "slots": 9, "output": true, "accepts": ["minecraft:stone"]}]}"#,
            "minecraft:barrel"
        )
        .contains("output"));
        assert!(refused(
            r#"{"screen": "dispenser", "groups": [{"name": "in", "slots": 9, "accepts": ["Stone"]}]}"#,
            "minecraft:barrel"
        )
        .contains("item id or #tag"));
        assert!(
            refused(
                r#"{"screen": "dispenser", "groups": [{"name": "in", "slots": 9, "limit": 0}]}"#,
                "minecraft:barrel"
            )
            .contains("limit")
        );
        assert!(serde_json::from_str::<Machine>(r#"{"screen": "anvil"}"#).is_err());
    }
}
