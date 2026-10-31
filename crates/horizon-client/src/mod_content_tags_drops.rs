//! What declared blocks and items say about tags and drops, put in the
//! content root (docs/mods.md, "Tools, tags and drops"): Weft's `mineable`,
//! `needs_tool`, `tag` and `drops` as vanilla data. Tools and tiers are the
//! vanilla tags the tool components read (`minecraft:mineable/<tool>`,
//! `minecraft:needs_<tier>_tool`), tags are entries in tag files merged as a
//! mod's own tag files are, and drops are a loot table at
//! `data/<ns>/loot_table/blocks/<name>.json` in vanilla's generated forms.
use super::{J, TagMerge};
use crate::Result;
use horizon_mods::content::{Content, Count, DropItem, Drops};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

/// Vanilla tag files listing each block or item directly, by id, for
/// `template_tags`: `kind` is `block` or `item`.
pub(super) struct TemplateTags(BTreeMap<String, Vec<String>>);
impl TemplateTags {
    /// Read only when some declaration asks for its template's tags.
    pub(super) fn read(base: &Path, kind: &str, wanted: bool) -> Result<Self> {
        let mut by_id: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if wanted {
            let rel = format!("data/minecraft/tags/{kind}");
            walk(&base.join(&rel), &rel, &mut by_id)?;
        }
        Ok(TemplateTags(by_id))
    }
    fn of(&self, id: &str) -> &[String] {
        self.0.get(id).map_or(&[], Vec::as_slice)
    }
}

fn walk(dir: &Path, rel: &str, out: &mut BTreeMap<String, Vec<String>>) -> Result<()> {
    let Ok(read) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let rel = format!("{rel}/{name}");
        if path.is_dir() {
            walk(&path, &rel, out)?;
            continue;
        }
        if !name.ends_with(".json") {
            continue;
        }
        let doc: Value = serde_json::from_slice(&fs::read(&path)?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        for v in doc["values"].as_array().into_iter().flatten() {
            let id = v.as_str().or_else(|| v["id"].as_str()).unwrap_or("#");
            if !id.starts_with('#') {
                out.entry(id.to_owned()).or_default().push(rel.clone());
            }
        }
    }
    Ok(())
}

/// A tag id's file: `c:ores/ruby` of `block` is `data/c/tags/block/ores/ruby.json`.
fn tag_file(kind: &str, tag: &str) -> String {
    let (ns, path) = tag.split_once(':').unwrap_or(("minecraft", tag));
    format!("data/{ns}/tags/{kind}/{path}.json")
}

/// Adds one mod's declared blocks and items to the tags they name, after
/// the mod's own tag files, in the order the declarations are read.
pub(super) fn add_tags(
    id: &str,
    content: &Content,
    block_tags: &TemplateTags,
    item_tags: &TemplateTags,
    template_items: &BTreeMap<&str, &str>,
    tags: &mut BTreeMap<String, TagMerge>,
) {
    let mut add = |file: String, value: &str| {
        let ns = file.split('/').nth(1).unwrap_or_default();
        let shared = !content.namespaces.iter().any(|n| n == ns);
        let merge = tags.entry(file.clone()).or_default();
        merge.add(id, &file, false, shared, &[J::Str(value.to_owned())]);
    };
    for block in &content.blocks {
        for tool in &block.mineable {
            add(
                tag_file("block", &format!("mineable/{}", tool.name())),
                &block.id,
            );
        }
        if let Some(tier) = block.tool_tier {
            add(
                tag_file("block", &format!("needs_{}_tool", tier.name())),
                &block.id,
            );
        }
        for tag in &block.tags {
            add(tag_file("block", tag), &block.id);
        }
        if block.template_tags {
            for file in block_tags.of(&block.template) {
                add(file.clone(), &block.id);
            }
            let item = template_items.get(block.id.as_str());
            for file in item.map_or(&[][..], |i| item_tags.of(i)) {
                add(file.clone(), &block.id);
            }
        }
    }
    for item in &content.items {
        for tag in &item.tags {
            add(tag_file("item", tag), &item.id);
        }
        if item.template_tags {
            for file in item_tags.of(&item.template) {
                add(file.clone(), &item.id);
            }
        }
    }
}

/// The loot tables of one mod's blocks that declare `drops`, as (file,
/// JSON). `known` says whether an item id exists in the root.
pub(super) fn drop_tables(
    content: &Content,
    known: impl Fn(&str) -> bool,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for block in &content.blocks {
        let Some(drops) = &block.drops else {
            continue;
        };
        if let Drops::Item(DropItem { item, .. }) = drops
            && !known(item)
        {
            return Err(format!("{}: drops item {item} is no item", block.id).into());
        }
        let (ns, path) = block.id.split_once(':').unwrap_or(("minecraft", &block.id));
        out.push((
            format!("data/{ns}/loot_table/blocks/{path}.json"),
            serde_json::to_vec_pretty(&table(&block.id, drops))?,
        ));
    }
    Ok(out)
}

/// The table vanilla's block loot generator writes for these drops:
/// `dropSelf`, `noDrop`, and `createOreDrop` / clay's with a count.
fn table(id: &str, drops: &Drops) -> Value {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let survives = json!({"type": "minecraft:survives_explosion"});
    let pools = match drops {
        Drops::Nothing => json!([]),
        Drops::Itself => json!([{
            "condition": survives,
            "entries": [{"type": "minecraft:item", "name": id}],
            "rolls": 1
        }]),
        Drops::Item(drop) => {
            let mut modifiers = Vec::new();
            match drop.count {
                Count::Exactly(1) => {}
                Count::Exactly(n) => {
                    modifiers.push(json!({"type": "minecraft:set_count", "count": n}));
                }
                Count::Between([min, max]) => modifiers.push(json!({
                    "type": "minecraft:set_count",
                    "count": {"type": "minecraft:uniform", "max": max, "min": min}
                })),
            }
            if drop.fortune {
                modifiers.push(json!({
                    "type": "minecraft:apply_bonus",
                    "enchantment": "minecraft:fortune",
                    "formula": "minecraft:ore_drops"
                }));
            }
            let mut entry = json!({"type": "minecraft:item", "name": drop.item});
            let mut pool = json!({"rolls": 1});
            // A plain item is kept or lost whole to an explosion
            // (`applyExplosionCondition`); a counted one decays.
            if modifiers.is_empty() {
                if drop.silk_touch {
                    entry["condition"] = survives;
                } else {
                    pool["condition"] = survives;
                }
            } else {
                modifiers.push(json!({"type": "minecraft:explosion_decay"}));
                entry["modifier"] = Value::Array(modifiers);
            }
            pool["entries"] = if drop.silk_touch {
                json!([{
                    "type": "minecraft:alternatives",
                    "children": [
                        {"type": "minecraft:item", "condition": "minecraft:tool/can_silk_touch", "name": id},
                        entry
                    ]
                }])
            } else {
                json!([entry])
            };
            json!([pool])
        }
    };
    json!({
        "type": "minecraft:block",
        "pools": pools,
        "random_sequence": format!("{ns}:blocks/{path}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated tables are the ones vanilla's generator writes for its
    /// own blocks of the same kind.
    #[test]
    fn drop_tables_match_vanilla_forms() {
        let itself = table("ruby:ruby_block", &Drops::Itself);
        assert_eq!(
            itself,
            json!({"type": "minecraft:block", "pools": [{
                "condition": {"type": "minecraft:survives_explosion"},
                "entries": [{"type": "minecraft:item", "name": "ruby:ruby_block"}], "rolls": 1}],
                "random_sequence": "ruby:blocks/ruby_block"})
        );
        assert_eq!(table("ruby:x", &Drops::Nothing)["pools"], json!([]));
        // lapis_ore.json: silk touch, else 4 to 9 with fortune and decay.
        let lapis = table(
            "minecraft:lapis_ore",
            &Drops::Item(DropItem {
                item: "minecraft:lapis_lazuli".into(),
                count: Count::Between([4, 9]),
                fortune: true,
                silk_touch: true,
            }),
        );
        assert_eq!(
            lapis["pools"],
            json!([{"entries": [{"type": "minecraft:alternatives", "children": [
                {"type": "minecraft:item", "condition": "minecraft:tool/can_silk_touch",
                 "name": "minecraft:lapis_ore"},
                {"type": "minecraft:item", "modifier": [
                    {"type": "minecraft:set_count", "count": {"type": "minecraft:uniform", "max": 9, "min": 4}},
                    {"type": "minecraft:apply_bonus", "enchantment": "minecraft:fortune",
                     "formula": "minecraft:ore_drops"},
                    {"type": "minecraft:explosion_decay"}],
                 "name": "minecraft:lapis_lazuli"}]}], "rolls": 1}])
        );
        // A single other item without silk touch is kept whole or lost.
        let single = table(
            "ruby:husk",
            &Drops::Item(DropItem {
                item: "ruby:seed".into(),
                count: Count::Exactly(1),
                fortune: false,
                silk_touch: false,
            }),
        );
        assert_eq!(
            single["pools"],
            json!([{"condition": {"type": "minecraft:survives_explosion"},
                    "entries": [{"type": "minecraft:item", "name": "ruby:seed"}], "rolls": 1}])
        );
    }
}
