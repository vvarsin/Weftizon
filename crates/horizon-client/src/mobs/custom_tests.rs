use super::*;

#[test]
fn mob_sounds_keep_a_mods_namespace() {
    assert_eq!(
        sound_id("entity.zombie.hurt"),
        "minecraft:entity.zombie.hurt"
    );
    assert_eq!(
        sound_id("ghosts:entity.ghost.hurt"),
        "ghosts:entity.ghost.hurt"
    );
    let a = intern("ghosts:entity.ghost.hurt");
    let b = intern(&String::from("ghosts:entity.ghost.hurt"));
    assert!(std::ptr::eq(a, b), "one copy per name");
}

#[test]
fn entries_read_with_defaults() {
    let dir = std::env::temp_dir().join(format!("horizon-custom-entries-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(entries(&dir).is_empty());
    std::fs::write(
        dir.join(FILE),
        r#"[{"id": "a:b", "template": "minecraft:zombie", "emissive": null, "model": null,
             "animations": null, "loot_table": null}]"#,
    )
    .unwrap();
    let e = entries(&dir);
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].scale, e[0].textures.len()), (1.0, 0));
    std::fs::write(dir.join(FILE), "not json").unwrap();
    assert!(
        entries(&dir).is_empty(),
        "a broken file is no entities, not a crash"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn bedrock_animations_play_by_their_loop_mode() {
    let model = br#"{"minecraft:geometry": [{"description": {}, "bones": [{"name": "head", "pivot": [0, 24, 0]}]}]}"#;
    let anims = br#"{"animations": {
        "animation.x.idle": {"loop": true, "animation_length": 1,
            "bones": {"head": {"rotation": {"0": [0, 0, 0], "1": [90, 0, 0]}}}},
        "animation.x.hurt": {"bones": {"head": {"position": {"0": [0, 0, 0], "0.5": [0, 8, 0]}}}}}}"#;
    let g = bedrock::read_geometry(model).unwrap();
    let (list, _) = bedrock::read_animations(anims, &g.renames).unwrap();
    let c = Custom {
        own_model: true,
        scale: 1.0,
        animations: list
            .iter()
            .map(|a| (a.name.clone(), (animation(a), a.looping)))
            .collect(),
        ..Default::default()
    };
    let part = part(&g.root);
    let kind = std::sync::Arc::new(crate::mobs::MobType {
        custom: Some(std::sync::Arc::new(c.clone())),
        ..(*crate::mobs::sensor_scan_tests::kind()).clone()
    });
    let mut m = Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0);
    let pose = |m: &Mob, age: u32| {
        let mut m = m.clone();
        m.age = age;
        let s = anim::state(&m, 0.0);
        let mut p = anim::Poses::new(&part);
        c.animate(&m, &s, &mut p);
        *p.get("head")
    };
    // Half way, then wrapped round to the start.
    assert!((pose(&m, 10)[3] - 45f32.to_radians()).abs() < 1e-4);
    assert!(pose(&m, 20)[3].abs() < 1e-4);
    // `hurt` plays once after a hit: 8 pixels up is -8 in Java's y.
    m.hurt_time = 0;
    assert_eq!(pose(&m, 0)[1], 0.0);
    m.hurt_time = 1;
    let after = pose(&m, 0)[1];
    assert!((after + 7.2).abs() < 1e-3, "{after}");
    assert_eq!(c.skin_index(&m), 0);
}

/// A Custom with the goals and targets of `json` (`{"goals": [..], "targets": [..]}`).
fn declared(json: &str) -> Custom {
    #[derive(serde::Deserialize)]
    struct Decl {
        goals: Option<Vec<behaviour::Goal>>,
        targets: Option<Vec<behaviour::Target>>,
    }
    let d: Decl = serde_json::from_str(json).unwrap();
    let tag = |t: &str| match t {
        "minecraft:cow_food" => vec!["minecraft:wheat".to_owned()],
        _ => Vec::new(),
    };
    let (goals, targets) = plan("ruby:bull", d.goals.as_deref(), d.targets.as_deref(), &tag);
    Custom {
        id: intern("ruby:bull"),
        scale: 1.0,
        goals,
        targets,
        ..Default::default()
    }
}

fn mob_of(c: Custom, id: u32, at: horizon_core::geometry::Vec3) -> Mob {
    let kind = std::sync::Arc::new(crate::mobs::MobType {
        id: c.id.into(),
        custom: Some(std::sync::Arc::new(c)),
        ..(*crate::mobs::sensor_scan_tests::kind()).clone()
    });
    Mob::new(id, kind, at, 0.0)
}

/// Declared goals and targets take the template's place, in their
/// priorities; a declaration without them keeps the template's.
#[test]
fn declared_goals_replace_the_templates() {
    let zero = horizon_core::geometry::Vec3::ZERO;
    let kept = mob_of(declared("{}"), 1, zero);
    assert!(kept.ai.goals.has("Panic") && kept.ai.goals.has("Breed"));
    let c = declared(
        r##"{"goals": [
            {"goal": "float"},
            {"goal": "melee", "reach": 2.0},
            {"goal": "ranged", "projectile": "snowball"},
            {"goal": "wander"},
            {"goal": "tempt", "items": ["#minecraft:cow_food", "minecraft:carrot"]},
            {"goal": "avoid", "entities": ["minecraft:player", "minecraft:wolf", "ruby:ghost"]},
            {"goal": "look_at_player", "priority": 9}
        ],
        "targets": [
            {"target": "hurt_by", "alert_others": true},
            {"target": "nearest_attackable", "entities": ["minecraft:player", "ruby:ghost"]}
        ]}"##,
    );
    let goals = c.goals.as_ref().unwrap();
    assert_eq!(
        goals.iter().map(|p| p.priority).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 9]
    );
    assert_eq!(
        &*goals[4].items,
        ["minecraft:carrot".to_owned(), "minecraft:wheat".to_owned()]
    );
    assert!(goals[5].player);
    assert_eq!(goals[5].kinds.species, [crate::mobs::Species::Wolf]);
    assert_eq!(goals[5].kinds.declared, ["ruby:ghost"]);
    let m = mob_of(c, 2, zero);
    assert!(!m.ai.goals.has("Panic") && !m.ai.goals.has("Breed"));
    for name in [
        "Float",
        "MeleeAttack",
        "RangedAttack",
        "Stroll",
        "Tempt",
        "AvoidEntity",
        "LookAtPlayer",
    ] {
        assert!(m.ai.goals.has(name), "{name}");
    }
    assert!(m.ai.targets.has("HurtByTarget") && m.ai.targets.has("NearestTarget"));
    // FloatGoal lets the navigation float, as for vanilla's floaters.
    assert!(m.ai.nav.can_float);
}

/// `melee` hits from its `reach` with the declared damage, and its
/// `attack_knockback` pushes the target.
#[test]
fn declared_melee_hits_from_its_reach() {
    use crate::mobs::{MobEvent, Who, goals::Cx};
    use horizon_core::{geometry::Vec3, random::LegacyRandom};
    let zombie = Mob::new(
        10,
        crate::mobs::sensor_scan_tests::kind(),
        Vec3::new(2.2, 64.0, 0.0),
        0.0,
    )
    .seen(None);
    let others = [zombie];
    let hits = |reach: f64| {
        let mut hit = None;
        crate::mobs::world::with_test_world(&others, |w, _| {
            let json = format!(r#"{{"goals": [{{"goal": "melee", "reach": {reach}}}]}}"#);
            let c = declared(&json);
            let kind = std::sync::Arc::new(crate::mobs::MobType {
                attack_damage: 4.0,
                attack_knockback: 2.0,
                id: c.id.into(),
                custom: Some(std::sync::Arc::new(c)),
                ..(*crate::mobs::sensor_scan_tests::kind()).clone()
            });
            let mut m = Mob::new(1, kind, Vec3::new(0.0, 64.0, 0.0), 0.0);
            m.ai.target = Some(Who::Mob(10));
            let mut lr = LegacyRandom::new(5);
            let mut events = Vec::new();
            for _ in 0..40 {
                let mut goals = std::mem::take(&mut m.ai.goals);
                goals.tick(
                    &mut m,
                    &mut Cx {
                        w,
                        lr: &mut lr,
                        events: &mut events,
                    },
                );
                m.ai.goals = goals;
            }
            let amount = events.iter().find_map(|e| match e {
                MobEvent::AttackMob {
                    target: 10, amount, ..
                } => Some(*amount),
                _ => None,
            });
            let push = events.iter().find_map(|e| match e {
                MobEvent::Push {
                    target: Who::Mob(10),
                    velocity,
                } => Some(velocity[0]),
                _ => None,
            });
            hit = Some((amount, push, m.ai.swinging));
        });
        hit
    };
    let Some(far) = hits(2.0) else {
        eprintln!("skipped: no extracted vanilla data");
        return;
    };
    // 2.2 blocks apart, cows 0.9 wide: 1.3 between the boxes.
    let (amount, push, swung) = far;
    assert_eq!(amount, Some(4.0));
    assert!(swung, "the hit swings");
    // (0 enchantment + 2 attack_knockback) * 0.5 along +x.
    assert!(push.is_some_and(|x| (x - 1.0).abs() < 1e-9), "{push:?}");
    // Vanilla's reach (about 0.83) is short of it.
    assert_eq!(hits(0.5).unwrap().0, None);
}

/// `nearest_attackable` names kinds: a mod's entity by its id, a vanilla
/// mob by its own, never a mod's entity copying it.
#[test]
fn declared_targets_hunt_by_kind() {
    use crate::mobs::goals::Cx;
    use horizon_core::{geometry::Vec3, random::LegacyRandom};
    let zombie_kind = std::sync::Arc::new(crate::mobs::MobType {
        id: "minecraft:zombie".into(),
        species: crate::mobs::Species::Zombie,
        ..(*crate::mobs::sensor_scan_tests::kind()).clone()
    });
    let ghost = Custom {
        id: intern("ruby:ghost"),
        ..Default::default()
    };
    let ghost_kind = std::sync::Arc::new(crate::mobs::MobType {
        id: "ruby:ghost".into(),
        custom: Some(std::sync::Arc::new(ghost)),
        ..(*zombie_kind).clone()
    });
    let zombie = Mob::new(10, zombie_kind, Vec3::new(4.0, 64.0, 0.0), 0.0).seen(None);
    let ghost = Mob::new(11, ghost_kind, Vec3::new(0.0, 64.0, 4.0), 0.0).seen(None);
    assert_eq!(
        (zombie.declared, ghost.declared),
        (None, Some("ruby:ghost"))
    );
    let others = [zombie, ghost];
    for (hunts, wanted) in [("ruby:ghost", 11), ("minecraft:zombie", 10)] {
        let json = format!(
            r#"{{"targets": [{{"target": "nearest_attackable", "entities": ["{hunts}"]}}]}}"#
        );
        let ran = crate::mobs::world::with_test_world(&others, |w, _| {
            let mut m = mob_of(declared(&json), 1, Vec3::new(0.0, 64.0, 0.0));
            let mut lr = LegacyRandom::new(5);
            let mut events = Vec::new();
            for _ in 0..200 {
                let mut targets = std::mem::take(&mut m.ai.targets);
                targets.tick(
                    &mut m,
                    &mut Cx {
                        w,
                        lr: &mut lr,
                        events: &mut events,
                    },
                );
                m.ai.targets = targets;
                if m.ai.target.is_some() {
                    break;
                }
            }
            assert_eq!(m.ai.target, Some(crate::mobs::Who::Mob(wanted)), "{hunts}");
        });
        if !ran {
            eprintln!("skipped: no extracted vanilla data");
            return;
        }
    }
}
