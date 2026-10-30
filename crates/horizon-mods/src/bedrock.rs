//! Bedrock entity geometry (`*.geo.json`) and animations (`*.animation.json`),
//! the files Blockbench exports for a "Bedrock Entity" model, read into the
//! space of vanilla's Java models (docs/mods.md#custom-entities).
//!
//! The two spaces differ by one flip: a Bedrock file has y up from the feet,
//! a Java model y down from 24 pixels above them, so a point `(x, y, z)` is
//! `(x, 24 - y, z)` in Java. Vanilla's own geometry says so: Bedrock's
//! humanoid right arm pivots at `[-5, 22, 0]`, Java's at `(-5, 2, 0)`; the
//! pig's head at `[0, 12, -6]` and `(0, 12, -6)`. Angles carry over as they
//! are (the chicken's body is `[90, 0, 0]` in Bedrock and `xRot = π/2` in
//! Java; a Bedrock arm turned -90° on x points forward, as a zombie's does).
//! A keyframe's position is `(x, -y, z)` (`KeyframeAnimations.posVec`) and
//! its scale is added as `scale - 1` (`scaleVec`).
//!
//! Boxes are unwrapped as `ModelPart.Cube` does, so a box UV reads the same
//! skin region in both formats. Bones are renamed to vanilla's part names by
//! their role (`rightArm` is `right_arm`, `leg0` a quadruped's
//! `right_hind_leg`...), so the template mob's own animation moves them.
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/// A model part in Java space, laid out like the parts of
/// `entity-models.json`: its pose (x, y, z, xRot, yRot, zRot, and scale
/// 1, 1, 1), quads of five-float vertices (position in pixels, UV 0..1).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub pose: [f32; 9],
    pub quads: Vec<Quad>,
    pub children: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    pub normal: [f32; 3],
    pub vertices: [[f32; 5]; 4],
}

/// A Bedrock geometry in Java space.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The skin size the UVs are laid out on.
    pub texture_size: [u32; 2],
    /// A nameless root holding the top-level bones.
    pub root: Part,
    /// Bedrock bone name → the part name it got (only the renamed ones).
    pub renames: BTreeMap<String, String>,
    /// What was read differently than written (Molang, unknown faces...).
    pub warnings: Vec<String>,
}

/// How an animation ends (`loop` in the file).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loop {
    /// `true`: repeats.
    Repeat,
    /// `false`: plays once, then the bones go back to their poses.
    Once,
    /// `"hold_on_last_frame"`: plays once and keeps the last pose.
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Position,
    Rotation,
    Scale,
}

/// One keyframe, in Java's units (radians, `posVec`, `scaleVec`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    pub pre: [f32; 3],
    pub post: [f32; 3],
    pub catmull: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub target: Target,
    pub keyframes: Vec<Keyframe>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// What follows the last dot of its key: `animation.ghost.idle` is `idle`.
    pub name: String,
    pub length: f32,
    pub looping: Loop,
    pub bones: Vec<(String, Vec<Channel>)>,
}

/// The most of anything a model may hold, so a broken file can't stall the game.
pub const MAX_BONES: usize = 1024;
pub const MAX_QUADS: usize = 65_536;
pub const MAX_KEYFRAMES: usize = 65_536;

fn num(v: &Value, what: &str) -> Result<f32, String> {
    let n = v
        .as_f64()
        .ok_or_else(|| format!("{what} is not a number: {v}"))?;
    if !n.is_finite() {
        return Err(format!("{what} is not finite"));
    }
    Ok(n as f32)
}

fn vec3(v: &Value, what: &str, default: [f32; 3]) -> Result<[f32; 3], String> {
    if v.is_null() {
        return Ok(default);
    }
    let a = v
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or_else(|| format!("{what} is not [x, y, z]"))?;
    Ok([num(&a[0], what)?, num(&a[1], what)?, num(&a[2], what)?])
}

fn vec2(v: &Value, what: &str) -> Result<[f32; 2], String> {
    let a = v
        .as_array()
        .filter(|a| a.len() == 2)
        .ok_or_else(|| format!("{what} is not [u, v]"))?;
    Ok([num(&a[0], what)?, num(&a[1], what)?])
}

/// A bone name with case and `_` ignored.
fn role_key(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// `legN`'s N.
fn leg_index(name: &str) -> Option<usize> {
    role_key(name).strip_prefix("leg")?.parse().ok()
}

/// The part name a bone's role gives it (case and `_` ignored); `legs` is
/// how many `legN` bones the model has: two are a biped's legs, four a
/// quadruped's (`leg2` and `leg3` in front), more a spider's.
fn role_name(name: &str, legs: usize) -> Option<&'static str> {
    const QUADRUPED: [&str; 4] = [
        "right_hind_leg",
        "left_hind_leg",
        "right_front_leg",
        "left_front_leg",
    ];
    const SPIDER: [&str; 8] = [
        "right_hind_leg",
        "left_hind_leg",
        "right_middle_hind_leg",
        "left_middle_hind_leg",
        "right_middle_front_leg",
        "left_middle_front_leg",
        "right_front_leg",
        "left_front_leg",
    ];
    Some(match role_key(name).as_str() {
        "head" => "head",
        "body" => "body",
        "rightarm" => "right_arm",
        "leftarm" => "left_arm",
        "rightleg" => "right_leg",
        "leftleg" => "left_leg",
        _ => {
            let n = leg_index(name)?;
            match legs {
                0..=2 => *["right_leg", "left_leg"].get(n)?,
                3..=4 => *QUADRUPED.get(n)?,
                _ => *SPIDER.get(n)?,
            }
        }
    })
}

struct Bone {
    name: String,
    parent: Option<String>,
    pivot: [f32; 3],
    rotation: [f32; 3],
    quads: Vec<Quad>,
    /// Rotated cubes, each its own part turning about the cube's pivot.
    rotated: Vec<Part>,
}

/// `ModelPart.Cube`'s six faces in Java space. `from` and `to` are relative
/// to the bone's pivot, already flipped (`from` is the low corner). `uv` is
/// the box UV's corner, or per-face rectangles (`[u1, v1, u2, v2]`) by Java
/// direction: down, up, west, north, east, south.
fn cube(
    from: [f32; 3],
    size: [f32; 3],
    grow: f32,
    mirror: bool,
    uv: &CubeUv,
    tex: [f32; 2],
    out: &mut Vec<Quad>,
) {
    let (mut x1, y1, z1) = (from[0] - grow, from[1] - grow, from[2] - grow);
    let (mut x2, y2, z2) = (
        from[0] + size[0] + grow,
        from[1] + size[1] + grow,
        from[2] + size[2] + grow,
    );
    if mirror {
        std::mem::swap(&mut x1, &mut x2);
    }
    let v1 = [x1, y1, z1];
    let v2 = [x2, y1, z1];
    let v3 = [x2, y2, z1];
    let v4 = [x1, y2, z1];
    let v5 = [x1, y1, z2];
    let v6 = [x2, y1, z2];
    let v7 = [x2, y2, z2];
    let v8 = [x1, y2, z2];
    let rects: [Option<[f32; 4]>; 6] = match uv {
        &CubeUv::Box([u, v]) => {
            // Box UVs lay faces out by whole pixels, as Blockbench draws them.
            let [w, h, d] = size.map(f32::floor);
            let (j, k, l, m) = (u, u + d, u + d + w, u + d + w + w);
            let (n, o) = (u + d + w + d, u + d + w + d + w);
            let (p, q, r) = (v, v + d, v + d + h);
            [
                Some([k, p, l, q]),
                Some([l, q, m, p]),
                Some([j, q, k, r]),
                Some([k, q, l, r]),
                Some([l, q, n, r]),
                Some([n, q, o, r]),
            ]
        }
        CubeUv::Faces(faces) => *faces,
    };
    let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
        ([v6, v5, v1, v2], [0.0, -1.0, 0.0]),
        ([v3, v4, v8, v7], [0.0, 1.0, 0.0]),
        ([v1, v5, v8, v4], [-1.0, 0.0, 0.0]),
        ([v2, v1, v4, v3], [0.0, 0.0, -1.0]),
        ([v6, v2, v3, v7], [1.0, 0.0, 0.0]),
        ([v5, v6, v7, v8], [0.0, 0.0, 1.0]),
    ];
    for ((corners, normal), rect) in faces.into_iter().zip(rects) {
        let Some([u1, v1, u2, v2]) = rect else {
            continue;
        };
        // ModelPart.Polygon: the corners take (u2, v1), (u1, v1), (u1, v2), (u2, v2).
        let uvs = [[u2, v1], [u1, v1], [u1, v2], [u2, v2]];
        let mut vertices: [[f32; 5]; 4] = std::array::from_fn(|i| {
            let [x, y, z] = corners[i];
            [x, y, z, uvs[i][0] / tex[0], uvs[i][1] / tex[1]]
        });
        let mut normal = normal;
        if mirror {
            vertices.reverse();
            normal[0] = -normal[0];
        }
        out.push(Quad { normal, vertices });
    }
}

enum CubeUv {
    Box([f32; 2]),
    Faces([Option<[f32; 4]>; 6]),
}

/// The geometry's description and bones: the format of 1.12 and later
/// (`minecraft:geometry`, a list) or the legacy one (`geometry.<name>` keys).
fn find_geometry(doc: &Value) -> Result<(&Value, [u32; 2]), String> {
    let size = |w: &Value, h: &Value| -> Result<[u32; 2], String> {
        let side = |v: &Value| match v.as_u64() {
            Some(n @ 1..=4096) => Ok(n as u32),
            None if v.is_null() => Ok(64),
            _ => Err(format!("texture size {v} is not 1 to 4096")),
        };
        Ok([side(w)?, side(h)?])
    };
    if let Some(list) = doc.get("minecraft:geometry") {
        let g = list
            .as_array()
            .and_then(|l| l.first())
            .ok_or("minecraft:geometry holds no geometry")?;
        let d = &g["description"];
        return Ok((g, size(&d["texture_width"], &d["texture_height"])?));
    }
    let g = doc
        .as_object()
        .and_then(|o| {
            o.iter()
                .find(|(k, _)| k.starts_with("geometry."))
                .map(|(_, v)| v)
        })
        .ok_or("no minecraft:geometry nor geometry.<name> in the file")?;
    Ok((g, size(&g["texturewidth"], &g["textureheight"])?))
}

/// Reads a `.geo.json`.
pub fn read_geometry(bytes: &[u8]) -> Result<Geometry, String> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let (g, texture_size) = find_geometry(&doc)?;
    let tex = texture_size.map(|n| n as f32);
    let list = g["bones"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    if list.len() > MAX_BONES {
        return Err(format!("more than {MAX_BONES} bones"));
    }
    let mut warnings = Vec::new();
    let names: Vec<&str> = list.iter().filter_map(|b| b["name"].as_str()).collect();
    let legs = names.iter().filter(|n| leg_index(n).is_some()).count();
    let mut renames = BTreeMap::new();
    for name in &names {
        if let Some(role) = role_name(name, legs).filter(|r| r != name) {
            renames.insert((*name).to_owned(), role.to_owned());
        }
    }
    let rename = |name: &str| {
        renames
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_owned())
    };
    let mut bones = Vec::with_capacity(list.len());
    let mut quads = 0usize;
    for (i, b) in list.iter().enumerate() {
        let name = b["name"]
            .as_str()
            .ok_or_else(|| format!("bone {i} has no name"))?;
        let what = |field: &str| format!("bone {name}: {field}");
        let pivot = vec3(&b["pivot"], &what("pivot"), [0.0; 3])?;
        let rotation = vec3(&b["rotation"], &what("rotation"), [0.0; 3])?;
        let bone_mirror = b["mirror"].as_bool().unwrap_or(false);
        let bone_inflate = if b["inflate"].is_null() {
            0.0
        } else {
            num(&b["inflate"], &what("inflate"))?
        };
        let mut bone = Bone {
            name: rename(name),
            parent: b["parent"].as_str().map(rename),
            pivot,
            rotation,
            quads: Vec::new(),
            rotated: Vec::new(),
        };
        for (c, cube_v) in b["cubes"].as_array().into_iter().flatten().enumerate() {
            let what = |field: &str| format!("bone {name}, cube {c}: {field}");
            let origin = vec3(&cube_v["origin"], &what("origin"), [0.0; 3])?;
            let size = vec3(&cube_v["size"], &what("size"), [0.0; 3])?;
            if size.iter().any(|s| *s < 0.0) {
                return Err(what("size is negative"));
            }
            let grow = if cube_v["inflate"].is_null() {
                bone_inflate
            } else {
                num(&cube_v["inflate"], &what("inflate"))?
            };
            let mirror = cube_v["mirror"].as_bool().unwrap_or(bone_mirror);
            let uv = match &cube_v["uv"] {
                Value::Null => CubeUv::Box([0.0, 0.0]),
                Value::Array(_) => CubeUv::Box(vec2(&cube_v["uv"], &what("uv"))?),
                Value::Object(faces) => {
                    // Bedrock face → Java polygon: up and down swap with the y flip.
                    const ORDER: [&str; 6] = ["up", "down", "west", "north", "east", "south"];
                    let mut rects = [None; 6];
                    for (face, f) in faces {
                        let Some(slot) = ORDER.iter().position(|o| o == face) else {
                            warnings.push(what(&format!("unknown face {face:?} left out")));
                            continue;
                        };
                        let [u, v] = vec2(&f["uv"], &what(&format!("{face} uv")))?;
                        let [w, h] = if f["uv_size"].is_null() {
                            [0.0, 0.0]
                        } else {
                            vec2(&f["uv_size"], &what(&format!("{face} uv_size")))?
                        };
                        rects[slot] = Some([u, v, u + w, v + h]);
                    }
                    CubeUv::Faces(rects)
                }
                other => return Err(what(&format!("uv is neither [u, v] nor faces: {other}"))),
            };
            // Java space: x and z as they are, y flipped; `from` is the low corner.
            let rotation = vec3(&cube_v["rotation"], &what("rotation"), [0.0; 3])?;
            let (center, out) = if rotation == [0.0; 3] {
                (pivot, &mut bone.quads)
            } else {
                let cube_pivot = vec3(&cube_v["pivot"], &what("pivot"), pivot)?;
                bone.rotated.push(Part {
                    name: format!("{}#cube{c}", bone.name),
                    pose: pose(cube_pivot, Some(pivot), rotation),
                    quads: Vec::new(),
                    children: Vec::new(),
                });
                let last = bone.rotated.len() - 1;
                (cube_pivot, &mut bone.rotated[last].quads)
            };
            let from = [
                origin[0] - center[0],
                center[1] - (origin[1] + size[1]),
                origin[2] - center[2],
            ];
            cube(from, size, grow, mirror, &uv, tex, out);
        }
        if let Some(mesh) = b.get("poly_mesh").filter(|m| !m.is_null()) {
            mesh_quads(mesh, pivot, tex, &mut bone.quads)
                .map_err(|e| format!("bone {name}: poly_mesh: {e}"))?;
        }
        quads += bone.quads.len() + bone.rotated.iter().map(|p| p.quads.len()).sum::<usize>();
        if quads > MAX_QUADS {
            return Err(format!("more than {MAX_QUADS} faces"));
        }
        if bones.iter().any(|o: &Bone| o.name == bone.name) {
            return Err(format!("two bones are called {}", bone.name));
        }
        bones.push(bone);
    }
    // The tree: each bone under its parent, the rest under the root.
    for b in &bones {
        if let Some(p) = &b.parent
            && !bones.iter().any(|o| &o.name == p)
        {
            warnings.push(format!(
                "bone {}: parent {p} not found, kept at the root",
                b.name
            ));
        }
    }
    let by_name: HashMap<String, usize> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name.clone(), i))
        .collect();
    fn build(
        i: usize,
        bones: &[Bone],
        kids: &HashMap<Option<usize>, Vec<usize>>,
        depth: usize,
    ) -> Result<Part, String> {
        if depth > MAX_BONES {
            return Err("bones parent each other in a loop".into());
        }
        let b = &bones[i];
        let parent = b
            .parent
            .as_ref()
            .and_then(|p| bones.iter().find(|o| &o.name == p))
            .map(|p| p.pivot);
        let mut children = b.rotated.clone();
        for &c in kids.get(&Some(i)).map(Vec::as_slice).unwrap_or(&[]) {
            children.push(build(c, bones, kids, depth + 1)?);
        }
        Ok(Part {
            name: b.name.clone(),
            pose: pose(b.pivot, parent, b.rotation),
            quads: b.quads.clone(),
            children,
        })
    }
    let mut kids: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    for (i, b) in bones.iter().enumerate() {
        let parent = b.parent.as_ref().and_then(|p| by_name.get(p).copied());
        kids.entry(parent.filter(|p| *p != i)).or_default().push(i);
    }
    let mut children = Vec::new();
    for &i in kids.get(&None).map(Vec::as_slice).unwrap_or(&[]) {
        children.push(build(i, &bones, &kids, 0)?);
    }
    let reached = count(&children);
    if reached != bones.len() {
        return Err("bones parent each other in a loop".into());
    }
    Ok(Geometry {
        texture_size,
        root: Part {
            name: "root".into(),
            pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            quads: Vec::new(),
            children,
        },
        renames,
        warnings,
    })
}

/// Bones under these parts (rotated cubes not counted).
fn count(parts: &[Part]) -> usize {
    parts
        .iter()
        .filter(|p| !p.name.contains("#cube"))
        .map(|p| 1 + count(&p.children))
        .sum()
}

/// A part's Java pose: its pivot relative to the parent's (the root's
/// pivot is 24 pixels above the feet, y down), and its angles in radians.
fn pose(pivot: [f32; 3], parent: Option<[f32; 3]>, rotation: [f32; 3]) -> [f32; 9] {
    let [x, y, z] = match parent {
        Some(p) => [pivot[0] - p[0], p[1] - pivot[1], pivot[2] - p[2]],
        None => [pivot[0], 24.0 - pivot[1], pivot[2]],
    };
    let r = rotation.map(f32::to_radians);
    [x, y, z, r[0], r[1], r[2], 1.0, 1.0, 1.0]
}

/// A `poly_mesh`'s triangles and quads (positions in model space), relative
/// to the bone's pivot. The y flip mirrors the mesh, so each face's corners
/// are listed backwards to keep it facing out; a triangle repeats its last
/// corner. UVs start at the bottom of the skin, as Bedrock's do.
fn mesh_quads(
    mesh: &Value,
    pivot: [f32; 3],
    tex: [f32; 2],
    out: &mut Vec<Quad>,
) -> Result<(), String> {
    let normalized = mesh["normalized_uvs"].as_bool().unwrap_or(false);
    let list = |key: &str, n: usize| -> Result<Vec<Vec<f32>>, String> {
        mesh[key]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| {
                let a = v
                    .as_array()
                    .filter(|a| a.len() == n)
                    .ok_or(format!("{key} entry {v}"))?;
                a.iter().map(|x| num(x, key)).collect()
            })
            .collect()
    };
    let positions = list("positions", 3)?;
    let normals = list("normals", 3)?;
    let uvs = list("uvs", 2)?;
    let polys = mesh["polys"].as_array().ok_or("no polys")?;
    if polys.len() > MAX_QUADS {
        return Err(format!("more than {MAX_QUADS} faces"));
    }
    for poly in polys {
        let corners = poly
            .as_array()
            .filter(|c| matches!(c.len(), 3 | 4))
            .ok_or("a poly is not 3 or 4 corners")?;
        let mut vertices = Vec::with_capacity(4);
        let mut normal = [0.0f32; 3];
        for corner in corners {
            let idx = |i: usize| -> Result<usize, String> {
                corner[i]
                    .as_u64()
                    .map(|n| n as usize)
                    .ok_or(format!("poly corner {corner}"))
            };
            let p = positions
                .get(idx(0)?)
                .ok_or("a corner's position is out of range")?;
            let n = normals
                .get(idx(1)?)
                .ok_or("a corner's normal is out of range")?;
            let t = uvs.get(idx(2)?).ok_or("a corner's uv is out of range")?;
            normal = [normal[0] + n[0], normal[1] - n[1], normal[2] + n[2]];
            let (u, v) = if normalized {
                (t[0], t[1])
            } else {
                (t[0] / tex[0], t[1] / tex[1])
            };
            vertices.push([
                p[0] - pivot[0],
                pivot[1] - p[1],
                p[2] - pivot[2],
                u,
                1.0 - v,
            ]);
        }
        if vertices.len() == 3 {
            vertices.push(vertices[2]);
        }
        vertices.reverse();
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        let normal = if len > 0.0 {
            normal.map(|c| c / len)
        } else {
            normal
        };
        out.push(Quad {
            normal,
            vertices: [vertices[0], vertices[1], vertices[2], vertices[3]],
        });
    }
    Ok(())
}

/// A keyframe value: `[x, y, z]`, one number for all three, or numbers
/// written as strings. Molang can't be evaluated: it reads as 0 (a warning).
fn value3(v: &Value, what: &str, warnings: &mut Vec<String>) -> Result<[f32; 3], String> {
    let one = |x: &Value, warnings: &mut Vec<String>| -> Result<f32, String> {
        match x {
            Value::Number(_) => num(x, what),
            Value::String(s) => Ok(s
                .trim()
                .parse::<f32>()
                .ok()
                .filter(|f| f.is_finite())
                .unwrap_or_else(|| {
                    warnings.push(format!("{what}: Molang {s:?} is not evaluated, read as 0"));
                    0.0
                })),
            _ => Err(format!("{what}: {x} is not a number")),
        }
    };
    match v {
        Value::Array(a) if a.len() == 3 => Ok([
            one(&a[0], warnings)?,
            one(&a[1], warnings)?,
            one(&a[2], warnings)?,
        ]),
        Value::Array(a) if a.len() == 1 => Ok([one(&a[0], warnings)?; 3]),
        Value::Number(_) | Value::String(_) => Ok([one(v, warnings)?; 3]),
        _ => Err(format!("{what}: {v} is not [x, y, z]")),
    }
}

/// In Java's units: radians, `posVec`, `scaleVec`.
fn to_java(target: Target, v: [f32; 3]) -> [f32; 3] {
    match target {
        Target::Rotation => v.map(f32::to_radians),
        Target::Position => [v[0], -v[1], v[2]],
        Target::Scale => v.map(|s| s - 1.0),
    }
}

fn channel(
    target: Target,
    v: &Value,
    what: &str,
    warnings: &mut Vec<String>,
) -> Result<Channel, String> {
    let key = |k: &Value, time: f32, warnings: &mut Vec<String>| -> Result<Keyframe, String> {
        let (pre, post, catmull) = match k {
            Value::Object(o) => {
                let catmull = o.get("lerp_mode").and_then(Value::as_str) == Some("catmullrom");
                let get =
                    |name: &str, warnings: &mut Vec<String>| -> Result<Option<[f32; 3]>, String> {
                        o.get(name).map(|x| value3(x, what, warnings)).transpose()
                    };
                let vector = get("vector", warnings)?;
                let pre = get("pre", warnings)?;
                let post = get("post", warnings)?;
                let post = post
                    .or(vector)
                    .or(pre)
                    .ok_or(format!("{what}: a keyframe without a value"))?;
                (pre.unwrap_or(post), post, catmull)
            }
            other => {
                let v = value3(other, what, warnings)?;
                (v, v, false)
            }
        };
        Ok(Keyframe {
            time,
            pre: to_java(target, pre),
            post: to_java(target, post),
            catmull,
        })
    };
    let mut keyframes = match v {
        Value::Object(o)
            if !o.contains_key("pre") && !o.contains_key("post") && !o.contains_key("vector") =>
        {
            let mut keys = Vec::with_capacity(o.len());
            for (t, k) in o {
                let time: f32 = t
                    .trim()
                    .parse()
                    .ok()
                    .filter(|t: &f32| t.is_finite() && *t >= 0.0)
                    .ok_or(format!("{what}: {t:?} is not a time in seconds"))?;
                keys.push(key(k, time, warnings)?);
            }
            keys
        }
        other => vec![key(other, 0.0, warnings)?],
    };
    keyframes.sort_by(|a, b| a.time.total_cmp(&b.time));
    Ok(Channel { target, keyframes })
}

/// Reads an `.animation.json`, naming bones as the geometry renamed them.
pub fn read_animations(
    bytes: &[u8],
    renames: &BTreeMap<String, String>,
) -> Result<(Vec<Animation>, Vec<String>), String> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let all = doc["animations"].as_object().ok_or("no animations")?;
    let mut warnings = Vec::new();
    let mut out = Vec::with_capacity(all.len());
    let mut keyframes = 0usize;
    for (key, a) in all {
        let name = key.rsplit('.').next().unwrap_or(key).to_owned();
        let looping = match &a["loop"] {
            Value::Bool(true) => Loop::Repeat,
            Value::String(s) if s == "hold_on_last_frame" => Loop::Hold,
            Value::Bool(false) | Value::Null => Loop::Once,
            other => {
                return Err(format!(
                    "{key}: loop {other} is not true, false or \"hold_on_last_frame\""
                ));
            }
        };
        let mut bones = Vec::new();
        for (bone, channels) in a["bones"].as_object().into_iter().flatten() {
            let mut list = Vec::new();
            for (field, target) in [
                ("rotation", Target::Rotation),
                ("position", Target::Position),
                ("scale", Target::Scale),
            ] {
                if let Some(v) = channels.get(field) {
                    let c = channel(target, v, &format!("{key}: {bone} {field}"), &mut warnings)?;
                    keyframes += c.keyframes.len();
                    list.push(c);
                }
            }
            let bone = renames.get(bone).cloned().unwrap_or_else(|| bone.clone());
            bones.push((bone, list));
        }
        if keyframes > MAX_KEYFRAMES {
            return Err(format!("more than {MAX_KEYFRAMES} keyframes"));
        }
        let last = bones
            .iter()
            .flat_map(|(_, c)| c.iter().flat_map(|c| c.keyframes.iter().map(|k| k.time)))
            .fold(0.0f32, f32::max);
        let length = match &a["animation_length"] {
            Value::Null => last,
            v => num(v, &format!("{key}: animation_length"))?.max(0.0),
        };
        out.push(Animation {
            name,
            length,
            looping,
            bones,
        });
    }
    Ok((out, warnings))
}

#[cfg(test)]
#[path = "bedrock_tests.rs"]
mod tests;
