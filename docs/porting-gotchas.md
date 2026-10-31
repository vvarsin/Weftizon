# Porting gotchas found by the Invasion-Mod test

The Invasion Mod port is a content-mod test: old 1.7.10 assets and registries carried into
Weftizon's `.hmod` format. It found these bugs/gotchas.

## 1. Stale game binary

A `.hmod` that passes `cargo horizon-mod check` can still fail in the game if the game binary
was built before the loader code changed. Symptom:

```text
unknown field `spawn_egg`, expected one of template, width, ... 
```

Fix: rebuild `horizon-client` after changing `horizon-mods` / `mod_content`:

```sh
RUSTUP_HOME=../rustup CARGO_HOME=../cargo-home cargo build -p horizon-client
```

Also clear the content-root cache when testing a newly generated format:

```sh
rm -rf .horizon-cache/mod-content
```

## 2. Model parents need `minecraft:`

Old mod models use `parent: "item/generated"` and `parent: "block/cube"`. In a mod namespace,
these resolve to `invasion:item/generated` / `invasion:block/cube`, which do not exist. The
item loads with no error but its icon is empty; the block model is missing.

Fix:

```json
{ "parent": "minecraft:item/generated", ... }
{ "parent": "minecraft:block/cube", ... }
```

This was the first real porting bug found. Fixed in the Invasion test and documented in
`docs/making-mods.md`.

## 3. Asset names must match declarations after snake-casing

The old texture `zombieT2old.png` snake-cases to `zombie_t2old.png`, not `zombie_t2_old.png`.
The declaration used `texture: "zombie_t2_old"`, so the mod was refused:

```text
assets/invasion/textures/entity/zombie_t2_old.png is not in the mod
```

Fix: use the actual snake-case file name in the declaration, or rename the asset to match.

## 4. Old lang keys do not map to new entity/item ids

The old lang file has keys like:

```text
entity.mod_Invasion.IMZombie.name=IM Zombie
nexusAdjuster.name=Nexus Adjuster
```

A naive `entity.` conversion collapses all of these to `entity.invasion.mod_invasion`. New ids
such as `invasion:zombie_t1a` had no name key.

Fix in Weftizon: the content root now generates default names when a key is missing:

- `item.<ns>.<name>` for items
- `block.<ns>.<name>` for blocks
- `entity.<ns>.<name>` for entities
- `item.<ns>.<name>_spawn_egg` for spawn eggs

A mod's own `lang/en_us.json` value wins. This fixed the Invasion port's missing names.

## 5. Generated spawn eggs need a lang key

Spawn eggs are generated as `<id>_spawn_egg`, but without a `lang` key the game shows a raw key,
e.g. `item.invasion.zombie_t1a_spawn_egg`. The content root now adds the key automatically and
generates a simple two-colour egg texture/model from `"spawn_egg": {"colors": [..]}`.

## 6. Loot modification works

`horizon:add_to` + `pools` correctly appends pools to a named loot table. The Invasion port's
Nexus loot table loaded. No bug found there.

## Result

After fixes, the full Invasion content port loaded and spawned:

- all item textures with custom item models
- the Nexus block and its model
- custom mobs from vanilla templates
- spawn eggs with generated names/textures
- recipes, loot tables and tags

The remaining gaps are behaviour: Invasion's Nexus machine, wave AI, custom item use and
entity behaviour need native mods or later Weft phases. Content data alone cannot port them.

## TheAfterdark port test

TheAfterdark is a 1.20.1 multi-loader mod: assets, a teleport block/catalyst, a custom
dimension and a large worldgen data set.

What loaded:

- blockstate/model/textures and the teleport catalyst/block item models
- item/block names and the custom item icons
- worldgen data files parsed without a game error
- the custom block and catalyst were placable/givable

What did not work:

- **Custom dimension.** This was originally a hole: Horizon's game dimensions were only
  Overworld, Nether and End. We now support content-mod dimensions generally:
  `dimension/` + `dimension_type/` data register the dimension, custom biomes load, and a
  synthetic `testdim:test` dimension was entered with `/execute in`. TheAfterdark still fails
  later, because its 1.20.1 worldgen schema is not 26.3: old noise-router/density fields,
  `surface_rule` instead of `material_rule`, and old block-state objects. Compatibility for
  `argument1/2`, `y_clamped_gradient` and string block states was added, but one
  `invalid world generation field: type` remains.
- **Custom worldgen feature types.** The configured features use Java feature types
  (`the_afterdark:*`). Data files alone do not register them; a native mod or generator support
  is needed once the dimension is playable.
- **Teleport block behaviour.** `TeleportBlockEntity` and the teleport logic are Java code.
  The content block is only a static block; it needs a native block-entity mod.
- **Old structure path.** The mod uses `data/the_afterdark/structures/` (plural); Horizon's
  content root expects `data/<ns>/structure/` (singular). The port renamed it to
  `structure/teleport_altar.nbt`.
- **1.20.1 item models** need 26.3 item definitions (`assets/<ns>/items/<name>.json`) in
  addition to `models/item/<name>.json`; the port added them.

So the data/assets port is clean, but TheAfterdark is not fully playable from content data
alone. Its dimension and teleport behaviour are the missing pieces.
