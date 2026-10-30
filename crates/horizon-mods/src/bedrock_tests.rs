use super::*;

fn geo(bones: &str) -> Vec<u8> {
    format!(
        r#"{{"format_version": "1.12.0", "minecraft:geometry": [{{
            "description": {{"identifier": "geometry.test", "texture_width": 64, "texture_height": 64}},
            "bones": {bones}}}]}}"#
    )
    .into_bytes()
}

fn part<'a>(p: &'a Part, name: &str) -> &'a Part {
    fn find<'a>(p: &'a Part, name: &str) -> Option<&'a Part> {
        if p.name == name {
            return Some(p);
        }
        p.children.iter().find_map(|c| find(c, name))
    }
    find(p, name).unwrap_or_else(|| panic!("no part {name}"))
}

/// Bedrock's `geometry.humanoid` right arm is Java's `HumanoidModel` right
/// arm: offset (-5, 2, 0), `texOffs(40, 16).addBox(-3, -2, -2, 4, 12, 4)`.
#[test]
fn a_bedrock_arm_is_vanillas_java_arm() {
    let g = read_geometry(&geo(r#"[{"name": "rightArm", "pivot": [-5, 22, 0],
            "cubes": [{"origin": [-8, 12, -2], "size": [4, 12, 4], "uv": [40, 16]}]}]"#))
    .unwrap();
    assert_eq!(g.texture_size, [64, 64]);
    let arm = part(&g.root, "right_arm");
    assert_eq!(arm.pose, [-5.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(arm.quads.len(), 6);
    // ModelPart.Cube's DOWN polygon: v6, v5, v1, v2 with (k, p, l, q) = (44, 16, 48, 20).
    let down = &arm.quads[0];
    assert_eq!(down.normal, [0.0, -1.0, 0.0]);
    let uv = |u: f32, v: f32| [u / 64.0, v / 64.0];
    let corner = |q: &Quad, i: usize| q.vertices[i];
    assert_eq!(
        corner(down, 0),
        [1.0, -2.0, 2.0, uv(48.0, 16.0)[0], uv(48.0, 16.0)[1]]
    );
    assert_eq!(
        corner(down, 1),
        [-3.0, -2.0, 2.0, uv(44.0, 16.0)[0], uv(44.0, 16.0)[1]]
    );
    assert_eq!(
        corner(down, 2),
        [-3.0, -2.0, -2.0, uv(44.0, 20.0)[0], uv(44.0, 20.0)[1]]
    );
    assert_eq!(
        corner(down, 3),
        [1.0, -2.0, -2.0, uv(48.0, 20.0)[0], uv(48.0, 20.0)[1]]
    );
    // SOUTH: (n, q, o, r) = (52, 20, 56, 32).
    let south = &arm.quads[5];
    assert_eq!(south.normal, [0.0, 0.0, 1.0]);
    assert_eq!(corner(south, 0)[3..], uv(56.0, 20.0));
    assert_eq!(corner(south, 2)[3..], uv(52.0, 32.0));
    assert_eq!(g.renames["rightArm"], "right_arm");
}

/// The chicken's body: Bedrock `[90, 0, 0]` about `[0, 8, 0]`, Java
/// `offsetAndRotation(0, 16, 0, π/2, 0, 0)` with `addBox(-3, -4, -3, 6, 8, 6)`.
#[test]
fn angles_carry_over_and_children_pivot_from_their_parent() {
    let g = read_geometry(&geo(
        r#"[{"name": "body", "pivot": [0, 8, 0], "rotation": [90, 0, 0],
              "cubes": [{"origin": [-3, 4, -3], "size": [6, 8, 6], "uv": [0, 9]}]},
            {"name": "tail", "parent": "body", "pivot": [0, 10, 4], "rotation": [0, 0, -15]}]"#,
    ))
    .unwrap();
    let body = part(&g.root, "body");
    assert_eq!(body.pose[..3], [0.0, 16.0, 0.0]);
    assert!((body.pose[3] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    let ys: Vec<f32> = body
        .quads
        .iter()
        .flat_map(|q| q.vertices.map(|v| v[1]))
        .collect();
    assert_eq!(ys.iter().copied().fold(f32::MAX, f32::min), -4.0);
    assert_eq!(ys.iter().copied().fold(f32::MIN, f32::max), 4.0);
    let tail = part(&g.root, "tail");
    assert_eq!(tail.pose[..3], [0.0, -2.0, 4.0]);
    assert!((tail.pose[5] + 15f32.to_radians()).abs() < 1e-6);
}

#[test]
fn mirror_inflate_and_rotated_cubes() {
    let g = read_geometry(&geo(
        r#"[{"name": "leftArm", "pivot": [5, 22, 0], "mirror": true,
            "cubes": [{"origin": [4, 12, -2], "size": [4, 12, 4], "uv": [40, 16], "inflate": 0.5},
                      {"origin": [4, 20, -1], "size": [2, 2, 2], "uv": [0, 0],
                       "pivot": [5, 21, 0], "rotation": [0, 45, 0], "mirror": false}]}]"#,
    ))
    .unwrap();
    let arm = part(&g.root, "left_arm");
    assert_eq!(arm.quads.len(), 6);
    // Mirrored: the corners reversed and the normal's x turned around.
    let west = &arm.quads[2];
    assert_eq!(west.normal, [1.0, 0.0, 0.0]);
    let xs: Vec<f32> = arm
        .quads
        .iter()
        .flat_map(|q| q.vertices.map(|v| v[0]))
        .collect();
    assert_eq!(xs.iter().copied().fold(f32::MAX, f32::min), -1.5);
    assert_eq!(xs.iter().copied().fold(f32::MIN, f32::max), 3.5);
    let rotated = part(&g.root, "left_arm#cube1");
    assert_eq!(rotated.pose[..3], [0.0, 1.0, 0.0]);
    assert!((rotated.pose[4] - 45f32.to_radians()).abs() < 1e-6);
    assert_eq!(rotated.quads.len(), 6);
}

#[test]
fn per_face_uvs_land_on_their_faces() {
    let g = read_geometry(&geo(
        r#"[{"name": "box", "pivot": [0, 0, 0],
            "cubes": [{"origin": [0, 0, 0], "size": [2, 2, 2],
              "uv": {"up": {"uv": [8, 0], "uv_size": [2, 2]}, "north": {"uv": [16, 8], "uv_size": [2, 2]}}}]}]"#,
    ))
    .unwrap();
    let b = part(&g.root, "box");
    assert_eq!(b.quads.len(), 2);
    // Bedrock's top is Java's DOWN polygon (y flipped), drawn first.
    assert_eq!(b.quads[0].normal, [0.0, -1.0, 0.0]);
    assert_eq!(b.quads[0].vertices[1][3..], [8.0 / 64.0, 0.0]);
    assert_eq!(b.quads[1].normal, [0.0, 0.0, -1.0]);
}

#[test]
fn legacy_files_and_leg_roles() {
    let file =
        br#"{"format_version": "1.8.0", "geometry.pig": {"texturewidth": 64, "textureheight": 32,
        "bones": [{"name": "leg0", "pivot": [-3, 6, 7]}, {"name": "leg1", "pivot": [3, 6, 7]},
                  {"name": "leg2", "pivot": [-3, 6, -5]}, {"name": "leg3", "pivot": [3, 6, -5]},
                  {"name": "head", "pivot": [0, 12, -6]}]}}"#;
    let g = read_geometry(file).unwrap();
    assert_eq!(g.texture_size, [64, 32]);
    assert_eq!(part(&g.root, "right_hind_leg").pose[..3], [-3.0, 18.0, 7.0]);
    assert_eq!(part(&g.root, "left_front_leg").pose[..3], [3.0, 18.0, -5.0]);
    assert_eq!(part(&g.root, "head").pose[..3], [0.0, 12.0, -6.0]);
    let two = read_geometry(&geo(r#"[{"name": "leg0"}, {"name": "Leg_1"}]"#)).unwrap();
    assert_eq!(two.renames["leg0"], "right_leg");
    assert_eq!(two.renames["Leg_1"], "left_leg");
    let eight: Vec<String> = (0..8).map(|i| format!(r#"{{"name": "leg{i}"}}"#)).collect();
    let spider = read_geometry(&geo(&format!("[{}]", eight.join(",")))).unwrap();
    assert_eq!(spider.renames["leg4"], "right_middle_front_leg");
}

#[test]
fn poly_meshes_and_broken_files() {
    let g = read_geometry(&geo(
        r#"[{"name": "wing", "pivot": [0, 10, 0], "poly_mesh": {"normalized_uvs": true,
            "positions": [[0, 10, 0], [4, 10, 0], [0, 14, 0]], "normals": [[0, 0, -1]],
            "uvs": [[0, 0], [1, 0], [0, 1]], "polys": [[[0, 0, 0], [1, 0, 1], [2, 0, 2]]]}}]"#,
    ))
    .unwrap();
    let wing = part(&g.root, "wing");
    assert_eq!(wing.quads.len(), 1);
    let q = &wing.quads[0];
    // Listed backwards, the triangle's last corner repeated, y flipped.
    assert_eq!(q.vertices[0], [0.0, -4.0, 0.0, 0.0, 0.0]);
    assert_eq!(q.vertices[1], q.vertices[0]);
    assert_eq!(q.vertices[3], [0.0, 0.0, 0.0, 0.0, 1.0]);
    assert_eq!(q.normal, [0.0, 0.0, -1.0]);
    for (bones, why) in [
        (
            r#"[{"name": "a", "parent": "b"}, {"name": "b", "parent": "a"}]"#,
            "loop",
        ),
        (r#"[{"name": "a"}, {"name": "a"}]"#, "two bones"),
        (
            r#"[{"name": "a", "cubes": [{"size": [-1, 1, 1]}]}]"#,
            "negative",
        ),
        (r#"[{"name": "a", "pivot": [0, "x", 0]}]"#, "not a number"),
    ] {
        let e = read_geometry(&geo(bones)).unwrap_err();
        assert!(e.contains(why), "{bones}: {e}");
    }
    assert!(read_geometry(b"{}").is_err());
}

#[test]
fn animations_read_in_javas_units() {
    let renames = BTreeMap::from([("rightArm".to_owned(), "right_arm".to_owned())]);
    let file = br#"{"format_version": "1.8.0", "animations": {
        "animation.ghost.idle": {"loop": true, "animation_length": 2,
            "bones": {"rightArm": {"rotation": {"0.0": [-90, 0, 0], "1.0": {"pre": [0, 10, 0], "post": [0, 20, 0], "lerp_mode": "catmullrom"}},
                                   "position": [1, 2, 3], "scale": 2}}},
        "animation.ghost.bow": {"loop": "hold_on_last_frame",
            "bones": {"head": {"rotation": {"0": [0, 0, 0], "0.4": ["40", 0, "math.sin(q.anim_time)"]}}}},
        "animation.ghost.wave": {"bones": {}}}}"#;
    let (anims, warnings) = read_animations(file, &renames).unwrap();
    let get = |n: &str| anims.iter().find(|a| a.name == n).unwrap();
    let idle = get("idle");
    assert_eq!((idle.length, idle.looping), (2.0, Loop::Repeat));
    let (bone, channels) = &idle.bones[0];
    assert_eq!(bone, "right_arm");
    let rot = &channels[0];
    assert_eq!(rot.target, Target::Rotation);
    assert!((rot.keyframes[0].post[0] + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert!(rot.keyframes[1].catmull);
    assert!((rot.keyframes[1].pre[1] - 10f32.to_radians()).abs() < 1e-6);
    assert!((rot.keyframes[1].post[1] - 20f32.to_radians()).abs() < 1e-6);
    assert_eq!(channels[1].keyframes[0].post, [1.0, -2.0, 3.0]);
    assert_eq!(channels[2].keyframes[0].post, [1.0, 1.0, 1.0]);
    let bow = get("bow");
    assert_eq!((bow.looping, bow.length), (Loop::Hold, 0.4));
    assert!((bow.bones[0].1[0].keyframes[1].post[0] - 40f32.to_radians()).abs() < 1e-6);
    assert_eq!(bow.bones[0].1[0].keyframes[1].post[2], 0.0);
    assert!(
        warnings.iter().any(|w| w.contains("Molang")),
        "{warnings:?}"
    );
    assert_eq!(get("wave").looping, Loop::Once);
    assert!(read_animations(br#"{"animations": {"a": {"loop": 3}}}"#, &renames).is_err());
}
