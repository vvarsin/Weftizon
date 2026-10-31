# Making mods for Weftizon

This is the practical guide. The reference pages are [mods.md](mods.md) for the formats,
[native-mods.md](native-mods.md) for code mods, and [ROADMAP.md](../ROADMAP.md)
for the Weftizon modding roadmap and what is still being ported.

There are two kinds of mod:

| Kind | File | What it is | Coding needed |
|---|---|---|---|
| **Content mod** | `.hmod` | Blocks, items, entities, recipes, loot, tags, models, textures, sounds, worldgen data | No code; data and assets only |
| **Native mod** | `.hnmod` | Rust helpers that hook into named game functions | Rust; experimental, full computer access |

A content mod is the safe, portable kind. A native mod is what you use when you need new
behaviour: a key, a HUD, a custom machine tick, a special attack, an event callback.

---

## 1. Quick start: a content mod

Install the mod tool from the fork:

```sh
cargo install --git https://github.com/vvarsin/Weftizon cargo-horizon-mod
cargo horizon-mod new my_mod --name "My Mod"
cd my_mod
cargo horizon-mod run
```

`run` builds `my_mod.hmod`, starts the game with it enabled, and lets you iterate. `build` creates
the file without running the game. `check` runs the same checks the registry and the game run.

A content mod is a folder, not a Cargo project:

```text
my_mod/
  horizon-mod.toml
  assets/
    my_mod/
      lang/en_us.json
      textures/block/my_block.png
      textures/item/my_item.png
      textures/entity/my_mob.png
      models/block/my_block.json
      models/entity/my_mob.geo.json
      animations/entity/my_mob.animation.json
      sounds/entity/my_mob/hurt1.ogg
  data/
    my_mod/
      horizon/block/my_block.json
      horizon/item/my_item.json
      horizon/entity/my_mob.json
      recipe/my_recipe.json
      loot_table/blocks/my_block.json
      loot_table/entities/my_mob.json
```

Names are generated when you do not provide a `lang` key: items get `item.<mod>.<name>`,
blocks get `block.<mod>.<name>`, entities get `entity.<mod>.<name>`, and spawn eggs get
`item.<mod>.<name>_spawn_egg`. Your own `assets/<mod>/lang/en_us.json` value wins. Add it to name
things exactly as you want.

The manifest:

```toml
id = "my_mod"                 # [a-z0-9_]{1,64}; settings key, log name, namespace
name = "My Mod"
version = "0.1.0"
description = "Adds a ghost, a ruby block and a few recipes."
authors = ["you"]
api = "0.2"                   # content mods
side = "both"
capabilities = ["registries"]
# namespaces = ["another_namespace_this_mod_uses"]
```

Mods are off by default. A fresh game with no mods is vanilla. Content mods are read once at
startup, so restart the game after enabling or changing one.

---

## 2. Blocks

Declare a block by copying a vanilla template. The template supplies its behaviour, states,
collision and default textures; you override only what differs.

```json
{
  "template": "minecraft:iron_block",
  "properties": {
    "hardness": 4.0,
    "light": 3,
    "requires_tool": true,
    "sounds": "minecraft:metal"
  },
  "item": true
}
```

Put it in `data/my_mod/horizon/block/my_block.json`. If `item` is true, the game also creates a
block item with the same id.

You can also give a block its own model and texture:

- `assets/my_mod/blockstates/my_block.json`
- `assets/my_mod/models/block/my_block.json`
- `assets/my_mod/textures/block/my_block.png`
- translation: `block.my_mod.my_block = My Block` in `assets/my_mod/lang/en_us.json`

For a block that looks like a different shape, use the declaration's `shapes` or a Blockbench
block model. A content mod block is still a copy of a vanilla block's code; it does not add new
behaviour by itself.

---

## 3. Items

Copy a vanilla item and patch its data components:

```json
{
  "template": "minecraft:iron_sword",
  "components": {
    "minecraft:max_damage": 900,
    "minecraft:attribute_modifiers": [
      {
        "type": "minecraft:attack_damage",
        "id": "minecraft:base_attack_damage",
        "amount": 7.0,
        "operation": "add_value",
        "slot": "mainhand"
      }
    ],
    "!minecraft:repairable": {}
  }
}
```

`minecraft:<type>` sets a component; `!minecraft:<type>` with `{}` removes it. A component type
that no vanilla item has is refused, so a typo fails the load instead of silently doing nothing.

Assets:

- `assets/my_mod/textures/item/my_item.png`
- `assets/my_mod/models/item/my_item.json` for a custom item model
- `item.my_mod.my_item = My Item`
- optional tooltip: `item.my_mod.my_item.tooltip = ...`

---

## 4. Custom entities and mobs

This is the Weftizon custom-entity feature that most resembles Entity Model Features, Entity
Texture Features and Entity Sound Features. A content mod can add a mob by copying a vanilla mob.

```json
{
  "template": "minecraft:zombie",
  "health": 30,
  "width": 0.8,
  "height": 2.0,
  "eye_height": 1.7,
  "speed": 0.28,
  "attack_damage": 4.0,
  "category": "monster",
  "texture": "ghost",
  "variants": ["ghost_blue"],
  "model": "ghost",
  "animations": "ghost",
  "sounds": {
    "ambient": "my_mod:entity.ghost.ambient",
    "hurt": "my_mod:entity.ghost.hurt",
    "death": "my_mod:entity.ghost.death"
  },
  "loot_table": "my_mod:entities/ghost",
  "spawns": [
    { "biomes": ["minecraft:plains", "minecraft:dark_forest"], "weight": 5, "max_count": 2 }
  ]
}
```

Put the declaration in `data/my_mod/horizon/entity/ghost.json`.

Most files are found by the entity's name:

| File | Purpose |
|---|---|
| `assets/my_mod/textures/entity/ghost.png` | Skin |
| `assets/my_mod/textures/entity/ghost_e.png` | Weftizon glow mask: red × alpha marks glowing pixels |
| `assets/my_mod/textures/entity/ghost_blue.png` | Another variant |
| `assets/my_mod/models/entity/ghost.geo.json` | Blockbench **Bedrock Entity** model |
| `assets/my_mod/animations/entity/ghost.animation.json` | Keyframe animations |
| `assets/my_mod/sounds/entity/ghost/ambient*.ogg` | Ambient sounds |
| `assets/my_mod/sounds/entity/ghost/hurt*.ogg` | Hurt sounds |
| `assets/my_mod/sounds/entity/ghost/death*.ogg` | Death sounds |
| `assets/my_mod/sounds/entity/ghost/step*.ogg` | Step sounds |
| `data/my_mod/loot_table/entities/ghost.json` | Its drop table |
| `assets/my_mod/lang/en_us.json` | `entity.my_mod.ghost = Ghost` |

### Blockbench model notes

Export a Blockbench **Bedrock Entity** model as `.geo.json`. The game reads bones with `pivot`,
`parent`, `rotation`, boxes with `origin`, `size`, `uv`, `mirror`, `inflate`, per-face UVs, and
`poly_mesh` bones. Bone names map to roles:

- `head`, `body`
- `rightArm`, `leftArm`
- `rightLeg`, `leftLeg` or `leg0`, `leg1`
- `leg0`..`leg3` for four-legged mobs
- `leg0`..`leg7` for spiders

That means the copied template's own walk/look animation can move your bones if you name them
the same way. A custom model can still keep the template's AI and movement.

Animations are keyframe animations exported by Blockbench. The animation name is the last part
of its key: `animation.ghost.idle` becomes `idle`. The game plays:

- `idle` all the time
- `walk` while walking
- `hurt` after a hit
- `death` while dying

Loop modes are `true`, `false`, and `"hold_on_last_frame"`. Molang is not evaluated yet:
expressions read as 0.

A porting gotcha: a model in your mod namespace must name vanilla parents with the
`minecraft:` prefix. Use `"parent": "minecraft:item/generated"` and
`"parent": "minecraft:block/cube"`, not `"item/generated"` or `"block/cube"`. Without the prefix,
the parent resolves to your namespace (`mymod:item/generated`), which does not exist; the item or
block loads with no error but its model/icon is missing. This was found while testing an
Invasion-Mod port.

Use it with:

```text
/summon my_mod:ghost
```

Worlds save and load it under its own id. A server and its clients need the same mod content;
entity network ids are assigned after vanilla's.

### Current limits

Phase 1 (custom entities) is done. Phase 2 is mostly done: declared AI goals, attack animations,
entity tick/hurt/death/interact hooks, per-entity components and basic solid/kinematic platforms
(`solid`, `kinematic`, `carry_height`) are in; Weft's block decks are still later. A declared entity
can now have its own goals and target selectors.
Spawn eggs are generated early: `"spawn_egg": {"colors": [..]}` creates item `<id>_spawn_egg`,
copying a vanilla egg with the entity's `minecraft:entity_data`; the colours make a simple egg texture
and item model. `"item_model": "mymod:item/egg"` names your own model and texture instead.
Armour/held items on a mod's own model, and the template's extra layers (for example a sheep's
wool on a custom model), are later phases.

---

## 5. Recipes, loot and tags

Recipes and loot use 26.3 data-pack formats.

- Recipe: `data/my_mod/recipe/my_recipe.json`
- Loot table: `data/my_mod/loot_table/blocks/my_block.json` or
  `data/my_mod/loot_table/entities/my_mob.json`
- Loot modification: `data/my_mod/loot_table/blocks/stone.json` with
  `"horizon:add_to": "minecraft:blocks/stone"` and `pools` appends those pools to the vanilla table
- Tags: `data/minecraft/tags/block/mineable/pickaxe.json` to add your block to a vanilla tag
- Convention tags shared by Fabric/NeoForge mods: `data/c/tags/...`

A tag file adds values:

```json
{
  "values": ["my_mod:my_block"]
}
```

If a tag names a block/item that no mod registers, that entry is dropped so the rest of the tag
still loads. Tag merging follows data-pack order, and `"replace": true` behaves like a data pack's
replace.

Recipes can reference another mod's content by tag, which is how mods compose:

```json
{
  "type": "minecraft:crafting_shaped",
  "pattern": ["###", "# #", "###"],
  "key": { "#": "#my_mod:gems" },
  "result": { "item": "my_mod:my_block", "count": 1 }
}
```

---

## 6. Native mods

Use a native mod when you need real code: a key binding, a HUD, a custom command, a machine tick,
an event callback, a special attack, or a client-side cosmetic.

A native mod is a **mixin bundle**:

```text
my_native_mod/
  horizon-mixin.toml
  mixins/my_native_mod/
    my_mod.rs
    data/...
```

The bundle hooks into named game functions with `horizon-mixins` (`HEAD`, `BEFORE_CALL`,
`RETURN`, `MODIFY_ARG`, `OVERWRITE`, etc.). Start from:

<https://github.com/horizon-rust/horizon-native-mod-template>

Build and install it with the launcher:

```sh
horizon-launcher native-build my_native_mod --trust-native-code
```

Native mods are experimental and not sandboxed: they run with the player's access to the
computer, and the launcher builds them from source. They pin a Horizon commit in
`horizon-mixin.toml` or declare the hook API they need in `horizon-native.json`.

### Hook points today

The stable hook catalog is `native-hooks.json` and [native-mods.md](native-mods.md). Current
hook API 15 includes block entities, item use/place/break, tooltips, server commands, menus,
server particles, lifecycle events, server ticks, and item/recipe panel APIs. Phase 2 of the
The Weftizon port is adding entity hooks: tick, hurt, death, interact, animations and per-entity data.

Named hooks are preferred over raw source targets when they exist. A bundle that uses a hook
only needs `"hooks": <API level>`; a bundle that patches arbitrary game code must pin the exact
commit.

### What native mods can do now

- HUD and screen drawing through the native HUD hooks
- Key bindings and client commands
- Server commands and player feedback
- Block entities for a mod's blocks (API 15)
- Item tooltips (API 12)
- Item use/place/break hooks (APIs 13–14)
- Server menus and item exchange (API 10)
- Particles sent to nearby players (API 11)
- World lifecycle and server ticks (APIs 3–4)
- Recipes/item panel APIs (API 2)

This is enough to build a lot of "crazy" mods already, but the full Weftizon SDK-style builder API
(events, questions, chains, attributes, machines, liquids, UI builders) is still being ported.

---

## 7. Testing and checking

For a content mod:

```sh
cargo horizon-mod build
cargo horizon-mod check target/horizon-mod/my_mod.hmod
cargo horizon-mod run
```

`check` reads the file as the registry does: size, SHA-256, manifest, content and signature.
The game refuses malformed declarations at load with the file and reason.

For a native mod, the launcher builds it and caches the build. Test it in an instance with
Native mods enabled and, for server behaviour, with `lan_e2e`.

Rust tests in the repo:

```sh
RUSTUP_HOME=../rustup CARGO_HOME=../cargo-home cargo test -p horizon-mods --lib
RUSTUP_HOME=../rustup CARGO_HOME=../cargo-home cargo test -p horizon-client --lib
```

Vanilla parity harnesses need Java and a Minecraft install; they are not available on every
machine.

---

## 8. Publishing

A mod is published as a GitHub release:

1. Build the `.hmod` or `.hnmod`.
2. Run `cargo horizon-mod check`.
3. Put the source in a public repository with a licence that lets players run it.
4. Publish a GitHub release with the file as an asset. Sign it with minisign if you want the
   registry to pin your key.
5. List the project once at <https://playhorizon.dev/mods/submit>.

For this fork, the registry links still point to the upstream Horizon registry; Weftizon will
need its own registry/index when it publishes its mods.

---

## 9. What to build first

Good first projects:

- A custom mob with a Blockbench model, a new skin, glow mask, sounds and biome spawns.
- A block/item set that uses vanilla templates and tags, with recipes and loot.
- A small native mod that adds a HUD, key, command or block entity tick.
- A resource-pack-style mod that replaces textures, models, sounds and language.

Big projects that need parts not finished yet:

- Energy networks and systems for machines (part 4)
- Liquids, custom terrain, ores and scatter (part 5)
- Screens, HUD builders, packets and settings (part 6)
- CTM, animated blocks, custom shaders and full Blockbench block/item models (part 7)

The roadmap, with what each part has and lacks, is [ROADMAP.md](../ROADMAP.md). If you need a feature that is
not there yet, a native mod can often hook it today.
