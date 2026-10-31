# Weftizon roadmap

This is the roadmap for **this fork**. It is separate from the upstream Horizon roadmap in
[ROADMAP.md](ROADMAP.md), which describes Horizon's own engine work and is inherited from the
fork. Use `ROADMAP.md` for Horizon feature work; use this file for Weftizon's engine + Weft-style
modding work.

The detailed Weft feature-by-feature port is in
[docs/weft-modding-port.md](docs/weft-modding-port.md). The practical modder guide is
[docs/making-mods.md](docs/making-mods.md).

## What Weftizon is

Weftizon is Horizon's Minecraft: Java Edition 26.3 Rust engine, extended with Weft's modding
surface: content registries, native code hooks, entities/mobs, blocks/items, worldgen data,
events/questions/chains, machines, client APIs and assets.

The goal is not just data-driven content mods. The goal is the same power as Weft: mods can
register content, attach behavior, intercept events and questions, build machines, add UI, and
use the world API.

## Current state

Done / substantially done:

- custom entities and mobs: vanilla templates, Bedrock models/animations, textures, glow, variants,
  sounds, spawn rules, spawn eggs, save/load
- entity behavior: declared AI goals/targets, attack animations, entity tick/hurt/died/interact
  hooks, per-entity components, solid/kinematic platforms
- content blocks: tools, tiers, tags, drops, replaceable properties
- native block hooks API 17
- content items: fuel, food, durability, attack, tools
- custom dimensions and biomes from content data
- worldgen registry carrying: biomes, noise settings, density functions, material rules,
  structures, features, presets
- loot modification via `horizon:add_to`
- auto names for items, blocks, entities and spawn eggs
- events/questions/damage chains API 18 (Warsin finished and pushed)
- TheAfterdark content port loads, custom dimension enters, legacy carver bug fixed
- custom TheAfterdark worldgen feature types implemented; skipped placed features reduced from
  62 to 0; legacy 1.20.1 block-state (DataFixer), provider, placement, `random_patch` and removed
  vanilla feature (`seagrass_simple`) support
- modder docs and porting-gotchas docs

Still missing for full Weft power:

1. **Runtime SDK / loader**
   - Weft's `declare_mod!`, C ABI, handles, builders, cdylib/zip mods loaded at runtime.
   - Weftizon currently has content `.hmod` data and native mixin bundles, not a full runtime SDK.

2. **Generic behavior API**
   - Weft's own `w.emit` events, timed attributes, full event bus, questions/chains/attributes.
   - API 18 covers many built-in events/questions/chains, but not every Weft behavior.

3. **Machines**
   - Data-driven block entity + inventory + menu + screen.
   - `core:energy`, systems, capabilities, mod networking, catch-up ticks.

4. **Liquids**
   - Flow, renewal, temperature, bucket items, reactions.

5. **Worldgen**
   - Custom terrain presets/generators, ores, scatter, spawns in mod biomes.
   - Custom Java feature algorithms still need native code.
   - Custom dimensions/biomes data now load; full feature generation is not done.

6. **Client APIs**
   - Screen/HUD builders, keybinds, packets, frame hooks, entity cosmetics, settings.
   - Currently native hook points rather than full builders.

7. **Assets and rendering**
   - CTM, animated/glow block textures and overlays.
   - First-person hand, armor, held items and template layers on custom models.
   - Full Blockbench block/item models and animation support.

8. **Items and recipes**
   - Item use/place/wearable behavior, custom recipe kinds, full loot modifiers.

9. **Tooltips/rich text**
   - Native tooltip hook exists; full Weft-style rich text/effects API is not complete.

10. **Debugging/tooling**
    - Per-mod timing, crash attribution, full mod tooling and registry/launcher polish.

## TheAfterdark status

TheAfterdark content port is done to the point of loading and dimension entry:

- assets/data load
- custom biomes load
- custom dimension `the_afterdark:afterdark` registers and can be entered
- Teleport Block/Teleport Catalyst items register and render

Not done:

- `TeleportBlockEntity` teleport behavior (optional; `/execute in` and `/tp` already enter/travel)
- `TeleportCatalystItem` use/teleport behavior (optional)
- data-only Teleport Catalyst/Teleport Block are not needed to enter the dimension; the block
  can stay decorative unless gameplay parity is wanted
- fire/lava/dripstone/grass-burn mixins
- catalyst loot modification
- the custom feature types (`spiral`, blobs, pillars, ponds, falls, cave plants) are clean-room
  approximations of TheAfterdark's Java generators, not exact ports; every placed feature its
  biomes list now decodes (0 skipped at load, from 8)
- config behavior
- full terrain/biome visual parity in the custom dimension

There is no wave AI in TheAfterdark; wave AI belongs to Invasion Mod.

## Rule for agents

Do not confuse `ROADMAP.md` (upstream Horizon) with `ROADMAP-WEFTIZON.md` (this fork).
Do not duplicate the full detailed port plan: keep [docs/weft-modding-port.md](docs/weft-modding-port.md)
as the detailed feature reference and this file as the fork's high-level roadmap/status.
