# Weftizon roadmap

This is the only roadmap of this fork: what is done, what is next, and the detail of each part. Upstream
Horizon's roadmap and its generator (`roadmap/`, `tools/roadmap.py`) are not used here. The practical modder
guide is [docs/making-mods.md](docs/making-mods.md); what a mod can do today is in [docs/mods.md](docs/mods.md)
(content mods) and [docs/native-mods.md](docs/native-mods.md) (code mods).

## What Weftizon is

Weftizon is Horizon's Minecraft: Java Edition 26.3 Rust engine, extended with the modding power of
[Weft](https://github.com/VortexSo4/Weft): content registries, native code hooks, entities and mobs, blocks and
items, worldgen data, events, questions and chains, machines, client APIs and assets. The goal is the same power
as Weft: mods register content, attach behaviour, intercept events and questions, build machines, add UI and use
the world API, while Horizon's vanilla parity stays intact.

Weft's modding surface is a native Rust SDK: handles, builders, components and tags, events, questions, value
chains, attributes, commands, a world API, UI/HUD/keybind/packet APIs and data-driven assets. Horizon has content
mods (`.hmod`, data and assets, no code) and native mods (`.hnmod`, mixin bundles that patch named Rust
functions). The port therefore has two layers:

1. **Data registries** for what a Weft mod declares without code: `data/<id>/horizon/<kind>/*.json` and files
   found by name under `assets/<id>/`, as Weft finds them.
2. **Runtime/API hooks** for code, behaviour, UI, events, machines, AI and worldgen: native mod hook points
   (`native-hooks.json`, [native-mods.md](docs/native-mods.md)), each with a data-driven form where one is enough.

Weft is MPL-2.0 and Horizon is MIT. Port from Weft's public docs and behaviour; do not copy Weft source.

## Rules every part keeps

- **No mods, no change.** With no content mod on, the resource root is the base root and every loader reads
  vanilla's files. A mod's content is *added* to the tables (rows and ids after vanilla's); vanilla's rows are
  never rewritten, so vanilla parity holds with mods on too.
- **Templates.** A mod's block, item or entity copies a vanilla one and keeps its behaviour; only what it declares
  differs.
- **Checked where it is read.** `horizon-mods` reads and checks every declaration and file (`Content::read`), so
  `cargo horizon-mod check`, the registry and the game refuse the same mod with the same reason.
- **Tested end to end.** Unit tests for the format in `horizon-mods`; the content root built over the prepared
  resources and read by the game's real loaders in `mod_content_tests.rs`; docs in [mods.md](docs/mods.md) or
  [native-mods.md](docs/native-mods.md); this file's status; then commit and push.
- **Hook points are only added.** Each addition raises `NATIVE_HOOKS_API`; a change that would break a built
  bundle is not made.

## Status

| Area | Weft provides | Weftizon today | Part |
|---|---|---|---|
| Packaging / loader | `declare_mod!`, native cdylib, C ABI, zip mods loaded at runtime | `.hmod` content mods and `.hnmod` mixin mods built by the launcher; no runtime SDK | 9 |
| Names / handles | `namespace:path` names, handles with ids, generated names | content namespaces, network ids after vanilla's, auto names for items, blocks, entities and eggs | done |
| Entities | size, health, speed, category, texture, model, animations, sounds, spawns | declarations from vanilla templates, Bedrock models and animations, skins, variants, glow, sounds, spawns, loot, spawn eggs | 1 ✅ |
| Entity behaviour | AI goals, attack damage, tick/hurt/death/interact, components, platforms | goals, targets, animations, hooks (API 16), components, solid/kinematic platforms; block decks later | 2 |
| World API: mobs | spawn, navigate, remove, set state | spawn, navigate, path reach, teleport, velocity, hurt, heal, remove (API 16, 20) | 2 ✅ |
| Blocks | hardness, tool tier, drops, light, render layers, model boxes, collision, states, events | templates with properties and shapes; tools, tiers, tags, drops as data; block hooks (API 17); break, hardness, cracks (API 20) | 3 |
| Items | texture, stack, durability, tools, damage, placed block, wearable, use | templates with component patches; fuel, food, durability, attack, tools as data; use hooks (API 13) | 3 |
| Recipes / loot | shaped/shapeless, recipe kinds, loot pools, loot modification | 26.3 data-pack recipes and loot; `horizon:add_to` loot modification; no custom recipe kinds | 3 |
| Components / tags | typed components, per-entity components, tags | vanilla components and tags; declarations join tags; per-entity components | 3 |
| Events / questions / chains | batched events, intercepts, value chains, timed attributes, `w.emit` | built-in events, questions and damage chains (API 18); a level's own tick (API 20); no `w.emit`, no timed attributes | 3 |
| Commands | typed command builders | native command hooks (API 8) | 3 |
| Machines | block + block entity + inventory + menu + screen; systems; `core:energy` | machine blocks with slot groups, menus, hopper rules, spill, save, catch-up ticks (API 19); no energy, no systems | 4 |
| Liquids | flow, renewal, temperature, bucket, reactions | vanilla fluids only | 5 |
| Worldgen | biomes, dimensions, terrain presets, ores, scatter, structures, spawns | custom dimensions and biomes, noise settings, density functions, material rules, structures, features, presets as data | 5 |
| Client UI | screens, HUD, keybinds, packets, frame hooks, cosmetics, settings | native HUD/key/packet/settings hooks; boss bars (API 20); no screen or HUD builders | 6 |
| Tooltips / rich text | tooltips, colours, effects, images, previews | native tooltip hook (API 12) | 6 |
| Textures | animated (`.anim`), glow (`_e`), overlays, CTM | resource packs, `.mcmeta`; glow for entity skins | 7 |
| Models / animations | Blockbench block/item/entity models, keyframes, first-person hand, armour | vanilla models; Bedrock entity models | 7 |
| Shaders / sounds / particles | shader snippets and packs, sound categories, particles | `.hshader` packs, sounds.json, vanilla particles | 7 |
| Debugging | per-mod log filters, crash reports, mod time per tick | native mod logs and crash reports, F3 mod timing | 8 |

## Next, in order

1. **Invasion Mod support** (part 2 and 3's World API, API 20): done. Tested in a real game (2026-10-31) with a
   wave written into `native_level_tick`: 6 zombies spawned, walked to a player in a glass box, found their paths
   blocked, dug through the glass, and a boss bar counted them. Not yet built through the launcher as a
   horizon-mixins bundle. Left: the invasion mod itself, as a native mod with its own mob declarations.
2. **Weft's own events** (`w.emit`) and timed attributes (part 3).
3. **Screen and HUD builders** (part 6).
4. **Energy and systems** for machines (part 4).
5. **Liquids** (part 5).
6. **Worldgen**: terrain presets, ores, scatter and spawns in mods' own biomes (part 5).
7. **Assets and rendering** (part 7).
8. **Items and recipes**: item use, place and wearable behaviour, custom recipe kinds, full loot modifiers (part 3).
9. **Runtime SDK and loader** (part 9).
10. **Debugging and tooling** (part 8).

## 1. Custom entities and mobs: done

Inspired by Weft's entities and by Entity Model Features, Entity Texture Features and Entity Sound Features.
Players' docs: [mods.md#custom-entities](docs/mods.md#custom-entities).

- `data/<id>/horizon/entity/<name>.json` copies a vanilla mob (`horizon_mods::entity`): size, eye height, health,
  speed, attack damage, armour, follow range, knockback resistance, category, fire immunity, render scale, skin,
  variants, emissive layer, model, animations, sounds, loot table, spawns.
- Blockbench Bedrock geometry (`.geo.json`, 1.12+ and legacy) and animations (`.animation.json`) read into Java
  model space (`horizon_mods::bedrock`): boxes unwrapped as `ModelPart.Cube`, per-face UVs, mirror, inflate, rotated
  boxes, poly meshes; bones renamed by role so the template's animation moves them; keyframes (linear, catmull-rom,
  pre/post) in Java's units, played over the template's pose (`idle`, `walk`, `hurt`, `death`; loop, once, hold).
- Skins, random variants picked by entity id, Weft `_e` glow masks turned into full-bright layers, ETF-style emissive
  layers (`mobs/render.rs`, the entity atlas reads mods' `textures/entity/`).
- Sounds by event or found by file name (`<event>*.ogg`), merged into the mod's `sounds.json`; modded sound ids keep
  their namespace through the server and client paths.
- Spawn rules: entries added to the named biomes' (and biome tags') `natural_mob_spawns` in the entity's category;
  spawn placement copied from the template; spawn rules follow the template's species.
- Registry ids after vanilla's (`registries.json`), `/summon` and entity argument lists, save and load under the
  mod's id, `entity.<id>.<name>` names, default loot table `<id>:entities/<name>`.
- Spawn eggs (`"spawn_egg": {"colors": [..]}`): the content root generates `<id>_spawn_egg` with a two-colour model
  and texture; a custom `item_model` names the mod's own.
- Tests: `bedrock_tests.rs`, `content.rs` (declarations), `mobs/custom_tests.rs`,
  `mod_content_tests.rs::custom_entities_join_the_root`.

Left for later parts: armour and held items and the template's extra layers on a mod's own model (7), Molang (not
planned: keyframes only), LAN/dedicated clients without the same mods (they need the same content, as for blocks).

## 2. Entity behaviour, AI and the mobs' World API

- ✅ Data: `goals` in the entity declaration from a fixed vocabulary mapped to Horizon's goal implementations
  (`horizon_mods::behaviour` → `mobs/custom.rs::register`): `float`, `melee` (`reach`), `leap_at_target`, `ranged`
  (arrow, snowball, llama spit), `wander`, `look_at_player`, `random_look_around`, `panic`, `tempt` (items and
  `#tags`), `avoid`, `restrict_sun`, `flee_sun`; `attack_knockback`; target selectors `hurt_by` (`alert_others`
  calls its own kind) and `nearest_attackable`. A declared list replaces the template's selector; brain-driven
  templates refuse them. Players' docs: [mods.md#entity-behaviour](docs/mods.md#entity-behaviour).
- ✅ Animation triggers: `attack` keyframes from each swing, and `Mob::play_animation(name, repeat)` (Weft's
  `play_animation`; native mods call `native_play_animation`), synced by the entity tracker as the
  `horizon:play_animation` custom payload.
- ✅ Callbacks for code (API 16, `game/native_entities.rs`): `entity_tick` (batched per type, as Weft's `on_tick`),
  `entity_hurt`, `entity_died`, `entity_interact` (claimable), and helpers by entity id (read, velocity, teleport,
  hurt, heal, remove, `native_play_animation`).
- ✅ Per-entity components (Weft's `entity_component`) saved in the entity's NBT under `horizon:components`.
- ✅ Kinematic platforms (`solid`, `kinematic`, `carry_height`). Weft's block decks are still later.
- ✅ The mobs' World API (API 20, `game/native_entities.rs`): `native_spawn_entity` (Weft's `spawn`: a persistent
  mob, finalized as a natural spawn, refused for monsters on Peaceful as `/summon` refuses them), `native_navigate`
  (Weft's `navigate`, vanilla's `PathNavigation.moveTo` over Horizon's ported pathfinder), `native_navigating` and
  `native_path_reaches` (`Path.canReach`: false for a target behind a wall, so a digging mob knows to dig).
- Tests: goal selection probes against the template's (same RNG order), callbacks in `lan_e2e`.

## 3. Blocks, items and inter-mod behaviour

- ✅ Data forms of Weft's block builder (`horizon_mods::content`, `mod_content_tags_drops.rs`): `mineable` and
  `tool_tier`, `tags` on blocks and items, `template_tags`, `drops` (`self`, `nothing`, an item with count, fortune
  and silk touch, written as vanilla's generated loot tables) and `properties.replaceable`. Players' docs:
  [mods.md#tools-tags-and-drops](docs/mods.md#tools-tags-and-drops). Unlike Weft, a block without `drops` or a table
  drops nothing, so ported mods keep their own tables' meaning.
- ✅ Block callbacks for code (API 17, `game/native_blocks.rs`): `native_use_block` (Weft's block `on_use`),
  `native_block_tick` (Weft's `on_scheduled`), and helpers `native_block`, `native_set_block`,
  `native_set_block_property` (Weft's `set_prop`) and `native_schedule_block_tick` (Weft's `schedule`).
  `on_neighbor`, `on_placed` and `on_broken` are API 6, 7 and 14's hooks.
- ✅ Breaking blocks (API 20, `game/native_blocks.rs`): `native_break_block` (`Level.destroyBlock`: particles, sound,
  drops without a tool), `native_block_hardness` (`destroy_time`; none for bedrock and barriers) and
  `native_block_crack` (`Level.destroyBlockProgress`, the crack a zombie shows on a door).
- ✅ Weft's item builder and its components as data (`horizon_mods::content::ItemTraits`): `fuel`, `food`,
  `durability`, `attack_damage`, `attack_speed`, `tool`. Players' docs:
  [mods.md#fuel-food-durability-attack-and-tools](docs/mods.md#fuel-food-durability-attack-and-tools).
- ✅ Weft's built-in events, questions and value chains (API 18, `game/native_events.rs`): `native_pickup_item`,
  `native_item_picked_up`, `native_player_joined`, `native_player_left`, `native_attack_entity`,
  `native_enter_dimension`, `native_hurt` (`question::HURT`), `native_damage` (`chain::DAMAGE`) and
  `native_attack_damage` (`chain::ATTACK_DAMAGE`). Verified with a real horizon-mixins bundle (`07dab17`).
- ✅ A level's own tick (API 20): `native_level_tick(dimension)`, once a tick for each level the server ticks, after
  its mobs, with that level in place. Where a mod runs waves, events and timers.
- ✅ Loot modification: `"horizon:add_to"` + `pools` appends to another table.
- Left: Weft's own events (`w.emit`), timed attributes, a player's hits on boats, carts, frames, armour stands and
  the dragon, `by_player`/`attacker` for hits on players, typed command builders, custom recipe kinds, item use,
  place and wearable behaviour.

## 4. Machines, block entities, inventories and menus

- ✅ One declaration (Weft's `r.machine`): a content block `machine` with slot groups, a menu, hopper rules, spill
  and save; `native_machine_tick` with catch-up ticks (API 19, `game/mod_machines.rs`).
- Left: `core:energy` as a block entity component with conductor tags, networks balanced once per tick; systems
  batched over a component; mod networking between client and server halves (custom payloads); broader menu
  screens.

## 5. Liquids and worldgen

- Liquid registry: flow speed and distance, renewal, temperature, bucket item, reactions; a template fluid (water,
  lava) keeps vanilla's code paths. Not started.
- ✅ Biomes and dimensions from content data (`data/<id>/worldgen/biome/`, `data/<id>/dimension/`,
  `data/<id>/dimension_type/`); custom dimensions can be entered with `/execute in`. Noise settings, density
  functions, material rules, structures, features and presets load from content data.
- Left: terrain presets, ores, scatter and spawns in mods' own biomes; custom generators where data alone is not
  enough (native code).

## 6. Client APIs

- ✅ Native hook points: HUD drawing, keys, mouse wheel, client commands, zoom, minimap, settings, item panel.
- ✅ Boss bars for server mods (API 20, `game/bossbars.rs`): `native_boss_bar` and `native_remove_boss_bar`, a saved
  custom bar shown to every player, as `/bossbar` makes it.
- Left: screen and HUD builders (data layouts + native callbacks), key binding builders, packets, frame hooks; entity
  cosmetics (`client_entity_tick`), screen effects, sound categories, music; mod settings in client, server and
  world scopes; rich text tooltips.

## 7. Assets and rendering

- Texture `.anim` (frametime, interpolate, frames), `_e`/`_s` glow for blocks, overlays, CTM.
- Java block and item models from Blockbench for mod blocks and items; first-person hand bone; armour and held items
  on mods' entity models; the template's layers mapped onto them by role.
- Shader snippets and packs; resource-pack layering over mod assets.

## 8. Debugging and verification

- ✅ Native mod logs and crash reports, F3 mod timing.
- Left: per-mod log filters, crash attribution, harnesses for each ported feature beside the vanilla parity checks,
  registry and launcher polish.
- With no mods, the vanilla code paths and resource root stay unchanged (checked by the content root tests:
  `no_content_keeps_the_base_root`).

## 9. Runtime SDK and loader

- Weft's `declare_mod!`, C ABI, handles and builders; cdylib and zip mods loaded at runtime, without a rebuild of
  the game. Today a native mod is a mixin bundle the launcher compiles into the game.

## Invasion Mod

Invasion Mod's waves need: a timer in the level (`native_level_tick`), spawning the wave (`native_spawn_entity`),
walking mobs to the target (`native_navigate`), knowing when a wall is in the way (`native_path_reaches`,
`native_navigating`), digging through it (`native_block_hardness`, `native_block_crack`, `native_break_block`) and
showing the wave (`native_boss_bar`). All of them are in API 20. Custom mobs for the invaders are content mod
entities (part 1 and 2). The mod itself is not written yet.

## TheAfterdark

The TheAfterdark content port loads and its dimension can be entered:

- assets and data load; custom biomes load; the dimension `the_afterdark:afterdark` registers and can be entered
  with `/execute in` and `/tp`; the Teleport Block and Teleport Catalyst items register and render.
- Custom feature types (`spiral`, blobs, pillars, ponds, falls, cave plants) are clean-room approximations of
  TheAfterdark's Java generators, not exact ports; every placed feature in its biome list decodes (0 skipped at
  load). Legacy 1.20.1 block states (DataFixer), providers, placements, `random_patch` and the removed vanilla
  feature `seagrass_simple` are supported; waterloggable random patches spread their tries.

Not done:

- `TeleportBlockEntity` and `TeleportCatalystItem` teleport behaviour (optional: `/execute in` and `/tp` already
  travel there)
- fire, lava, dripstone and grass-burn mixins; catalyst loot modification; config behaviour
- full terrain and biome visual parity in the custom dimension

## Rule for agents

This file is the one roadmap. Update its status and its parts here when a feature lands; do not start a second
roadmap file. Upstream Horizon's `ROADMAP.md` and `roadmap/` were removed from this fork on purpose.
