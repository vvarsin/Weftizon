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

/// `attack` plays once from each swing; an animation a mod plays takes the
/// place of idle and walk while it plays, once or over and over.
#[test]
fn attack_and_played_animations() {
    let model = br#"{"minecraft:geometry": [{"description": {}, "bones": [{"name": "head", "pivot": [0, 24, 0]}]}]}"#;
    let anims = br#"{"animations": {
        "animation.x.idle": {"loop": true, "animation_length": 1,
            "bones": {"head": {"rotation": {"0": [0, 0, 0], "1": [90, 0, 0]}}}},
        "animation.x.attack": {"animation_length": 0.5,
            "bones": {"head": {"rotation": {"0": [0, 0, 0], "0.5": [0, 0, 30]}}}},
        "animation.x.bow": {"animation_length": 1,
            "bones": {"head": {"position": {"0": [0, 0, 0], "1": [0, 8, 0]}}}}}}"#;
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
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    // `bow` once from age 0: idle waits, then comes back.
    m.play_animation("bow", false);
    assert_eq!(m.extra.animation.as_ref().unwrap().started, 0);
    let at = pose(&m, 10);
    assert!(near(at[1], -4.0) && near(at[3], 0.0), "{at:?}");
    let after = pose(&m, 25);
    assert!(
        near(after[1], 0.0) && near(after[3], 22.5f32.to_radians()),
        "{after:?}"
    );
    // Over and over.
    m.play_animation("bow", true);
    assert!(near(pose(&m, 30)[1], -4.0));
    // An empty name stops it; a name the model has no animation of plays nothing.
    m.play_animation("", false);
    assert!(m.extra.animation.is_none());
    m.play_animation("nope", true);
    assert!(near(pose(&m, 5)[3], 45f32.to_radians() / 2.0));
    m.play_animation("", false);
    // `attack` from the swing, once.
    m.age = 100;
    m.swing();
    assert_eq!(m.anim.swing_started, Some(100));
    assert!(near(pose(&m, 105)[5], 15f32.to_radians()));
    assert!(near(pose(&m, 120)[5], 0.0));
}

/// The payload a server sends its clients reads back as it was written.
#[test]
fn animation_payloads_round_trip() {
    let playing = Playing {
        name: "bow".into(),
        repeat: true,
        started: 40,
    };
    let data = animation_payload(77, Some(&playing), 12);
    assert_eq!(
        read_animation_payload(&data),
        Some((77, Some(("bow".to_owned(), true, 12))))
    );
    assert_eq!(
        read_animation_payload(&animation_payload(5, None, 0)),
        Some((5, None))
    );
    assert_eq!(read_animation_payload(&data[..2]), None);
    // A client times it from its own clock of the entity.
    let kind = crate::mobs::sensor_scan_tests::kind();
    let mut mobs = vec![Mob::new(77, kind, horizon_core::geometry::Vec3::ZERO, 0.0)];
    mobs[0].age = 30;
    assert!(receive_animation(&mut mobs, &data));
    assert_eq!(
        mobs[0].extra.animation,
        Some(Playing {
            name: "bow".into(),
            repeat: true,
            started: 18,
        })
    );
    assert!(receive_animation(
        &mut mobs,
        &animation_payload(77, None, 0)
    ));
    assert!(mobs[0].extra.animation.is_none());
    assert!(!receive_animation(
        &mut mobs,
        &animation_payload(5, Some(&playing), 0)
    ));
}

/// The batches `native_entity_tick` gets: each type of mods' entities with
/// its alive ones, in order; and the hits `native_entity_hurt` gets, taken
/// once, for mods' entities only.
#[test]
fn hook_batches_and_hits() {
    use crate::mobs::{Damage, DamageKind};
    use horizon_core::geometry::Vec3;
    let of = |id: &str| {
        std::sync::Arc::new(crate::mobs::MobType {
            id: id.into(),
            custom: Some(std::sync::Arc::new(Custom {
                id: intern(id),
                ..Default::default()
            })),
            ..(*crate::mobs::sensor_scan_tests::kind()).clone()
        })
    };
    let (ghost, imp, cow) = (
        of("ruby:ghost"),
        of("ruby:imp"),
        crate::mobs::sensor_scan_tests::kind(),
    );
    let mut mobs: Vec<Mob> = [
        (1, &ghost),
        (2, &cow),
        (3, &imp),
        (4, &ghost),
        (5, &ghost),
        (6, &ghost),
    ]
    .into_iter()
    .map(|(id, k)| Mob::new(id, k.clone(), Vec3::ZERO, 0.0))
    .collect();
    mobs[4].health = 0.0;
    mobs[5].removed = true;
    let batches = tick_batches(&mobs);
    assert_eq!(
        batches.into_iter().collect::<Vec<_>>(),
        [("ruby:ghost", vec![1, 4]), ("ruby:imp", vec![3])]
    );
    // Hits: the mod's entities' are kept for the hook, vanilla's are not.
    assert!(mobs[0].damage(3.0, Damage::mob(9, [1.0, 0.0, 0.0])));
    assert!(mobs[1].damage(3.0, Damage::of(DamageKind::Generic)));
    assert!(mobs[2].damage(2.5, Damage::of(DamageKind::Fall)));
    assert!(mobs[1].extra.hurts.is_empty());
    let hits = take_hurts(&mut mobs);
    assert_eq!(hits.len(), 2);
    assert_eq!((hits[0].0, hits[0].1, hits[0].3), ("ruby:ghost", 1, 3.0));
    assert_eq!(hits[0].2.attacker, Some(crate::mobs::Who::Mob(9)));
    assert_eq!((hits[1].0, hits[1].2.kind), ("ruby:imp", DamageKind::Fall));
    assert!(take_hurts(&mut mobs).is_empty(), "heard once");
}

/// Declared component values as NBT.
#[test]
fn component_values_read_as_nbt() {
    let tag = |s: &str| json_tag(&serde_json::from_str(s).unwrap());
    assert_eq!(tag("10"), Some(Tag::Int(10)));
    assert_eq!(tag("5000000000"), Some(Tag::Long(5_000_000_000)));
    assert_eq!(tag("0.5"), Some(Tag::Double(0.5f64.to_bits())));
    assert_eq!(tag("true"), Some(Tag::Byte(1)));
    assert_eq!(tag(r#""x""#), Some(Tag::String("x".into())));
    assert_eq!(
        tag("[1, 2.5]"),
        Some(Tag::List(vec![
            Tag::Double(1f64.to_bits()),
            Tag::Double(2.5f64.to_bits())
        ]))
    );
    assert_eq!(
        tag(r#"{"a": [[1], []]}"#),
        Some(Tag::Compound(
            [(
                "a".into(),
                Tag::List(vec![Tag::List(vec![Tag::Int(1)]), Tag::List(vec![])])
            )]
            .into_iter()
            .collect()
        ))
    );
    assert_eq!(tag("null"), None);
    assert_eq!(tag(r#"[1, "a"]"#), None);
}

/// Per-entity components start at the declared values, are saved under
/// `horizon:components` and read back over the declared ones.
#[test]
fn components_save_with_the_entity() {
    use horizon_core::geometry::Vec3;
    let c = Custom {
        id: intern("ruby:bull"),
        components: [
            ("ruby:mana".into(), Tag::Int(10)),
            ("ruby:owner".into(), Tag::String("".into())),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let kind = std::sync::Arc::new(crate::mobs::MobType {
        id: "ruby:bull".into(),
        custom: Some(std::sync::Arc::new(c)),
        ..(*crate::mobs::sensor_scan_tests::kind()).clone()
    });
    let mut m = Mob::new(3, kind.clone(), Vec3::new(1.0, 64.0, 2.0), 0.0);
    assert_eq!(
        m.extra.components.get(&"ruby:mana".into()),
        Some(&Tag::Int(10))
    );
    m.extra.components.insert("ruby:mana".into(), Tag::Int(7));
    m.extra.components.remove(&"ruby:owner".into());
    m.extra.components.insert("ruby:new".into(), Tag::Byte(1));
    let saved = crate::entity_nbt::mob_to_nbt(&m, [0, 0, 0, 3], None);
    let Some(Tag::Compound(on_disk)) = saved.get(COMPONENTS) else {
        panic!("{saved:?}")
    };
    assert_eq!(on_disk.len(), 2);
    let types = std::collections::HashMap::from([("ruby:bull".to_owned(), kind)]);
    let back = crate::entity_nbt::mob_from_nbt(&saved, &types, 9).unwrap();
    // Its own values; the one it dropped comes back at its declared value.
    assert_eq!(
        back.extra.components,
        [
            ("ruby:mana".into(), Tag::Int(7)),
            ("ruby:new".into(), Tag::Byte(1)),
            ("ruby:owner".into(), Tag::String("".into())),
        ]
        .into_iter()
        .collect()
    );
    // A mob with none writes no key, even over a loaded compound that had one.
    let cow = Mob::new(4, crate::mobs::sensor_scan_tests::kind(), Vec3::ZERO, 0.0);
    let plain = crate::entity_nbt::mob_to_nbt(&cow, [0, 0, 0, 4], Some(&saved));
    assert!(plain.get(COMPONENTS).is_none());
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
