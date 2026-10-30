# Weft modding port

Goal: give Weftizon the modding power of [Weft](https://github.com/VortexSo4/Weft), while keeping
Horizon's vanilla parity and existing content/native mods intact.

Weft's modding surface is a native Rust SDK: handles, builders, components/tags, events,
questions, value chains, attributes, commands, a world API, UI/HUD/keybind/packet APIs, and
data-driven assets. Horizon has content mods (`.hmod`, data and assets, no code) and native mods
(`.hnmod`, mixin bundles that patch named Rust functions). The port therefore has two layers:

1. **Data registries** for what a Weft mod can declare without code: `data/<id>/horizon/<kind>/*.json`
   and files found by name under `assets/<id>/`, as Weft finds them.
2. **Runtime/API hooks** for code, behaviour, UI, events, machines, AI and worldgen: native mod hook
   points (`native-hooks.json`, [native-mods.md](native-mods.md)), each with a data-driven form where
   one is enough.

Weft is MPL-2.0 and Weftizon's Horizon base is MIT. Implement from Weft's public docs and observed
behaviour; do not copy Weft source.

## Rules every phase keeps

- **No mods, no change.** With no content mod on, the resource root is the base root and every loader
  reads vanilla's files. A mod's content is *added* to the tables (rows after vanilla's, ids after
  vanilla's); vanilla's rows are never rewritten, so vanilla parity holds with mods on too.
- **Templates.** A mod's block, item or entity copies a vanilla one and keeps its behaviour; only what
  it declares differs. Behaviour of its own comes in the behaviour phases.
- **Checked where it is read.** `horizon-mods` reads and checks every declaration and file
  (`Content::read`), so `cargo horizon-mod check`, the registry and the game refuse the same mod with the
  same reason. The game then never meets a malformed file.
- **Tested end to end.** Unit tests for the format in `horizon-mods`; the content root built over the
  prepared resources and read by the game's real loaders in `mod_content_tests.rs`; docs in
  [mods.md](mods.md); then commit and push.

## Feature inventory

| Area | Weft provides | Weftizon today | Phase |
|---|---|---|---|
| Packaging / loader | `declare_mod!`, native cdylib, C ABI, zip mods loaded in parallel | `.hmod` content mods and `.hnmod` mixin mods | Foundation (exists) |
| Names / handles | `namespace:path` names, handles with ids, generated names | content namespaces; network ids after vanilla's | Foundation (exists) |
| **Entities** | size, health, speed, category, texture, model, animations, sounds, spawns | **done (phase 1)**: declarations from vanilla templates, Bedrock models and animations, skins, variants, glow, sounds, spawns, loot | 1 ✅ |
| Entity behaviour | AI goals, attack damage, tick/hurt/death/interact, components, kinematic platforms | template behaviour only | 2 |
| Blocks | hardness, tool tier, drops, light, render layers, model boxes, collision, states, events, particles | template copies with properties and shapes; native hooks for behaviour | 3 |
| Items | texture, stack, durability, tools, damage, placed block, wearable, use | template copies with component patches | 3 |
| Recipes / loot | shaped/shapeless, recipe kinds, loot pools, loot modification | 26.3 data-pack recipes and loot; no loot modifiers | 3 |
| Components / tags | typed components, per-entity components, tags | vanilla components and tags | 3 |
| Events / questions / chains / attributes | batched events, intercepts, value chains, timed attributes | native hooks only | 3 |
| Commands / World API | typed command builders; blocks, entities, inventories, particles, sounds | native command hooks (API 8) | 3 |
| Machines | block + block entity + inventory + menu + screen; systems; `core:energy` | block entities (API 15), menus (API 10) | 4 |
| Liquids | flow, renewal, temperature, bucket, reactions | vanilla fluids | 5 |
| Worldgen | biomes, dimensions, terrain presets, ores, scatter, structures, spawns | data-pack worldgen, structure mods; entity spawns into vanilla biomes (phase 1) | 5 |
| Client UI | screens, HUD, keybinds, packets, frame hooks, cosmetics, settings | native HUD/key/packet/settings hooks | 6 |
| Tooltips / rich text | tooltips, colours, effects, images, previews | native tooltip hook (API 12) | 6 |
| Textures | animated (`.anim`), glow (`_e`), overlays, CTM | resource packs, `.mcmeta`; glow for entity skins (phase 1) | 7 |
| Models / animations | Blockbench block/item/entity models, keyframes, first-person hand, armour | vanilla models; Bedrock entity models (phase 1) | 7 |
| Shaders / sounds / particles | shader snippets and packs, sound categories, particles | `.hshader` packs, sounds.json, vanilla particles | 7 |
| Debugging | per-mod log filters, crash reports, mod time per tick | native mod logs and crash reports, F3 mod timing | 8 |

## 1. Custom entities and mobs — done

Inspired by Weft's entities and by Entity Model Features, Entity Texture Features and Entity Sound
Features. Players' docs: [mods.md#custom-entities](mods.md#custom-entities).

- `data/<id>/horizon/entity/<name>.json` copies a vanilla mob (`horizon_mods::entity`): size, eye
  height, health, speed, attack damage, armour, follow range, knockback resistance, category, fire
  immunity, render scale, skin, variants, emissive layer, model, animations, sounds, loot table, spawns.
- Blockbench Bedrock geometry (`.geo.json`, 1.12+ and legacy) and animations (`.animation.json`) read
  into Java model space (`horizon_mods::bedrock`): boxes unwrapped as `ModelPart.Cube`, per-face UVs,
  mirror, inflate, rotated boxes, poly meshes; bones renamed by role so the template's animation moves
  them; keyframes (linear, catmull-rom, pre/post) in Java's units, played over the template's pose
  (`idle`, `walk`, `hurt`, `death`; loop, once, hold).
- Skins, random variants picked by entity id, Weft `_e` glow masks turned into full-bright layers, ETF-style
  emissive layers (`mobs/render.rs`, the entity atlas reads mods' `textures/entity/`).
- Sounds by event or found by file name (`<event>*.ogg`), merged into the mod's `sounds.json`; modded
  sound ids keep their namespace through the server and client paths.
- Spawn rules: entries added to the named biomes' (and biome tags') `natural_mob_spawns` in the entity's
  category; spawn placement copied from the template; spawn rules follow the template's species.
- Registry ids after vanilla's (`registries.json`), `/summon` and entity argument lists, save and load
  under the mod's id, `entity.<id>.<name>` names, default loot table `<id>:entities/<name>`.
- Tests: `bedrock_tests.rs`, `content.rs` (declarations), `mobs/custom_tests.rs`,
  `mod_content_tests.rs::custom_entities_join_the_root` (build → registries, tables, mob types, sounds,
  biomes, renderer, animation, save/load).

Left for later phases: own AI goals and callbacks (2), spawn eggs (3, items), armour/held items and the
template's extra layers on a mod's own model (7), Molang (not planned: keyframes only), LAN/dedicated
clients without the same mods (they need the same content, as for blocks).

## 2. Entity behaviour and AI

- ✅ Data: `goals` in the entity declaration from a fixed vocabulary mapped to Horizon's goal
  implementations (`horizon_mods::behaviour` → `mobs/custom.rs::register`): `float`, `melee` (`reach`),
  `leap_at_target`, `ranged` (arrow, snowball, llama spit), `wander`, `look_at_player`,
  `random_look_around`, `panic`, `tempt` (items and `#tags`), `avoid`, `restrict_sun`, `flee_sun`;
  `attack_knockback`; target selectors `hurt_by` (`alert_others` calls its own kind) and
  `nearest_attackable` (entity ids; a mod's entity is matched by its id, not its template's species).
  A declared list replaces the template's selector; brain-driven templates refuse them. Players' docs:
  [mods.md#entity-behaviour](mods.md#entity-behaviour).
- ✅ Animation triggers: `attack` keyframes from each swing (`AnimState.swing_started`), and
  `Mob::play_animation(name, repeat)` (Weft's `play_animation`; native mods call
  `native_play_animation`) in place of idle and walk, synced by the entity tracker as the
  `horizon:play_animation` custom payload: at once when it changes, and with the pairing (with the
  ticks it has run) for a player who comes near later.
- ✅ Callbacks for code (API 16, `game/native_entities.rs`): native hook points `entity_tick`
  (batched per type as Weft's `on_tick` is, after the level's mobs ticked), `entity_hurt` (after the
  hit, before the death it caused), `entity_died` (before the loot), `entity_interact` (claimable,
  before the template's interaction), and helpers by entity id (read, velocity, teleport, hurt, heal,
  remove, `native_play_animation`). Cancelling or changing damage waits for phase 3's questions and
  chains. Not verified with a built native mod yet: the dispatch (batches, hits taken once) is unit tested.
- Per-entity components (Weft `entity_component`) saved in the entity's NBT under `horizon:components`.
- Kinematic platforms (`solid`, `kinematic`, `carry_height`, block decks) last, after machines.
- Tests: goal selection probes against the template's (same RNG order), callbacks in `lan_e2e`.

## 3. Blocks, items and inter-mod behaviour

- Block/item behaviour callbacks: use, place, break, neighbour change, scheduled tick, as hook points
  with data-driven forms for the common ones (drop table, light, replaceable, tool tier).
- Spawn eggs for declared entities (`"spawn_egg": {"colors": [..]}` → an item copying a vanilla egg with
  `minecraft:entity_data`).
- Weft components/tags as data (`fuel`, `food`, `hardness`, `light`) over vanilla components.
- Events, questions, value chains and attributes as a hook-point family (`horizon_mods::events` names,
  native handlers), the built-in events of Weft mapped to Horizon's call sites.
- Commands (typed builders over `horizon-command`) and a World API for native mods.
- Loot modification (`horizon:add_pools` over another table, as `horizon:add_to` does for pools) and recipe kinds.

## 4. Machines, block entities, inventories and menus

- One declaration: block + block entity + inventory groups + menu + screen layout (Weft's `r.machine`),
  over the block entity hooks (API 15) and menu hooks (API 10).
- NBT state, ticking with catch-up ticks, client sync; systems batched over a component.
- `core:energy` as a block entity component with conductor tags, networks balanced once per tick.
- Mod networking between client and server halves (custom payloads).

## 5. Liquids and worldgen

- Liquid registry: flow speed and distance, renewal, temperature, bucket item, reactions; a template
  fluid (water/lava) keeps vanilla's code paths.
- Biomes, dimensions and terrain presets from mods (today the worldgen loader reads structure bundles
  only); ores and scatter features; spawns into mods' own biomes.
- Custom generators where data alone is insufficient (native).

## 6. Client APIs

- Screen and HUD builders (data layouts + native callbacks), key bindings, packets, frame hooks.
- Entity cosmetics (`client_entity_tick`), particles, screen effects, sound categories, music.
- Mod settings shown in Mod Settings (client, server, world scopes); tooltips and rich text.

## 7. Assets and rendering

- Texture `.anim` (frametime, interpolate, frames), `_e`/`_s` glow for blocks, overlays, CTM.
- Java block/item models from Blockbench for mod blocks and items; first-person hand bone; armour and
  held items on mods' entity models; the template's layers mapped onto them by role.
- Shader snippets and packs; resource-pack layering over mod assets.

## 8. Debugging and verification

- Per-mod log filters, crash attribution, mod tick timing.
- Harnesses for each ported feature beside the vanilla parity checks.
- With no mods the vanilla code paths and resource root stay unchanged (checked by the content root
  tests: `no_content_keeps_the_base_root`).
