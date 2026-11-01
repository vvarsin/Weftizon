# Horizon mods

Two kinds of mod run in Horizon:

| Kind | File | What it is | Where it runs |
|---|---|---|---|
| **Content mod** | `.hmod` | Data and assets: blocks, items, entities, recipes, loot, tags, models, textures, structures. No code. | Read when the game starts |
| **Native mod** | `.hnmod` | A [horizon-mixins](https://github.com/horizon-rust/horizon-mixins) bundle: Rust that changes the game's own source before it is compiled. | In a game the launcher builds for the player ([native-mods.md](native-mods.md)) |

Fabric and NeoForge jars do **not** run as they are: their code calls and patches Minecraft's Java classes, and
Horizon has none. What carries over is the data and assets inside a jar ([modpacks.md](modpacks.md)).

Mods are **off by default**. A fresh install has no `horizon-mods` folder and plays exactly like vanilla 26.3.

## Playing with mods

1. Install a mod from the launcher's Mods page ([mod-browser.md](mod-browser.md)), or put its files by hand in the
   game folder (the one holding `options.txt`): a content mod's `.hmod` (or a folder holding `horizon-mod.toml`) in
   `horizon-mods/`. Native mods go in `horizon-native-mods/`, and the launcher builds them
   ([native-mods.md](native-mods.md)).
2. Options > **Mods** lists them with what each may do ("May add blocks, items and mob effects to your worlds") and a
   switch. They start off. The choice is kept in `horizon-mods.txt` (`mod.<id>:true`).
3. A content mod's blocks, items and mob effects load once, when the game starts: restart the game after a switch.
4. A mod that can't load shows why, in red, on its row.

Two turned-on content mods never own the same namespace: the first by file name loads, and the other stays out with
the reason in the log.

Benchmarks (`--benchmark`) never load mods.

## Making a content mod

```sh
cargo install --git https://github.com/horizon-rust/horizon cargo-horizon-mod
cargo horizon-mod new my-mod        # a content mod: manifest, one block with its model, loot and recipe
cd my-mod
cargo horizon-mod run               # build my-mod.hmod, then start the game with it turned on
cargo horizon-mod build             # only build: target/horizon-mod/my_mod.hmod
cargo horizon-mod check target/horizon-mod/my_mod.hmod   # the registry's checks; --json for JSON
```

- `new <path> [--name "Name"]` writes the template. It makes content mods only: any other `--template` is refused.
- `build` packs `horizon-mod.toml`, `assets/` and `data/` into `target/horizon-mod/<id>.hmod`, with the same checks
  the game makes when it reads the content. It refuses a folder with a `Cargo.toml` (code is native now) and a
  manifest without capability `registries`.
- `run` builds, then starts `horizon-client` (on `PATH`, or `--game PATH`, or `HORIZON_CLIENT`) with
  `--dev-mod target/horizon-mod/<id>.hmod`, always on. Arguments after `--` go to the game. The game reads content once
  at start, so rebuild and run again after an edit.
- `check <file>` runs what the registry runs: the file's size and SHA-256, its manifest and content, and a signature
  with `--key <public key> --signature <file>.minisig`. It reads `.hmod`, `.hnmod`, `.hpack`, resource pack `.zip`
  and `.hshader` files.
- `registry build|review|check|list|release|page` is the registry repository's CI ([api/mod-index.md](api/mod-index.md)).

### The manifest

```toml
id = "my_mod"                # [a-z0-9_]{1,64}: settings key, log name
name = "My mod"
version = "0.1.0"
description = "..."
authors = ["..."]
api = "0.2"                  # content mods
side = "both"
capabilities = ["registries"]
namespaces = []              # optional: more namespaces the mod's files use (below)
```

Unknown keys are refused, so a typo is an error, not a silently ignored permission.

### Content mods (API 0.2)

A content mod has capability `registries` and carries `assets/` and `data/` in the `.hmod`. It adds blocks and items
by declaring them in `data/<id>/horizon/block/*.json` and `data/<id>/horizon/item/*.json`, each copying a vanilla
template ([modpacks.md](modpacks.md#registries-02-data-driven-blocks-items-and-mob-effects)), and mob effects (below):

```json
{ "template": "minecraft:iron_block",
  "properties": { "hardness": 4.0, "light": 3, "sounds": "minecraft:amethyst_block" },
  "item": true }
```

A block copies its template's states. It can also declare state properties of
its own, which its template does not have — how a pipe says which sides it
connects to, or a machine whether it is running. Its states then become its
template's states times the combinations of the ones it declares, in a fixed
order with every declared default first, so ids do not move between loads:

```json
{ "template": "minecraft:stone",
  "state_properties": {
    "facing": { "type": "enum", "values": ["north", "south", "east", "west"] },
    "lit": { "type": "boolean" } },
  "default_state": { "lit": "true" } }
```

A `boolean` is `false` and `true`, as vanilla spells them; an `enum` is a list
of values, and a name or value is lower case letters, digits and underscores.
A declared name may not be one the template already has, and `default_state`
takes one value per declared property too, so a block's default is exactly one
state of its own. Every combination has to fit in the states a world holds
(2^20), so a declaration that would not is refused when the mods load.

An item copies its template's data components, with `max_stack_size` and
`rarity` changed, and any other 26.3 component set or removed in
`components`, written as a data component patch is: `minecraft:<type>` sets
one, `!minecraft:<type>` with `{}` removes it. That is how a modded sword
gets its own damage and durability, or a tool its mining speed:

```json
{ "template": "minecraft:iron_sword",
  "components": {
    "minecraft:max_damage": 900,
    "minecraft:attribute_modifiers": [{ "type": "minecraft:attack_damage",
      "id": "minecraft:base_attack_damage", "amount": 7.0,
      "operation": "add_value", "slot": "mainhand" }],
    "!minecraft:repairable": {} } }
```

A block's item takes the same `components`, over the components of its
template block's item; a block with `"item": false` can't have them. Block files
have no `max_stack_size` or `rarity` keys, so those two go in `components` here
(an integer 1 to 99, and one of `common`, `uncommon`, `rare`, `epic`):

```json
{ "template": "minecraft:iron_block",
  "components": {
    "minecraft:max_stack_size": 16,
    "minecraft:damage_resistant": { "types": "#minecraft:is_fire" } } }
```

A mod's creative inventory tabs are `data/<id>/horizon/creative_tab/*.json`:
a title, the item on the tab's button, and its items in order (items the game
doesn't have are left out):

```json
{ "title": "Ruby", "icon": "ruby:ruby", "items": ["ruby:ruby", "ruby:ruby_block"] }
```

They come after vanilla's tabs, ten to a page as NeoForge pages them; the
buttons above the inventory turn the pages. A mod that declares no tab gets one
named after it with all its items, blocks' items and spawn eggs, so every
installed mod's items are a page turn away. The title is a lang key or English
text: `"title": "itemGroup.ruby"` uses that key from the mod's lang files, and a
lang file's `itemGroup.<mod id>.<file name>` (`creative_tab/gems.json` →
`itemGroup.ruby.gems`) translates any title. A mod's items join the search tab
under their names.

A component type no vanilla item has is refused, so a misspelt one fails
the load instead of doing nothing. Behaviour the game ties to a vanilla
item's id doesn't come along yet: a copy of the trident doesn't throw
([#3305](https://github.com/horizon-rust/horizon/issues/3305)).

#### Tools, tags and drops

A block declaration can say what Weft's block builder says with `mineable`, `needs_tool`, `tag` and `drops`:

```json
{ "template": "minecraft:stone",
  "mineable": "pickaxe",
  "tool_tier": "iron",
  "tags": ["c:ores/ruby"],
  "drops": { "item": "ruby:ruby", "count": [1, 2], "fortune": true, "silk_touch": true } }
```

- `mineable`: `pickaxe`, `axe`, `shovel` or `hoe`, or a list of them. The block joins
  `minecraft:mineable/<tool>`, so those tools mine it at their speed.
- `tool_tier`: `stone`, `iron` or `diamond`, the lowest tier that harvests it
  (`minecraft:needs_<tier>_tool`). It turns `requires_tool` on, as vanilla's blocks of a tier all have it,
  unless `properties` sets `requires_tool` itself.
- `tags`: block tags it joins, `<namespace>:<path>` with no `#`, in the mod's namespaces, `minecraft` or `c`.
  An item declaration has `tags` too, for item tags.
- `template_tags: true`: it joins every vanilla tag its template is in (an oak log copy is in
  `minecraft:oak_logs`, so it burns, makes planks and counts as a log; a fence copy connects to fences), and
  its item every item tag of the template's item. An item declaration has it too (a sword copy is in
  `minecraft:swords`, so it takes sword enchantments). Off by default: a port's tags come from its own files.
- `drops`: `"self"` (its item, unless an explosion destroys it), `"nothing"`, or another item:
  `item`, `count` (a number or `[min, max]`, 1 to 64), `fortune` (Fortune adds to it as for ores) and
  `silk_touch` (Silk Touch drops the block itself). The game writes it as the block's loot table,
  `data/<id>/loot_table/blocks/<name>.json`, in the form vanilla's own blocks have, so a mod's table and
  `drops` for the same block are refused. Without either, the block drops nothing, as a vanilla data pack's
  block with no table does (Weft's blocks drop themselves by default: write `"drops": "self"`).
- `properties.replaceable`: placing a block into it replaces it, as for grass and snow layers.

Tags and tiers are entries in the vanilla tags the game already reads, merged after the mod's own tag files;
vanilla's entries and tables are never changed.

#### Fuel, food, durability, attack and tools

An item declaration can set what Weft's item builder and its `fuel` and `food` components set, without writing
the vanilla components by hand:

```json
{ "template": "minecraft:stick",
  "durability": 900, "attack_damage": 5, "attack_speed": 1.2,
  "tool": { "kind": "pickaxe", "tier": "iron", "speed": 7.5 } }
```

- `fuel`: ticks it burns in a furnace (coal is 1600), half that in a smoker or a blast furnace, as vanilla's fuels.
- `food`: `nutrition` (hunger points), `saturation` (as `minecraft:food` stores it: an apple's is 2.4) and
  `always_eat`. The item becomes eatable if its template wasn't.
- `durability`: uses before it breaks; the item then stacks to 1.
- `attack_damage` and `attack_speed`: the numbers its tooltip shows (a bare hand deals 1 and attacks 4 times a
  second); they replace the template's main-hand modifiers of the same kind and keep the others.
- `tool`: `kind` (`pickaxe`, `axe`, `shovel`, `hoe`), `tier` (`wooden`, `stone`, `copper`, `iron`, `gold`,
  `diamond`, `netherite`: what it can harvest, as vanilla's tools of that tier) and `speed` (an iron pickaxe's is 6).

Each writes the component vanilla's own items have (`cooking_fuel`, `food` and `consumable`, `max_damage`,
`attribute_modifiers`, `tool`) over the template's, so the game reads it as it reads vanilla's. Setting one of
these and its component in `components` too is refused.

A mod effect is declared in `data/<id>/horizon/mob_effect/<name>.json` with what vanilla's `MobEffect` takes: its
category (`beneficial`, `harmful` or `neutral`) and its RGB colour (0 to `0xFFFFFF`), which its particles take:

```json
{ "category": "harmful", "color": 9154528 }
```

It is the effect `<id>:<name>`: `/effect give @s ruby:glow 30` gives it, saves and mobs keep it, and it reaches LAN and
dedicated server players who run the same mods (its network id follows vanilla's). Its name is the translation
`effect.<id>.<name>` from the mod's `assets/<id>/lang/`, its icon `assets/<id>/textures/mob_effect/<name>.png` (18x18,
as vanilla's; without one it draws as the missing texture). It does nothing on its own yet: no tick behaviour and no
attribute modifiers. A world opened without the mod drops the effect from its players and mobs. Clients join a server
only with the same mods' effects, as with its blocks.

`crafting_remainder` names what the item leaves in the crafting grid when a
recipe uses it, as a milk bucket leaves a bucket (vanilla's
`Item.Properties.craftRemainder`, not a data component): a vanilla item or
one of the mod's own, refused at load otherwise. It holds on the crafting
table, the inventory grid and the crafter, not yet as furnace fuel or in a
brewing stand. What the item leaves once eaten
or drunk is the `minecraft:use_remainder` component instead:

```json
{ "template": "minecraft:mushroom_stew", "crafting_remainder": "minecraft:bowl" }
```

The fence: files go in the mod's own namespaces (`assets/<id>/`,
`data/<id>/`, and those its manifest lists), plus additions to vanilla tags
(`data/minecraft/tags/`) and to the convention tags NeoForge and Fabric mods
share (`data/c/tags/`).
Templates and `sounds` name vanilla blocks; unknown keys are refused.

Some Java mods ship their files under other names than their mod id (Towns and Towers, mod id `t_and_t`, under
`kaisyn` and `towns_and_towers`). A content mod lists those in its manifest:

```toml
id = "t_and_t"
api = "0.2"
capabilities = ["registries"]
namespaces = ["kaisyn", "towns_and_towers"]
```

A listed namespace is the mod's as its id is: its `assets/` and `data/` files load, and the game reads its tags,
recipes, names and loot. Blocks, items and mob effects are declared in `data/<id>/horizon/` only. `minecraft`, `c` and
`horizon` cannot be listed. Two mods that are on may not own the same namespace (one's id or listed namespace being
the other's): the game keeps the first in load order and leaves the other's content out, with "mods A and B both use
the namespace N; turn one of them off" in the log. Paths use vanilla's resource characters, at most 65,536 files, 16
MiB each and 512 MiB in all. A mod whose content is refused is left out, with the reason in the log. The game reads
this content into its block and item tables (`horizon_client::mod_content`, [modpacks.md](modpacks.md)). A tag entry
naming an id nobody registers (a block a port left out, another mod's item) is dropped, so the rest of the tag still
loads; vanilla would drop the whole tag.

Tags merge as a data pack's do. Every mod's file for one tag (vanilla's `data/minecraft/tags/`, the convention
`data/c/tags/`, or another mod's) adds its values after vanilla's and the mods' before it, in load order. A file with
`"replace": true` first drops what came before it, vanilla's entries and the earlier mods', and the mods after it
still add theirs, as a higher data pack does. Since the result depends on load order, the game says so in the log:
"mod towns replaces data/minecraft/tags/worldgen/biome/has_structure/village_plains.json: vanilla's entries and those
of adds are left out, ...". Tags are the only `minecraft` files a mod may hold: its other content stays in its own
namespaces.

A mod adds to structure sets, template pools and processor lists that are not its own (vanilla's villages, another
mod's houses) with a set, pool or processor list file in its own namespace holding `horizon:add_to`, a key vanilla data
never has. Its entries are appended to each set, pool or list named, after every mod's own files, by
`horizon:priority` (lower first, 1000 when it has none), then in mod and file order; the file is no set, pool or list
itself. A name nobody defines is skipped.

```json
{ "horizon:add_to": "minecraft:villages",
  "structures": [ { "structure": "mymod:village_beach", "weight": 10 } ] }
```

A pool addition holds `"elements"` as a pool does, a processor list addition `"processors"`. This is what
Lithostitched's `add_structure_set_entries`, `add_template_pool_elements` and `add_processor_list_processors` modifiers
do on NeoForge and Fabric, and `horizon-port-pack carry` writes the last two from those.

A pool element may be one of Lithostitched's: `lithostitched:limited` (`delegate`, `limit`, `min_depth`) places its
`delegate` at most `limit` times in one structure start, and `lithostitched:guaranteed` (`delegate`, `count`,
`min_depth`) is tried before its pool's other elements at each jigsaw that reaches that pool (as target or fallback)
until `count` are placed. Neither goes below a piece shallower than `min_depth` (the start is at depth 0).

A mod can also modify another loot table without replacing it: its own
`data/<id>/loot_table/<path>.json` holds `"horizon:add_to": "<namespace>:<path>"` and a `pools`
array. The pools are appended to the named table, and the file is not a table itself.

A structure (and any of its pool elements, in place of the structure's) may hold
`horizon:enhanced_terrain_adaptation`: MoogsStructureLib's terrain kernel, which carves or buries terrain around the
pieces on top of vanilla's `terrain_adaptation`. `kernel_size` and `kernel_distance` shape the kernel; `top` and
`bottom` (`carve`, `bury` or `none`) act above and below a piece's floor; `bottom_offset`, `padding` (`x`, `top`,
`bottom`, `z`) and MoogsStructureLib's `band` (`bottom`, `top`, `piece_heights`) move the box it measures from. On a
structure, `rigid_junctions` adds rigid pieces' junctions as well and `inflate` grows the start's box by the kernel
radius. `kernel_size` 0 is none.

```json
{ "horizon:enhanced_terrain_adaptation": { "kernel_size": 24, "kernel_distance": 24,
    "top": "carve", "bottom": "none", "rigid_junctions": true, "inflate": true } }
```

### Custom dimensions and biomes

A content mod can add `data/<id>/dimension/<name>.json` (the level stem) and
`data/<id>/dimension_type/<name>.json` (the build range and dimension type). The game
registers these dimensions, and `/execute in <id>:<name>` can enter them. Biomes declared in
`data/<id>/worldgen/biome/` load too. Custom noise settings, density functions and material
rules are read from the mod's `worldgen` data, but feature algorithms that are Java classes
still need native code.

### Custom entities

A content mod adds mobs by declaring them in `data/<id>/horizon/entity/<name>.json`. Each copies a vanilla mob, its
*template*: its AI, its behaviour and the animation its model plays come along, and the declaration changes what it
names. Unset keys keep the template's; unknown keys are refused.

```json
{ "template": "minecraft:zombie",
  "health": 30, "speed": 0.3, "attack_damage": 4, "armor": 2, "follow_range": 32, "knockback_resistance": 0.2,
  "width": 0.8, "height": 2.4, "eye_height": 2.2, "category": "monster", "fire_immune": true,
  "scale": 1.25,
  "texture": "ghost", "variants": ["ghost_blue", "ghost_green"], "emissive": "ghost_eyes",
  "model": "ghost", "animations": "ghost",
  "sounds": { "ambient": "minecraft:entity.ghast.ambient", "step": "minecraft:block.wool.step" },
  "loot_table": "mymod:entities/ghost",
  "spawns": [ { "biomes": ["minecraft:plains", "#minecraft:is_forest"], "weight": 10, "min_count": 1, "max_count": 3 } ] }
```

Files are found by the entity's name, as in Weft, so most declarations name none of them:

| File | What it does |
|---|---|
| `assets/<id>/textures/entity/<name>.png` | The skin (`texture` names another under `textures/entity/`; `minecraft:zombie/husk` is vanilla's) |
| `assets/<id>/textures/entity/<name>_e.png` | Weft's glow mask: red × alpha is how much a pixel of the skin glows. It is drawn full bright over the body, by day and in caves alike. Each variant may have its own |
| `emissive` | A texture drawn full bright over the body as it is (an Entity Texture Features `_e` layer: the glowing pixels in colour, the rest clear) |
| `variants` | More skins: each entity wears one of `texture` and these, picked by its entity id, so every player sees the same one |
| `assets/<id>/models/entity/<name>.geo.json` | A Blockbench **Bedrock Entity** model, replacing the template's |
| `assets/<id>/animations/entity/<name>.animation.json` | Its keyframe animations |
| `assets/<id>/sounds/entity/<name>/<event>*.ogg` | `hurt*.ogg`, `death*.ogg`, `ambient*.ogg`, `step*.ogg`: one is played at random. They become the sound event `<id>:entity.<name>.<event>`; the mod's own `sounds.json` entry for that event wins |
| `sounds` | Sound events by event (`ambient`, `hurt`, `death`, `step`) instead of files. Unset ones are the template's |
| `data/<id>/loot_table/entities/<name>.json` | What it drops (vanilla's `EntityType.getDefaultLootTable`); without one, its template's table, or `loot_table` |
| `spawns` | Natural spawns: the entity joins the spawn lists of these biomes (ids or `#tags`) in its category, with the weight and group size. Where and when it may spawn (light, ground, height) follow its template's spawn rules |

The model file is read as Blockbench exports it: bones with `pivot`, `parent` and `rotation`, boxes with `origin`,
`size`, `uv` (box UV or per face), `mirror`, `inflate`, and a `pivot` and `rotation` of their own (such a box turns in
a part of its own), and `poly_mesh` bones (triangles and quads, `normalized_uvs` or not). Both the format of 1.12 and
later (`minecraft:geometry`) and the legacy `geometry.<name>` one read. A bone's name gives it its role, case and `_`
ignored, so the template's own animation moves it:

| Bone | Part it drives |
|---|---|
| `head`, `body` | `head` (looks where the mob looks), `body` |
| `rightArm`, `leftArm` | the arms (a zombie holds them out, a skeleton aims) |
| `rightLeg`, `leftLeg`, or `leg0`, `leg1` in a model with two legs | the legs, swinging as it walks |
| `leg0` .. `leg3` in a model with four | a quadruped's hind (`leg0`, `leg1`) and front legs |
| `leg0` .. `leg7` in a model with more | a spider's legs |

An animation's name is what follows the last dot of its key (`animation.ghost.idle` is `idle`). Keyframes carry
`rotation` (degrees), `position` (pixels) and `scale`, as `[x, y, z]`, one number, or `pre`/`post` with
`"lerp_mode": "catmullrom"`; they are added over the template's pose. `idle` plays all the time, `walk` as much as the
mob walks, `attack` once from each swing (a melee hit), `hurt` after a hit and `death` while it dies. `loop` is
`true`, `false` (once, then back) or `"hold_on_last_frame"`. Molang isn't evaluated: an expression reads as 0, and the
log says so.

Any other animation is started by a native mod (Weft's `play_animation`): `native_play_animation(id, "bow", false)`
plays it once, `true` over and over, `""` goes back; while it plays it takes the place of `idle` and `walk`. The server
tells every player who sees the entity, and one who comes near later, at the point it has reached (the
`horizon:play_animation` custom payload).

A broken declaration, model or animation file refuses the mod, with the file and the reason. `/summon <id>:<name>`
summons it, worlds save it under its id, and its name is the translation `entity.<id>.<name>`. Content mods'
entities get network ids after vanilla's, so a server and its players need the same mods. An entity burns in
daylight only if its own id is in the `minecraft:burn_in_daylight` entity type tag, as in vanilla: a zombie or
skeleton template does not make it burn. Add it to `data/minecraft/tags/entity_type/burn_in_daylight.json` to
burn. A declared entity's
spawn egg is generated as item `<id>_spawn_egg`, copying a vanilla egg and carrying its `minecraft:entity_data`:
`"spawn_egg": {"colors": [10, 20]}`. The colours make a simple egg item model and texture automatically;
`"item_model": "mymod:item/ghost_spawn_egg"` names your own model and texture instead. Add the item name
to the lang (`item.<mod>.<name>_spawn_egg`). If no key is present, the content root adds a default
name for the item, block, entity and spawn egg; your own lang value wins. A model of the mod's own wears
the template's armour models and holds items in the bone named `rightArm`/`leftArm`; armour is drawn
from the item's `equippable.asset_id`, so a mod's own armour (`assets/<id>/equipment/<name>.json` and
`textures/entity/equipment/humanoid*/<name>.png`) shows on any mob. Not yet: the template's other
extra layers (a sheep's wool) on a model of the mod's own.

A declaration can also make an entity a platform: `"solid": true` blocks players and lets them stand on it;
`"kinematic": true` makes it a moving platform that carries a player standing on top; `carry_height` is the
height above its top a player may ride. Weft's block decks (`blocks(&DECK)`) are still later.

`"swims": true` adds vanilla's FloatGoal on top of the template's goals, so a template that sinks (a skeleton) swims
up in water and lava and keeps the rest of its AI. A `goals` list does the same with `{ "goal": "float" }`, but
replaces the template's goals.

`"melee_swing": true` makes an entity whose template raises both arms to fight (a skeleton holding a sword, a
zombie) hold its weapon in one hand and swing that arm, as a vindicator or a piglin does.
`"perfect_aim": true` makes a skeleton-like shooter's arrows fly with no spread, arced to make up for their fall.
`"block_chance": 0.8` makes a raised shield (`native_mob_shield`) stop only that share of the hits from in front
(1, always, by default).

#### Entity behaviour

`goals` and `targets` give a declared entity an AI of its own, from a fixed vocabulary (Weft's `goals(..)`); each
entry is the vanilla goal of the same job, run by vanilla's goal selector. A list replaces the template's list of
that kind; leaving it out keeps the template's. `priority` orders them as vanilla's selector does (lower first; a
lower one interrupts a higher one that uses the same controls) and defaults to the entry's place in the list.

```json
{ "template": "minecraft:cow", "attack_damage": 4, "attack_knockback": 1.5,
  "goals": [
    { "goal": "float" },
    { "goal": "melee", "speed": 1.2, "reach": 1.5 },
    { "goal": "tempt", "items": ["#minecraft:cow_food"], "speed": 1.1 },
    { "goal": "avoid", "entities": ["minecraft:wolf"], "distance": 8 },
    { "goal": "wander", "speed": 0.8 },
    { "goal": "look_at_player", "distance": 6 },
    { "goal": "random_look_around" } ],
  "targets": [
    { "target": "hurt_by", "alert_others": true },
    { "target": "nearest_attackable", "entities": ["minecraft:player", "mymod:ghost"] } ] }
```

| Goal | Vanilla goal | Keys (default) |
|---|---|---|
| `float` | `FloatGoal`: swims up in water and lava | |
| `melee` | `MeleeAttackGoal`: walks to its target and hits it with its `attack_damage`; `attack_knockback` pushes | `speed` (1), `follow_unseen` (false), `reach` past its box (vanilla's, about 0.83), `interval` ticks (20) |
| `leap_at_target` | `LeapAtTargetGoal` | `velocity_y` (0.4) |
| `ranged` | `RangedAttackGoal`: keeps within `range` and shoots | `projectile`: `arrow` (a skeleton's), `snowball`, `llama_spit`; `speed` (1), `interval` (40), `range` (15) |
| `wander` | `WaterAvoidingRandomStrollGoal`, `RandomStrollGoal` with `avoid_water` false | `speed` (1), `interval` (120), `avoid_water` (true) |
| `look_at_player` | `LookAtPlayerGoal` | `distance` (8), `probability` (0.02) |
| `random_look_around` | `RandomLookAroundGoal` | |
| `panic` | `PanicGoal`: runs after a hit or while burning | `speed` (1.25) |
| `tempt` | `TemptGoal`: follows a player holding one of `items` (ids or `#tags`) | `speed` (1), `scared` (false) |
| `avoid` | `AvoidEntityGoal`: runs from `entities` | `distance` (6), `walk_speed` (1), `sprint_speed` (1.2) |
| `restrict_sun`, `flee_sun` | `RestrictSunGoal`, `FleeSunGoal` | `flee_sun`: `speed` (1) |

| Target | Vanilla goal | Keys (default) |
|---|---|---|
| `hurt_by` | `HurtByTargetGoal`: whoever hurt it | `alert_others` (false): its own kind around joins in |
| `nearest_attackable` | `NearestAttackableTargetGoal`: the nearest of `entities` it can see within its `follow_range` | `must_see` (true), `interval` (10) |

Entities are ids: `minecraft:player`, a vanilla mob (`minecraft:zombie` is a zombie, never a mod's entity copying
one) or a mod's entity (`mymod:ghost`). The template's own behaviour beyond its goals stays: a cow still gives
milk, a creeper with declared goals no longer swells (swelling is one of its goals). Templates that think with a
brain instead of goals (villager, warden, sniffer, axolotl, breeze, creaking, copper golem, happy ghast) take no
`goals` or `targets`.

Behaviour that needs code (every tick, on a hit, at death, on a right click) is a native mod's: the hook points
`native_entity_tick`, `native_entity_hurt`, `native_entity_died` and `native_entity_interact` name the declared
entity's type ([native-mods.md#entities](native-mods.md#entities)).

#### Overlay layers

`overlay` draws a texture over the body, lit as the skin is (an `emissive` texture is full bright): one texture, or
frames shown in turn.

```json
"overlay": "creaking/spruce_layer"
"overlay": {"frames": ["creaking/crimson_layer1", "creaking/crimson_layer2"], "frame_ticks": 10}
```

#### Creaking variants

An entity whose template is `minecraft:creaking` may carry a `creaking` block: a creaking heart between two logs it
names grows this creaking instead of vanilla's, in the manner of
[Grafted Creaking](https://github.com/DialingSpoon527/Grafted-Creaking) (MIT). The log below the heart (or west or
north, along its axis) picks the variant. With no variant declared, hearts keep vanilla's pale oak rules.

```json
{
  "template": "minecraft:creaking",
  "texture": "creaking/birch",
  "overlay": "creaking/birch_layer",
  "creaking": {
    "heart_logs": ["#minecraft:birch_logs"],
    "active": "night",
    "freeze": "gaze",
    "hunts": "players",
    "water_walk": false,
    "multiply_when_hurt": false,
    "blinds_players": 10
  }
}
```

- `heart_logs`: blocks or `#tags`. A log with no axis (`minecraft:mushroom_stem`) fits any heart.
- `active`: when its heart is awake and it is out. `night` (vanilla's), `day` (it goes away at night), `always`.
- `freeze`: `gaze` (vanilla's: it stands still while a player looks its way), `strict_gaze` (once awake only a
  look straight at its eyes holds it), `inverted` (it moves only while looked at).
- `hunts`: `players` (vanilla's), `nothing` (never wakes), `players_and_monsters` (monsters but creepers too),
  `everything` (every living thing but creakings and creepers).
- `water_walk`: water holds it up, as lava holds a strider. `multiply_when_hurt`: a player's hit while its heart
  protects it calls another of its kind near the heart. `blinds_players`: Blindness for players in survival within
  that many blocks.
- Stats such as `attack_damage` and `speed` work as for any declared entity.

A native mod plants hearts in trees with `native_plant_creaking_heart` ([native-mods.md](native-mods.md#creaking-hearts-in-trees)).

## Native mods

Behaviour that needs code (a new key, a HUD, a tool that changes how mining works) is a native mod. It is a bundle of
a `horizon-mixin.toml` and a few `.rs` helpers that hook into named functions of the game. Start from
[horizon-native-mod-template](https://github.com/horizon-rust/horizon-native-mod-template). How the launcher builds
one, what it pins and why it is not sandboxed: [native-mods.md](native-mods.md).

A native mod's config files go in the game's shared `config/` folder, beside `options.txt` (on a dedicated server,
beside `server.properties`), as Fabric and NeoForge mods' do: the mod picks its own file names, such as
`config/<mod id>.json`. It asks for the folder with `Game::native_config_dir()` (hook points API 26), which needs no
game, so it works from any hook, and makes the folder when missing. Settings kept with `native_set_setting` stay where they were, in `horizon-mods/.horizon-mod-data/<id>/`.

## Publish your mod

Horizon's registry lists mods, modpacks (`.hpack`) and resource packs made for Horizon. **A project is reviewed
once, when it is listed; its later releases are checked by machine, not read by a person.** Players find them at
[playhorizon.dev/mods](https://playhorizon.dev/mods) and in the launcher (Mods, then **Horizon** beside Modrinth),
which checks every file before it installs it.

1. Run `cargo horizon-mod check` on the file (above): the registry runs the same checks.
2. Put the source in a public GitHub repository, under a licence that lets players run it. A native mod's `.hnmod` is
   its source.
3. Publish a **GitHub release** in it with the file as an asset (`.hmod`, `.hnmod`, `.hpack`, `.hshader`, or a resource
   pack's `.zip`), and its `.minisig` beside it if you sign. The release notes are the changelog.
4. List the project once: open [playhorizon.dev/mods/submit](https://playhorizon.dev/mods/submit), sign in with GitHub
   and give the repository and the page (name, summary, licence, optionally a minisign public key). It opens a pull
   request in [horizon-rust/horizon-mods](https://github.com/horizon-rust/horizon-mods) adding `mods/<id>.toml`, with
   you as owner. A reviewer reads the repository and its source and merges it.

**Releasing a version is only step 3.** Publish a GitHub release (not a draft or a pre-release); within the hour the
registry reads it, runs the checks a submission runs (the file's own, the signature, the id and the version in the
file's manifest), and lists it. A release that fails is skipped and reported in an issue of the registry, never
published. Nobody opens a pull request and nobody reviews it. The version is the one in the file's manifest (the tag
when the file has none), the date and the changelog are the release's, the SHA-256 is computed by the registry, and
the launcher downloads the asset from your repository and refuses any other bytes. A published file never changes:
publish a new version. `revoked.toml` takes a bad one down.

Once it is published, its page and its card in the launcher show its rating: players signed in with GitHub on its
page give it 1 to 5 stars, one rating per account, which they can change. Its owners can't rate it, and nobody sees
who rated.

Your page can live in your repository too: a `horizon-registry.toml` at its root (on the newest release's tag) with
`name`, `summary`, `description`, `authors` and `license` fills what the registry's entry leaves out; the entry's own
words win. Its owners, key, kind and icon are the registry's, changed by a pull request. Projects listed before this
(`versions` in their entry) keep their hosted versions. The formats: [api/mod-index.md](api/mod-index.md).

### Integrity and signing

- **Integrity.** Every download is checked against the SHA-256 the index lists before the file reaches the game
  folder.
- **Signing.** Authors sign files with an Ed25519 key (minisign format: a `.minisig` beside the file); the index pins
  each project's public key, so an update signed by another key is refused. Unsigned local files stay allowed, with an
  "unsigned" mark.
- **Takedowns.** A file the registry takes down (`revoked.toml`) is refused by the launcher and turned off where
  installed.

## Horizon's official mods

Horizon's own mods are native. They live in
[horizon-rust/horizon-mods](https://github.com/horizon-rust/horizon-mods) under `native/<name>/`:

| Mod | Folder | Replaces |
|---|---|---|
| FPS Counter | `native/fps-overlay` | Extras > FPS counter |
| Keystrokes | `native/keystrokes` | Extras > Keystrokes |
| Zoom | `native/zoom` | Extras > Zoom |
| Minimap | `native/minimap` | Extras > Minimap |
| Recipes | `native/recipes` | The Recipes mod (item panel and recipe pages, on hook points version 2) |
| Coordinates HUD | `native/coords-hud` | A small example mod (position and facing, `/where`, `/coords`) |

The game keeps the parts that must be fast, such as drawing the minimap's map, and a native mod picks the options
([extras.md](extras.md)). Far View is not a mod any more: its distance and detail are Graphics > LOD Distance and Far
Detail ([graphics.md](graphics.md)).
