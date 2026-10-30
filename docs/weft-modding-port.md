# Weft modding port

Goal: give Weftizon the modding power of [Weft](https://github.com/VortexSo4/Weft), while keeping
Horizon's vanilla parity and existing content/native mods intact.

Weft's modding surface is a native Rust SDK: handles, builders, components/tags, events,
questions, value chains, attributes, commands, a world API, UI/HUD/keybind/packet APIs, and
data-driven assets. Horizon currently has content mods (`.hmod`, data and assets, no code) and
native mods (`.hnmod`, mixin bundles that patch named Rust functions). The port therefore has
two layers:

1. **Data registries** for content Weft mods can declare without code.
2. **Runtime/API hooks** for code, behavior, UI, events, machines, AI and worldgen.

Weft is MPL-2.0 and Weftizon's Horizon base is MIT. Implement from Weft's public docs and observed
behavior; do not copy Weft source wholesale unless the licence is handled deliberately.

## Feature inventory

| Area | Weft provides | Horizon today | Port phase |
|---|---|---|---|
| Packaging / loader | `declare_mod!`, native cdylib, C ABI, zip mods loaded in parallel | `.hmod` content mods and `.hnmod` mixin mods; no runtime SDK | Foundation |
| Names / handles | `namespace:path` names, handles with ids, generated names | content namespaces; native hooks use ids/names | Foundation |
| Blocks | hardness, tool tier, drops/loot, light RGB, cutout/translucent, model boxes, collision/selection, support rules, growth/spread, states/properties, placement, events, particles | content mods copy a vanilla template and override properties/shapes; native hooks for behavior | Data + behavior |
| Items | texture, stack, durability, tool/mining speed, attack damage, placed block, wearable, use callback | content mods copy vanilla templates and components; native hooks for use | Data + behavior |
| Entities | size, health, speed, category, texture, model, animations, sounds, AI goals, attack damage, tick/hurt/death/interact, kinematic platforms | vanilla entity types; textures from resource packs; vanilla baked models; no custom entity registry | **First feature** |
| Machines | block + block entity + inventory + menu + screen in one builder | block entities via native hook API 15; menus via API 10 | Machines |
| Liquids | flow, renewal, temperature, bucket, reactions | vanilla fluids; no content-mod liquid registry | World/registries |
| Worldgen | biomes, dimensions, terrain presets, ores, scatter, templates, structures, spawns | data-pack worldgen and worldgen JSON carry-over; custom generators still native | Worldgen |
| Recipes / loot | shaped/shapeless, recipe kinds, loot pools, loot modification | content mods carry 26.3 data-pack recipes/loot; native recipe APIs exist | Data + behavior |
| Components / tags | arbitrary typed components, per-entity components, tags, inter-mod vocabulary | vanilla components/tags; content mod tags; no generic mod components | Foundation / behavior |
| Events | batched events, emit/listen | native hooks only; no generic event bus | Behavior |
| Questions / chains / attributes | intercept, value chains, timed attributes | native hooks; no generic API | Behavior |
| Systems / energy | batched systems, `core:energy`, conductors | block entity ticks; no generic energy contract | Machines |
| Commands | typed command builders, branches, replacements | native command hooks (API 8) | Behavior/UI |
| World API | blocks, fills, entities, inventories, components, energy, menus, particles, sounds, rules, gizmos | native server/world hooks; no generic World object | Behavior |
| Client UI | screens, HUD elements, keybinds, packets, frame hooks, entity cosmetics, particles, shaders, sounds, music, settings | native HUD/key/packet/command/settings hooks; no screen builder SDK | Client |
| Textures | animated, glowing, overlays, CTM, resource packs | resource packs work; animation lacks interpolation; CTM not present | Assets |
| Models / animations | Blockbench/Bedrock geometry, keyframe animations, first-person hand, armor | vanilla entity models baked; block/item models from resources | Assets / entities |
| Tooltips / rich text | tooltips, colors, effects, images, block/item previews | native tooltip hook (API 12); vanilla text formatting | Client |
| Shaders | shader snippets and packs | Horizon shader packs (`.hshader`) and WGSL renderer | Assets |
| Debugging | per-mod log filters, crash reports, mod time per tick | native mod logs/crash reports; F3 mod timing | Foundation |

## Implementation order

Each phase is one complete feature, tested before moving on.

### 1. Custom entities / custom mobs

This is the first port because it directly addresses custom mobs and the linked Entity Model
Features, Entity Texture Features and Entity Sound Features inspiration.

- `data/<mod>/horizon/entity/<name>.json` declares an entity from a vanilla template:
  size, health, speed, category, spawn rules, sounds, texture, model, animations.
- Support Blockbench/Bedrock-style geometry and keyframe animation files in the mod's assets.
- Support per-entity textures, emissive/glow layers, random variants where feasible.
- Support ambient/hurt/death sound files per entity.
- Register spawn rules and entity AI hooks.
- Add tests for registration, model loading, textures, sounds, spawn and save/load.

### 2. Entity behavior and AI

- Entity tick, hurt, death, interact callbacks.
- AI goals and attack damage like Weft's `ai::melee`.
- Entity components and attributes.
- Kinematic/moving platforms and block decks.
- Entity events (`ENTITY_HURT`, `ENTITY_DIED`, etc.).

### 3. Blocks, items and inter-mod behavior

- Block/item behavior callbacks: use, place, break, neighbor change, scheduled tick.
- Block/item components and tags (fuel, food, hardness, light, etc.).
- Events, questions, value chains and attributes.
- Commands and the World API.
- Loot modification and recipe kinds.

### 4. Machines, block entities, inventories and menus

- Block entity NBT state, ticking and client sync.
- Inventories and slots.
- Menu/screen builders.
- Energy/capability contracts.
- Mod networking between client and server halves.

### 5. Liquids and worldgen

- Liquid registry: flow, renewable sources, temperature, buckets, reactions.
- Biomes, dimensions, terrain presets, ores, scatter, templates, structures and spawns.
- Custom generators where data alone is insufficient.

### 6. Client APIs

- Screen and HUD builders.
- Key bindings, packets, frame hooks.
- Entity cosmetics, particles, screen effects.
- Sounds, music and settings.
- Tooltips and rich text.

### 7. Assets and rendering

- Animated/glowing/overlay textures and CTM.
- Custom entity and block models/animations.
- Shader packs/snippets.
- Resource-pack layering for mod assets.

### 8. Debugging and verification

- Per-mod logs, crash attribution, mod tick timing.
- Harnesses for each ported feature, plus vanilla parity checks.
- No mods means the vanilla code paths and resource root stay unchanged.

## First concrete deliverable

Implement phase 1 (custom entities) in Weftizon, then continue in the order above. The feature must
be fully tested and committed before the next phase starts.
