//! What a declared entity does (docs/mods.md#entity-behaviour): AI `goals` and
//! `targets` from a fixed vocabulary, each mapped to the vanilla goal of the
//! same job (`mobs/goals.rs`), Weft's `goals(..)` as data.
//!
//! A declared list replaces the template's selector of that kind: an entity
//! with `goals` runs those goals and none of the template's, one with
//! `targets` picks its targets that way only. Leaving a list out keeps the
//! template's. Entities are named by id: `minecraft:player`, a vanilla mob
//! (`minecraft:zombie` is a zombie, not a mod's entity copying one) or a mod's
//! entity (`mymod:ghost`).
use serde::{Deserialize, Serialize};

/// Templates whose AI is a brain (activities and behaviours), not goals:
/// declared goals would run beside it, so a declaration of these has none.
pub const BRAIN_TEMPLATES: [&str; 8] = [
    "minecraft:villager",
    "minecraft:warden",
    "minecraft:sniffer",
    "minecraft:axolotl",
    "minecraft:breeze",
    "minecraft:creaking",
    "minecraft:copper_golem",
    "minecraft:happy_ghast",
];

/// What a `ranged` goal shoots: a skeleton's arrow, a snow golem's snowball,
/// a llama's spit.
pub const PROJECTILES: [&str; 3] = ["arrow", "snowball", "llama_spit"];

fn one() -> f64 {
    1.0
}
fn yes() -> bool {
    true
}
fn twenty() -> u32 {
    20
}
fn forty() -> u32 {
    40
}
fn stroll_interval() -> u32 {
    120
}
fn leap() -> f32 {
    0.4
}
fn ranged_range() -> f32 {
    15.0
}
fn look_distance() -> f32 {
    8.0
}
fn look_probability() -> f32 {
    0.02
}
fn panic_speed() -> f64 {
    1.25
}
fn avoid_distance() -> f32 {
    6.0
}
fn sprint() -> f64 {
    1.2
}
fn target_interval() -> u32 {
    10
}

/// A goal (`goals`). `priority` orders them as vanilla's selector does
/// (lower first, and a lower one interrupts a higher one using the same
/// controls); unset, it is the goal's place in the list.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "goal", rename_all = "snake_case", deny_unknown_fields)]
pub enum Goal {
    /// `FloatGoal`: swims up in water and lava.
    Float { priority: Option<i32> },
    /// `MeleeAttackGoal`: walks to its target and hits it with its
    /// `attack_damage`. `reach` is how far past its own box it hits (vanilla:
    /// about 0.83); `interval` the ticks between hits.
    Melee {
        priority: Option<i32>,
        #[serde(default = "one")]
        speed: f64,
        /// Keeps chasing a target it can't see.
        #[serde(default)]
        follow_unseen: bool,
        reach: Option<f64>,
        #[serde(default = "twenty")]
        interval: u32,
    },
    /// `LeapAtTargetGoal`: jumps at a target 2 to 4 blocks away.
    LeapAtTarget {
        priority: Option<i32>,
        #[serde(default = "leap")]
        velocity_y: f32,
    },
    /// `RangedAttackGoal`: keeps within `range` of its target and shoots a
    /// [`PROJECTILES`] one every `interval` ticks.
    Ranged {
        priority: Option<i32>,
        projectile: String,
        #[serde(default = "one")]
        speed: f64,
        #[serde(default = "forty")]
        interval: u32,
        #[serde(default = "ranged_range")]
        range: f32,
    },
    /// `WaterAvoidingRandomStrollGoal` (`RandomStrollGoal` when
    /// `avoid_water` is false): walks about now and then.
    Wander {
        priority: Option<i32>,
        #[serde(default = "one")]
        speed: f64,
        #[serde(default = "stroll_interval")]
        interval: u32,
        #[serde(default = "yes")]
        avoid_water: bool,
    },
    /// `LookAtPlayerGoal`: looks at a player within `distance`.
    LookAtPlayer {
        priority: Option<i32>,
        #[serde(default = "look_distance")]
        distance: f32,
        #[serde(default = "look_probability")]
        probability: f32,
    },
    /// `RandomLookAroundGoal`.
    RandomLookAround { priority: Option<i32> },
    /// `PanicGoal`: runs after a hit or while burning.
    Panic {
        priority: Option<i32>,
        #[serde(default = "panic_speed")]
        speed: f64,
    },
    /// `TemptGoal`: follows a player holding one of `items` (ids or `#tags`).
    Tempt {
        priority: Option<i32>,
        items: Vec<String>,
        #[serde(default = "one")]
        speed: f64,
        /// Startled by a player moving fast (a chicken is not, a cat is).
        #[serde(default)]
        scared: bool,
    },
    /// `AvoidEntityGoal`: runs from `entities` within `distance`.
    Avoid {
        priority: Option<i32>,
        entities: Vec<String>,
        #[serde(default = "avoid_distance")]
        distance: f32,
        #[serde(default = "one")]
        walk_speed: f64,
        #[serde(default = "sprint")]
        sprint_speed: f64,
    },
    /// `RestrictSunGoal`: keeps out of the sun when it can.
    RestrictSun { priority: Option<i32> },
    /// `FleeSunGoal`: runs to shade while burning in the sun.
    FleeSun {
        priority: Option<i32>,
        #[serde(default = "one")]
        speed: f64,
    },
}

/// A target selector (`targets`), ordered as goals are.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    /// `HurtByTargetGoal`: whoever hurt it; `alert_others` calls the same
    /// kind of entity around to it.
    HurtBy {
        priority: Option<i32>,
        #[serde(default)]
        alert_others: bool,
    },
    /// `NearestAttackableTargetGoal`: the nearest of `entities` it sees,
    /// within its `follow_range`, looked for once in `interval` ticks on
    /// average.
    NearestAttackable {
        priority: Option<i32>,
        entities: Vec<String>,
        #[serde(default = "yes")]
        must_see: bool,
        #[serde(default = "target_interval")]
        interval: u32,
    },
}

impl Goal {
    pub fn priority(&self) -> Option<i32> {
        match self {
            Goal::Float { priority }
            | Goal::Melee { priority, .. }
            | Goal::LeapAtTarget { priority, .. }
            | Goal::Ranged { priority, .. }
            | Goal::Wander { priority, .. }
            | Goal::LookAtPlayer { priority, .. }
            | Goal::RandomLookAround { priority }
            | Goal::Panic { priority, .. }
            | Goal::Tempt { priority, .. }
            | Goal::Avoid { priority, .. }
            | Goal::RestrictSun { priority }
            | Goal::FleeSun { priority, .. } => *priority,
        }
    }
    fn check(&self) -> Result<(), String> {
        match self {
            Goal::Melee {
                speed,
                reach,
                interval,
                ..
            } => {
                check_speed("melee", *speed)?;
                if let Some(r) = reach
                    && !(r.is_finite() && *r > 0.0 && *r <= 64.0)
                {
                    return Err("melee: reach is above 0 and at most 64".into());
                }
                check_interval("melee", *interval)
            }
            Goal::LeapAtTarget { velocity_y, .. } => {
                if !(velocity_y.is_finite() && (0.0..=4.0).contains(velocity_y)) {
                    return Err("leap_at_target: velocity_y is 0 to 4".into());
                }
                Ok(())
            }
            Goal::Ranged {
                projectile,
                speed,
                interval,
                range,
                ..
            } => {
                if !PROJECTILES.contains(&projectile.as_str()) {
                    return Err(format!(
                        "ranged: projectile {projectile:?} is none of {}",
                        PROJECTILES.join(", ")
                    ));
                }
                check_speed("ranged", *speed)?;
                check_interval("ranged", *interval)?;
                check_distance("ranged", "range", *range)
            }
            Goal::Wander {
                speed, interval, ..
            } => {
                check_speed("wander", *speed)?;
                check_interval("wander", *interval)
            }
            Goal::LookAtPlayer {
                distance,
                probability,
                ..
            } => {
                check_distance("look_at_player", "distance", *distance)?;
                if !(probability.is_finite() && (0.0..=1.0).contains(probability)) {
                    return Err("look_at_player: probability is 0 to 1".into());
                }
                Ok(())
            }
            Goal::Panic { speed, .. } | Goal::FleeSun { speed, .. } => {
                check_speed(self.name(), *speed)
            }
            Goal::Tempt { items, speed, .. } => {
                check_speed("tempt", *speed)?;
                if items.is_empty() {
                    return Err("tempt: names no items".into());
                }
                if let Some(i) = items
                    .iter()
                    .find(|i| !crate::entity::resource_id(i.trim_start_matches('#')))
                {
                    return Err(format!("tempt: {i:?} is not an item or #tag"));
                }
                Ok(())
            }
            Goal::Avoid {
                entities,
                distance,
                walk_speed,
                sprint_speed,
                ..
            } => {
                check_entities("avoid", entities)?;
                check_distance("avoid", "distance", *distance)?;
                check_speed("avoid", *walk_speed)?;
                check_speed("avoid", *sprint_speed)
            }
            Goal::Float { .. } | Goal::RandomLookAround { .. } | Goal::RestrictSun { .. } => Ok(()),
        }
    }
    /// The goal's name in a declaration.
    pub fn name(&self) -> &'static str {
        match self {
            Goal::Float { .. } => "float",
            Goal::Melee { .. } => "melee",
            Goal::LeapAtTarget { .. } => "leap_at_target",
            Goal::Ranged { .. } => "ranged",
            Goal::Wander { .. } => "wander",
            Goal::LookAtPlayer { .. } => "look_at_player",
            Goal::RandomLookAround { .. } => "random_look_around",
            Goal::Panic { .. } => "panic",
            Goal::Tempt { .. } => "tempt",
            Goal::Avoid { .. } => "avoid",
            Goal::RestrictSun { .. } => "restrict_sun",
            Goal::FleeSun { .. } => "flee_sun",
        }
    }
}

impl Target {
    pub fn priority(&self) -> Option<i32> {
        match self {
            Target::HurtBy { priority, .. } | Target::NearestAttackable { priority, .. } => {
                *priority
            }
        }
    }
    fn check(&self) -> Result<(), String> {
        match self {
            Target::HurtBy { .. } => Ok(()),
            Target::NearestAttackable {
                entities, interval, ..
            } => {
                check_entities("nearest_attackable", entities)?;
                check_interval("nearest_attackable", *interval)
            }
        }
    }
}

fn check_speed(goal: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 && v <= 16.0 {
        Ok(())
    } else {
        Err(format!("{goal}: a speed is above 0 and at most 16"))
    }
}
fn check_interval(goal: &str, v: u32) -> Result<(), String> {
    if (1..=72_000).contains(&v) {
        Ok(())
    } else {
        Err(format!("{goal}: interval is 1 to 72000 ticks"))
    }
}
fn check_distance(goal: &str, key: &str, v: f32) -> Result<(), String> {
    if v.is_finite() && v > 0.0 && v <= 128.0 {
        Ok(())
    } else {
        Err(format!("{goal}: {key} is above 0 and at most 128"))
    }
}
fn check_entities(goal: &str, entities: &[String]) -> Result<(), String> {
    if entities.is_empty() {
        return Err(format!("{goal}: names no entities"));
    }
    match entities
        .iter()
        .find(|e| !(e.contains(':') && crate::entity::resource_id(e)))
    {
        Some(e) => Err(format!(
            "{goal}: {e:?} is not an entity id (minecraft:player, minecraft:zombie, mymod:ghost)"
        )),
        None => Ok(()),
    }
}

/// What a component value's NBT tag is, for the lists that must hold one
/// kind: numbers (whole ones are ints, unless a list has a fraction),
/// booleans, strings, lists and compounds.
fn value_kind(v: &serde_json::Value) -> Result<u8, String> {
    use serde_json::Value as V;
    match v {
        V::Null => Err("null is no value; leave the component out".into()),
        V::Bool(_) => Ok(0),
        V::Number(_) => Ok(1),
        V::String(_) => Ok(2),
        V::Array(list) => {
            let mut kinds = list.iter().map(value_kind);
            if let Some(first) = kinds.next().transpose()? {
                for k in kinds {
                    if k? != first {
                        return Err("a list holds values of one kind".into());
                    }
                }
            }
            Ok(3)
        }
        V::Object(map) => {
            for v in map.values() {
                value_kind(v)?;
            }
            Ok(4)
        }
    }
}

/// Checks a declaration's `components`: Weft's per-entity components, each
/// entity's own values that start at these and are saved with it.
pub(crate) fn check_components(
    components: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    for (name, value) in components {
        if !(name.contains(':') && crate::entity::resource_id(name)) {
            return Err(format!(
                "component {name:?} is not a namespaced name (mymod:mana)"
            ));
        }
        value_kind(value).map_err(|e| format!("component {name}: {e}"))?;
    }
    Ok(())
}

/// Checks a declaration's `goals` and `targets` for its `template`.
pub(crate) fn check(
    template: &str,
    goals: Option<&[Goal]>,
    targets: Option<&[Target]>,
) -> Result<(), String> {
    if (goals.is_some() || targets.is_some()) && BRAIN_TEMPLATES.contains(&template) {
        return Err(format!(
            "{template} thinks with a brain, not goals: copy a mob that uses goals to declare goals or targets"
        ));
    }
    for g in goals.into_iter().flatten() {
        g.check()?;
        if g.priority().is_some_and(|p| !(-1000..=1000).contains(&p)) {
            return Err(format!("{}: priority is -1000 to 1000", g.name()));
        }
    }
    for t in targets.into_iter().flatten() {
        t.check()?;
        if t.priority().is_some_and(|p| !(-1000..=1000).contains(&p)) {
            return Err("a target's priority is -1000 to 1000".into());
        }
    }
    Ok(())
}
