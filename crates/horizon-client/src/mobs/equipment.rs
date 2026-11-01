//! `LivingEntity.collectEquipmentChanges` for mobs: what a mob holds and
//! wears adds the items' attribute modifiers, as a player's do. A sword
//! adds its damage, armour its protection, toughness and knockback
//! resistance; a content mod's items (their `attribute_modifiers`) the same.
use super::Mob;
use super::finalize::{self, Attr, Op};
use std::sync::Arc;

/// The values the equipment gives, worked out again when it changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Equipment {
    /// The items it was worked out for (main hand, off hand, head to feet).
    key: [Option<Arc<str>>; 6],
    /// `getAttributeValue(ATTACK_DAMAGE)` with the equipment.
    pub attack_damage: f32,
    /// The armour and toughness the worn and held items add.
    pub armor: f32,
    pub toughness: f32,
    /// `getAttributeValue(KNOCKBACK_RESISTANCE)` with the equipment.
    pub knockback_resistance: f64,
}

/// `EquipmentSlotGroup.test`.
fn slot_in_group(group: &str, slot: &str) -> bool {
    group == slot
        || group == "any"
        || (group == "hand" && matches!(slot, "mainhand" | "offhand"))
        || (group == "armor" && matches!(slot, "feet" | "legs" | "chest" | "head" | "body"))
}

fn op_named(op: &str) -> Op {
    match op {
        "add_multiplied_base" => Op::AddMultipliedBase,
        "add_multiplied_total" => Op::AddMultipliedTotal,
        _ => Op::AddValue,
    }
}

fn key(m: &Mob) -> [Option<Arc<str>>; 6] {
    let e = &m.extra;
    [
        e.main_hand.clone(),
        e.off_hand.clone(),
        e.head.clone(),
        e.chest.clone(),
        e.legs.clone(),
        e.feet.clone(),
    ]
}

/// Every modifier of `attribute` its items give in their slots.
fn modifiers(m: &Mob, items: &crate::items::Items, attribute: &str) -> Vec<(f64, Op)> {
    let e = &m.extra;
    let slots = [
        ("mainhand", &e.main_hand),
        ("offhand", &e.off_hand),
        ("feet", &e.feet),
        ("legs", &e.legs),
        ("chest", &e.chest),
        ("head", &e.head),
    ];
    let mut out = Vec::new();
    for (slot, item) in slots {
        let Some(info) = item.as_deref().and_then(|i| items.get(i)) else {
            continue;
        };
        for (a, _, amount, op, group) in &info.modifiers {
            if a == attribute && slot_in_group(group, slot) {
                out.push((*amount, op_named(op)));
            }
        }
    }
    out
}

/// The armour attributes the type itself doesn't have: the plain sum of
/// the items' `add_value` modifiers (armour pieces use no other).
fn added(mods: &[(f64, Op)]) -> f32 {
    mods.iter()
        .filter(|(_, op)| *op == Op::AddValue)
        .map(|(a, _)| *a as f32)
        .sum()
}

/// Works the values out again when what it holds or wears changed. With
/// no item data (a bare unit test) nothing changes.
pub fn refresh(m: &mut Mob, items: Option<&crate::items::Items>) {
    let Some(items) = items else { return };
    let now = key(m);
    if m.extra.equipment.as_ref().is_some_and(|q| q.key == now) {
        return;
    }
    let damage = modifiers(m, items, "minecraft:attack_damage");
    let knock = modifiers(m, items, "minecraft:knockback_resistance");
    let q = Equipment {
        attack_damage: finalize::value_with(m, Attr::AttackDamage, &damage) as f32,
        armor: added(&modifiers(m, items, "minecraft:armor")),
        toughness: added(&modifiers(m, items, "minecraft:armor_toughness")),
        knockback_resistance: finalize::value_with(m, Attr::KnockbackResistance, &knock),
        key: now,
    };
    m.extra.equipment = Some(Box::new(q));
}
