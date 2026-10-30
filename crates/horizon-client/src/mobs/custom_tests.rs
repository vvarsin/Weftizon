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
