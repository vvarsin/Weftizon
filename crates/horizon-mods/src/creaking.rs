//! Creaking variants a content mod declares (docs/mods.md#creaking-variants):
//! an entity whose template is `minecraft:creaking` names the logs its heart
//! grows between, and how it differs from the pale oak creaking, in the
//! manner of Grafted Creaking's wood variants (MIT, DialingSpoon527): when it
//! is awake, how a gaze holds it, what it hunts, and what it does besides.
//! With no such declaration a creaking heart keeps vanilla's pale oak rules.
use serde::{Deserialize, Serialize};

/// When a variant's heart is awake and its creaking out.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Active {
    /// Vanilla's: from nightfall to dawn.
    #[default]
    Night,
    /// By day (Grafted's dark oak, which goes away at night).
    Day,
    /// Day and night.
    Always,
}

/// How players' gaze holds it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Freeze {
    /// Vanilla's: it stands still while a player looks its way.
    #[default]
    Gaze,
    /// Once awake, only a player looking straight at it holds it (Grafted's jungle).
    StrictGaze,
    /// It moves only while looked at, and stands still otherwise (Grafted's warped).
    Inverted,
}

/// What it hunts.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Prey {
    /// Vanilla's: players.
    #[default]
    Players,
    /// Nothing: it never wakes on anyone (Grafted's cherry).
    Nothing,
    /// Players and monsters but creepers (Grafted's mushroom).
    PlayersAndMonsters,
    /// Every living thing but creakings and creepers (Grafted's crimson).
    Everything,
}

/// A declaration's `creaking` block.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Creaking {
    /// Blocks or `#tags` a heart between two of them grows this creaking:
    /// `["#minecraft:spruce_logs"]`. The log below the heart (or west, or
    /// north, along its axis) picks the variant.
    pub heart_logs: Vec<String>,
    #[serde(default)]
    pub active: Active,
    #[serde(default)]
    pub freeze: Freeze,
    #[serde(default)]
    pub hunts: Prey,
    /// It walks on water as on ground (Grafted's mangrove).
    #[serde(default)]
    pub water_walk: bool,
    /// A hit on it while its heart protects it calls another of its kind
    /// (Grafted's bamboo).
    #[serde(default)]
    pub multiply_when_hurt: bool,
    /// Players within this many blocks are blinded while it is out
    /// (Grafted's birch: 10).
    #[serde(default)]
    pub blinds_players: Option<f64>,
}

impl Creaking {
    /// Refuses what the game could not use.
    pub fn check(&self, template: &str) -> Result<(), String> {
        if template != "minecraft:creaking" {
            return Err(
                "creaking: only an entity whose template is minecraft:creaking has one".into(),
            );
        }
        if self.heart_logs.is_empty() {
            return Err("creaking: heart_logs names no block or #tag".into());
        }
        if let Some(b) = self
            .heart_logs
            .iter()
            .find(|b| !crate::entity::resource_id(b.trim_start_matches('#')))
        {
            return Err(format!("creaking: heart_logs {b:?} is not a block or #tag"));
        }
        if let Some(r) = self.blinds_players
            && !(r.is_finite() && r > 0.0 && r <= 64.0)
        {
            return Err("creaking: blinds_players is above 0 and at most 64".into());
        }
        Ok(())
    }
}

/// A texture layer drawn over the body as the skin is lit (not full bright):
/// one texture, or frames shown in turn (`frame_ticks` each).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum OverlayFile {
    One(String),
    Frames {
        frames: Vec<String>,
        #[serde(default = "ten")]
        frame_ticks: u32,
    },
}
fn ten() -> u32 {
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> Result<Creaking, String> {
        let c: Creaking = serde_json::from_str(json).map_err(|e| e.to_string())?;
        c.check("minecraft:creaking")?;
        Ok(c)
    }

    #[test]
    fn a_variant_names_its_logs_and_keeps_vanilla_defaults() {
        let c = read(r##"{"heart_logs": ["#minecraft:spruce_logs"]}"##).unwrap();
        assert_eq!(
            (
                c.active,
                c.freeze,
                c.hunts,
                c.water_walk,
                c.multiply_when_hurt,
                c.blinds_players
            ),
            (
                Active::Night,
                Freeze::Gaze,
                Prey::Players,
                false,
                false,
                None
            )
        );
        let c = read(
            r##"{"heart_logs": ["minecraft:warped_stem"], "active": "always", "freeze": "inverted",
                "hunts": "everything", "water_walk": true, "multiply_when_hurt": true, "blinds_players": 10}"##,
        )
        .unwrap();
        assert_eq!(
            (c.active, c.freeze, c.hunts),
            (Active::Always, Freeze::Inverted, Prey::Everything)
        );
        assert_eq!(c.blinds_players, Some(10.0));
    }

    #[test]
    fn bad_variants_are_refused() {
        assert!(read(r#"{"heart_logs": []}"#).is_err());
        assert!(read(r#"{"heart_logs": ["Not A Block"]}"#).is_err());
        assert!(read(r#"{"heart_logs": ["oak_log"], "blinds_players": 0}"#).is_err());
        assert!(read(r#"{"heart_logs": ["oak_log"], "hunts": "cows"}"#).is_err());
        let c: Creaking = serde_json::from_str(r#"{"heart_logs": ["oak_log"]}"#).unwrap();
        assert!(c.check("minecraft:zombie").is_err());
    }
}
