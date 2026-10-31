# Native mods (experimental)

Native mods are **mixin bundles** that change the game's own Rust source before it is compiled, with
[horizon-mixins](https://github.com/horizon-rust/horizon-mixins) (MIT, by levzzz5154, made for Horizon). A bundle
is a `horizon-mixin.toml` and a few `.rs` helpers that hook into named functions of the game (HEAD, BEFORE_CALL,
RETURN, MODIFY_ARG, OVERWRITE...; the tool's README lists them). A port such as Veinminer is three HEAD hooks and
one module. Start a mod from [horizon-native-mod-template](https://github.com/horizon-rust/horizon-native-mod-template).

Native mods are native code with the player's rights, not sandboxed. The launcher turns them on only when the player
installs one (after a notice) and builds them only behind a red warning; the registry lists one only after a review (its later releases are checked by machine); and
they play in a game of their own, built for the instance they are installed in.

## Pinned to a Nightly

A native mod builds on one exact Horizon commit: `[compatibility] horizon_revision` in its `horizon-mixin.toml`,
and `horizon` in the `horizon-native.json` of its registry file (`.hnmod`, `horizon_mods::index::native`). Mods
pin a **Nightly's commit** (or a release's). Every Nightly ships its source and keeps its commit for good as the
tag `nightly-source/<date>-<short>` ([release-flow.md](release-flow.md#nightly-channel)), so a pinned mod still
builds after its Nightly is replaced. When the mods of one build pin different commits, the launcher says which
mod pins which and the commit the build would use, the newest of them, and builds only after the player chooses
**Build anyway** (`native-build --allow-version-mismatch`): such mods often still apply and compile. A failed build
then says the mods were made for other versions. Their horizon-mixins pins may differ too: the build uses the
newest of them and the launcher's own, never one older than a mod was made with. A bundle with no pin builds on
the commit of the installed version the build starts from.

### Mods on the hook points

A mod that hooks only Horizon's native hook points (`crates/horizon-client/src/game/native_hooks.rs`: empty
functions the game calls at fixed places, plus helpers, kept stable across releases) can name the version it
needs instead of pinning: `"hooks": 1` in its `.hnmod`'s `horizon-native.json`, the `NATIVE_HOOKS_API` of that
file when it was made. Hook points are only ever added: each addition raises `NATIVE_HOOKS_API`, and a mod builds
on any Horizon whose version is the one it needs or a later one. Its `horizon` is then only the commit it was tested on. The launcher builds it on the commit the other mods
pin, else on the installed version's commit (the game the player plays), after checking that commit's
`NATIVE_HOOKS_API` (read from GitHub) is at least the mod's; when it isn't and nothing pins, it builds on the commit the
mod was tested on. In its own copy of the mod it sets `horizon_revision` to the commit it builds on. Horizon's
own native mods (Zoom, FPS counter, Keystrokes, Minimap; horizon-rust/horizon-mods, `native/<name>/`) are made this way, so any of them build together
with no re-release for each Nightly.

API 2 adds the item panel beside container screens and its recipe pages (`native_panel_items`,
`native_panel_recipes`, and the hook `native_panel_event` for the player's clicks, searches and keys over items), and
the game's items and recipes (`native_recipes`, `native_known_recipes`, `native_place_recipe`, `native_give`): what a
recipe viewer needs.

API 11 adds `GameLevel::native_send_particles` on authoritative servers. It sends supported
vanilla particles to nearby players. Block particles require a valid block state. Item, trail and
vibration particles are not supported by the current dispatch. Count zero sends one directional
particle. The return value says whether the call was accepted, not whether any player received it. Bundles that use it need `"hooks": 11`.

API 3 adds authoritative world lifecycle events through `native_world_lifecycle`: open ready, open failed,
and final close. Bundles that use these events need `"hooks": 3`.

API 4 adds authoritative server tick events through `native_server_tick`: start (before any level ticks, like NeoForge's `ServerTickEvent.Pre`) and end of each non-paused
server tick, with `world_running`. A frozen world reports false except during a requested step. Bundles that
use these events need `"hooks": 4`.

API 8 adds server commands and players: `native_register_commands` (the commands a mod adds to every player's
command tree) and `native_perform_command` (a player's command on the server), with helpers to answer
([Server commands and players](#server-commands-and-players)). Bundles that use them need `"hooks": 8`.

API 9 adds the acting player's plain items for server mods (count, take, give; [Server commands and
players](#server-commands-and-players)). Bundles that use them need `"hooks": 9`.

API 10 adds server menus: a vanilla chest menu a server mod opens and fills, whose clicks and closing reach the mod
(`native_menu_clicked`, `native_menu_closed`), and `native_exchange_items`, two players' stacks swapped in one checked
step ([Server menus](#server-menus)). Bundles that use them need `"hooks": 10`.

API 12 adds item tooltips through `native_tooltip`: the hovered stack, with its components, and its tooltip's
lines, which a mod can add to, change or take out of, wherever the game shows an item's tooltip. Bundles that use it
need `"hooks": 12`.

API 13 adds using an item, on the server for every player: `native_use_item_on` hears a right click with an item on a
block once the block let it through (`ItemStack.useOn`), and `native_use_item` hears the item's own use, in the air or
after `native_use_item_on` let it through (`Item.use`). Each hears a click once. A mod that claims one swings the hand
instead of the item's use. Bundles that use them need `"hooks": 13`.

API 14 adds breaking and placing a block, on the server for every player: `native_destroy_block` hears a player break a
block once the game allowed it (`ServerPlayerGameMode.destroyBlock`) and may cancel it, which keeps the block and its
drops; `native_place_block` hears a block placed from an item (`BlockItem.place`). To stop a placement, a mod claims
`native_use_item_on`, which runs before it. Bundles that use them need `"hooks": 14`.

API 15 adds block entities for a mod's blocks (`native_block_entity`, `native_block_entity_ticks`,
`native_block_entity_tick`): made when the block is placed, ticked on the server and saved with the chunk. Bundles that use them need `"hooks": 15`.

API 16 adds content mods' entities, Weft's `on_tick`, `on_hurt`, `on_death` and `on_interact` for the mobs a content
mod declares ([Entities](#entities)): `native_entity_tick`, `native_entity_hurt`, `native_entity_died`,
`native_entity_interact`, and helpers to read, move, hurt, heal, remove and animate an entity
(`native_play_animation`). Bundles that use them need `"hooks": 16`.

API 17 adds blocks, Weft's block `on_use` and `on_scheduled` and the World API's blocks ([Blocks](#blocks)):
`native_use_block`, `native_block_tick`, and helpers to read a block, set it by name, change one property and
schedule a tick. Bundles that use them need `"hooks": 17`.

API 18 adds Weft's built-in events, questions and value chains ([Events and questions](#events-and-questions)):
`native_pickup_item`, `native_item_picked_up`, `native_player_joined`, `native_player_left`,
`native_attack_entity`, `native_enter_dimension`, `native_hurt`, `native_damage` and `native_attack_damage`.
Bundles that use them need `"hooks": 18`.

API 19 adds content mods' machines (`native_machine_tick`). Bundles that use it need `"hooks": 19`.

API 20 adds a level's own tick and the World API an invasion needs ([Levels, waves and boss bars](#levels-waves-and-boss-bars)):
`native_level_tick`, and helpers to spawn a mob, walk it somewhere, tell whether its path reaches, break a block
with its crack, read a block's hardness, and show a boss bar. Bundles that use them need `"hooks": 20`.

API 21 adds spawning with gear, effects, lightning, weather, projectiles and block search
([Gear, effects, weather and projectiles](#gear-effects-weather-and-projectiles)). Bundles that use them need
`"hooks": 21`.

API 22 plants creaking hearts in trees for content mods' creaking variants
([Creaking hearts in trees](#creaking-hearts-in-trees)). Bundles that use them need `"hooks": 22`.

## What the launcher does

In the window: install a native mod from the Mods page. The first one shows a notice once (the first build downloads
Rust and can take 20 to 30 minutes; later builds are faster) and turns on Settings → Game → **Native mods
(experimental)**, which stays off for players who never install one; `mods install` does the same. Then **Build**. The launcher names the mods, warns in red, and builds only from "I trust these mods: build"; the
row shows each step, the error if one fails, and the build the instance plays. From the command line:
`horizon-launcher native-build [FOLDER] [--instance NAME] --trust-native-code`. FOLDER is a bundle, an unpacked
`.hnmod`, or a folder of them; without it, the native mods turned on in the instance. It is not an MCP tool:
trusting native code is the player's decision, never an agent's.

1. **Rust.** The launcher's own toolchain ([`rust_toolchain`](../launcher/src/rust_toolchain.rs)): Rust 1.99.1's
   official `rustc`, `cargo` and `rust-std` (and `rust-mingw` on Windows), each pinned by SHA-256, downloaded once
   from static.rust-lang.org into `<launcher data>/rust/`, with its own Cargo home `<launcher data>/cargo/`. No
   rustup, nothing on the system; the player's own Rust is never used. Windows uses the `windows-gnu` host, which
   links without Visual Studio. `rust-mingw` only links, so on Windows the toolchain also brings MinGW-w64
   (`rust_toolchain::MINGW`: WinLibs' GCC 16.2.0 on MinGW-w64 14.0.0, MSVCRT like `rust-mingw`, one zip pinned by
   SHA-256, its `mingw64/` without the documentation) on the build's `PATH`: `ring` and `hidapi` compile C,
   and `windows-sys`' `raw-dylib` imports need `dlltool` and the assembler it runs. The linker is the system's on
   macOS (the Command Line Tools: the launcher asks for `xcode-select --install`) and Linux (`cc`).
   `horizon-launcher rust status|install|remove`.
2. **horizon-mixins**, at the commit the mods name (their `.hnmod`'s `mixins`), else `native_mods::MIXINS_REV`,
   built once with that cargo (`cargo install --locked --git … --rev …`) into `native/tools/<commit>/`.
3. **The source** of the pinned commit: GitHub's archive of it, once per commit (`native/source/<commit>/`, about
   63 MB unpacked), with the commit in `.horizon-revision`. Never the player's own checkout. Links in the
   archive are left out: the tool refuses them and nothing needs them to build.
4. **Mixins.** The bundles are copied first, so the build and its key use that copy whatever happens to the
   folder meanwhile. `horizon-mixins apply` stages the modded source; the launcher copies only the files that
   changed into `native/stage/`, so Cargo, which goes by file times, recompiles only the crates the mods touch.
5. **Compile**: `cargo build --locked --profile release --features horizon-client/mods -p horizon-client`, the
   release's own command (game-build.yml), into `native/target/`. Cargo downloads the locked crates from
   crates.io (Horizon has no git dependencies). The log is `native/build.log`; a failure shows its last lines (a
   horizon-mixins diagnostic in one line).
6. **Install**: a new version, `native-<commit>-<key>`, a copy of the starting version's folder (resources
   included, cloned on APFS, Btrfs and ReFS) with the built game in place of its own, and a `native-mods.json`
   saying which commit, mods, horizon-mixins and Rust made it. The instance the mods are installed in plays it (a
   folder built from the command line, or mods of the default home, go to the **Native mods** instance). That
   instance gets each mod's settings from the default home the first time it plays it (its
   `horizon-mods/.horizon-mod-data/<id>/` folder and `key.<id>.` lines of `horizon-mods.txt`, where earlier versions of
   the mod kept them); nothing it already has is overwritten.

## Install from GitHub

An author can ship a native mod without a registry pull request per version: the player installs it from a
GitHub link, on the Mods page ("Add from GitHub") or with `horizon-launcher mods add-github <url>
[--mod ID | --all] [--instance NAME] --trust-native-code`. The link is `https://github.com/<owner>/<repo>`
(its latest release), `.../releases/tag/<tag>`, `.../tree/<ref>` or `.../commit/<sha>`.

- **Resolved to something exact.** GitHub's public API (no sign-in; a rate limit is said plainly, and
  `HORIZON_GITHUB_TOKEN` raises it) turns the link into a release's `.hnmod` file or a **commit SHA**, never a
  branch that moves. A release with a `.hnmod` asset installs that file (its SHA-256 is compared with the one GitHub
  lists). With no release or no such asset, the launcher fetches
  `codeload.github.com/<owner>/<repo>/tar.gz/<sha>` and looks in it for mods by the rules of
  `native-build <folder>`: a folder with `horizon-mixin.toml`, or an unpacked `.hnmod` (`horizon-native.json`).
  Several mods: they are listed, and the player picks one or installs all. A source folder must pin the Horizon commit
  (`[compatibility] horizon_revision`); the launcher packs its Rust, TOML and `data/` files into a `.hnmod`.
  A release that ships only a content mod (`.hmod`) says "not supported yet": installing `.hmod` files from a link
  is not built.
- **Checked like the registry's files.** The `.hnmod` is read as `horizon_mods::index::native::Bundle::read` reads a
  registry file (only Rust, TOML and data under `mixins/<id>/`, size limits, nothing run) before it is installed.
- **Said first.** Before anything is downloaded: "not reviewed by the Horizon registry; native mods run with full
  access to this computer; read the source", with a link to the exact commit. The command needs
  `--trust-native-code`, as `native-build` does. It is not an MCP tool: trusting native code is the player's
  decision, never an agent's.
- **Origin kept.** `horizon-native-mods/<id>.origin.json` beside the `.hnmod`: owner/repo, release tag (for a
  release), commit, asset name and the SHA-256 of the installed file. `mods list` shows "from GitHub: owner/repo @
  tag" (or the commit's first 7 digits).
- **Updates.** `horizon-launcher mods update-github [ID] [--check]` compares a mod from a release with the
  repository's latest release, and a mod from a commit with the head of the default branch; a newer one is
  installed with the same warning and `--trust-native-code`. `mods remove <id>` removes it. A mod already
  installed from the registry (or by hand) is never replaced.

Installing turns Native mods on, as any native mod does; then build as usual.

## Cache and disk

The key hashes the commit, the horizon-mixins commit, Rust's version, the profile and features, and every file of
every bundle. The same set on the same commit is never built twice. A change gives a new build, and
`native/target/` stays, so a rebuild compiles only `horizon-client` and what the mods touched.

Measured 2026-10-28 on the Mac mini (M4 Pro, 14 cores) with Veinminer 0.3.0 on Nightly `83f15fa`,
`CARGO_BUILD_JOBS=8` while other agents' builds ran (upper bounds):

| What | Size, time |
|---|---|
| Rust 1.99.1 (`<launcher data>/rust/`) | 453 MB; 58 s to download and unpack |
| horizon-mixins (`native/tools/`) | 7 MB; built once in 19 s |
| Crates in the launcher's Cargo home (`<launcher data>/cargo/`) | 289 MB |
| Source of one commit, and the stage | 64 MB each |
| First build: release profile, the whole game | 9 min 10 s of compiling, 9 min 41 s in all; `native/target/` 1.5 GB |
| Rebuild after a helper's code changed | 2 min 46 s of compiling (only `horizon-client`), 2 min 53 s in all |
| A change horizon-mixins ignores (a comment) | nothing compiled; 31 s, mostly the version folder's copy |

Measured 2026-10-29 on Windows 11 (Ryzen 7 9800X3D, 16 threads), the same mod and Nightly, an empty `native/target/`
(horizon-mixins, the source and the crates were already downloaded):

| What | Size, time |
|---|---|
| Rust 1.99.1 and MinGW-w64 (`<launcher data>/rust/`) | 1.7 GB, MinGW-w64 875 MB of it |
| Crates in the launcher's Cargo home | 657 MB |
| First build: release profile, the whole game | 6 min 57 s of compiling, 8 min 3 s in all; `native/target/` 2.0 GB |

Measured 2026-10-28 on Linux (an x86_64 server with no Rust of its own), Zoom on Horizon `1149c92`, 3 jobs while CI
ran: `rust install` in 11 s (592 MB), the first build in 12 min 9 s. The built game ran Zoom's proof (under `xvfb-run`:
the game needs a display even with `--offscreen`).

Only the native builds an instance plays are kept (`native_mods::prune`); `native/target/` stays for the next one.

## Trust

- **Off until the player installs one.** Settings → Game → Native mods stays off for a player who never installs a
  native mod; the first install shows the notice and turns it on. The command needs `--trust-native-code`; the window
  shows a warning in red before every build: the mods run with full access to the computer, and their ids and
  versions. The Mods page says who made each one, and "Not made or endorsed by the Horizon project" for any mod not
  Horizon's own.
- **The registry** (horizon-rust/horizon-mods) lists them as their own kind, `native-mod`, only after a maintainer
  has read the source and the repository they come from. Its entry names the GitHub repository (`repo`); each
  release of it with a `.hnmod` asset is a version once the bundle passes the registry's checks, with no further
  review (nothing is built or run by the registry; the player's launcher builds it behind its warning). The `.hnmod` contains `.rs` and `.toml` files and may contain data files under
  `mixins/<id>/data/`. The launcher checks the bundle without running anything. Native code is trusted.
  The file is pinned by SHA-256 like every registry file, and the version, game and pins are the bundle's own
  `horizon-native.json`. `revoked.toml` takes a bad release down.
  A content mod's entry may turn `native-mod` once (never back): its `.hmod` versions stay published and
  are checked as `.hmod`s, the index lists only its `.hnmod` versions, and the launcher's update keeps the
  player's `.hmod`; removing the mod removes both.
- **Isolation of data.** Native mods run only in the instance built for them, with its own worlds, options and logs.
- What this does not do: sandbox the compiled game. horizon-mixins' checks stop mistakes, not a malicious helper
  ([security notes](https://github.com/horizon-rust/horizon-mixins/blob/main/docs/security.md)).

## Multiplayer

- **Single player and LAN host:** the mods run in the host's game and its integrated server, so a server-side
  hook applies to every player of the world.
- **Joining a server:** only client-side hooks do anything; the protocol is unchanged.
- **Dedicated servers:** native mods are not built yet. Lifecycle hook call sites are ready for when the pipeline builds them (`--no-default-features`).

## Left to do

- Rebuild after the mods or the Nightly change (asking first, as it compiles for minutes).
- A prebuilt horizon-mixins from its CI instead of building it on the player's computer.

## Named hook catalog (draft)

`native-hooks.json` maps stable hook names and contract versions to the current
source targets. A contract includes the signature, call timing and behavior.
Keep an existing contract or add a new version when it changes incompatibly.
The catalog does not cover helper methods or arbitrary private game code.
Named injection resolution needs a matching horizon-mixins change; existing raw
targets and the `hooks` metadata continue to work as before.

### Reading server blocks (draft)

A native server tick hook can call `native_server_block` with its context, a dimension id and a block position. It returns the block name and state properties from a loaded chunk. It does not load a chunk or open a dimension. Reads outside that hook, from a stale context, from unavailable dimensions or outside the world bounds return an error.

### Neighbour observations (draft)

`native_neighbor_changed` observes each authoritative server `neighborChanged` before the block handles it. It supplies the dimension, position, current state ID, changed block ID and moved flag. Resolve IDs with the level's registries during the hook; the hook does not load chunks or change the update queue.

### Server block changes (draft)

`native_block_changed` observes a committed authoritative server `Level.setBlock` change before block side effects and neighbour updates. It supplies the dimension, position and old and new state IDs. It does not observe chunk loading or direct storage writes that bypass `Level.setBlock`.

### Server commands and players

For mods that run where the world is simulated: a dedicated server, a LAN host, single player's server thread
(`crates/horizon-client/src/game/native_server.rs`). The SMP mod of the official server is built on them
([smp.md](smp.md)).

- **`native_register_commands(commands)`**: push a `NativeCommand` per usage, asked once when a world opens to
  players. A usage is words, then arguments `<name:kind>`, optional ones last in brackets:
  `NativeCommand::new("pay <player:player> <amount:integer>")`, `NativeCommand::op("eco set <player:player>
  <amount:integer>", 3)` for operators of level 3. Kinds: `word`, `text` (the rest of the line, last), `integer`,
  `number`, `boolean`, `player`, `item`, `block_pos`. The usages of one command merge into one tree, which every
  player receives after vanilla's (filtered by their level, as vanilla's is), so their chat box completes and colours
  it. A name vanilla has, or a usage that doesn't parse, is left out with a warning in the log.
- **`native_perform_command(player, command, ran)`**: a player typed a registered command (without its slash) and
  their level allows it, whether the world allows commands or not. The mod parses the line and sets `ran`. The
  player's state is swapped in for the call, as for their other actions, so `native_player` reads them; a
  registered command no mod takes answers as vanilla's unknown command.
- Helpers: `native_server_players()` (uuid, name, level, dimension, position and game mode of each player on);
  `native_tell(uuid, text)`, a system line to one player; `native_broadcast(text)`, to every player and the server's
  log; `native_feedback(text)`, the command's answer to the player whose command it is; `native_world_dir()`, the
  open world's folder, where a server mod keeps its data in a folder of its own so the world's backups carry it.
- Items (API 9), of the acting player (inside `native_perform_command`, whose player is swapped in): `native_held()`
  (the main hand's item, count, and whether it is plain), `native_count_items(item)`, `native_carried_items()`,
  `native_take_items(item, count)` (the main hand first, then as `Inventory.add` orders slots),
  `native_room_for(item)` and `native_give_items(item, count)` (only where it fits: nothing is dropped). They look at
  the 36 inventory slots and the off hand, never the armour, and only at **plain** stacks: no damage and no
  component (name, enchantment...), so a shop never takes a worn or enchanted item for a new one. Each returns how
  many it counted, took or gave; the player's screen follows as for any change to their inventory.

### Server menus

API 10, `crates/horizon-client/src/game/native_menus.rs`: menus a server mod builds from ordinary items, which a
stock client shows as a chest (`generic_9x1` to `generic_9x6`); nothing is installed on the player's side.

- `native_open_menu(uuid, rows, title, items)` opens one in place of the player's open menu (its window id);
  `native_set_menu_items(uuid, items)` changes what it shows; `native_close_menu(uuid)` closes it.
- **No click moves anything.** Each reaches `native_menu_clicked(uuid, window, slot, button, mode)`, and the client
  is sent the menu, its inventory and its cursor as they were. `slot` counts the menu's own slots first, then the
  player's inventory below it: `native_menu_inventory_slot(size, slot)` gives the inventory slot (0..9 the hotbar).
- `native_menu_closed(uuid, window)`: the player closed it, another menu opened, or the player left.
- `native_player_stacks(uuid)`: the player's 36 inventory slots, to show copies or offer them.
- `native_exchange_items(a, a_offer, b, b_offer)`: each offer is inventory slots, each with the exact stack it must
  still hold. Both offers leave their slots and go to the other player where they fit whole (onto matching stacks,
  then empty slots), or nothing moves and the error says why (an offer that changed, no room, a player gone or
  dead). A trade on these menus leaves the offered stacks in their owners' inventories until this one step, so
  closing the menu, leaving or dying moves nothing.

The menu hooks run with no player's state swapped in; `native_player_stacks` and `native_exchange_items` refuse a
player whose state is (inside `native_perform_command`): call them from a menu hook or `native_server_tick`.

### Entities

API 16, `crates/horizon-client/src/game/native_entities.rs`: code for the mobs a content mod declares
(`data/<id>/horizon/entity/<name>.json`, [mods.md](mods.md#custom-entities)), so one mod can be a content mod's
declarations and a native mod's behaviour, as a Weft mod's `r.entity(..)` is. The hooks name the entity's type
(`mymod:ghost`): check it is yours. They run where the world is simulated, for every player of it.

- **`native_entity_tick(entity, ids)`**: once a tick for each type with any alive in the level, after every mob of the
  level ticked: all its entities at once (Weft's `on_tick`; batch your work over `ids`).
- **`native_entity_hurt(entity, id, damage)`**: a hit landed (Weft's `on_hurt`, the `ENTITY_HURT` event): the damage
  type (`minecraft:player_attack`, `minecraft:fall`...), what it took of the health, whether a player dealt it and the
  attacking mob's id. Heard after the hit, at the end of the tick it landed in or the start of the next, always
  before the death it caused. Stopping or changing damage is not a hook yet (Weft's `question::HURT` and
  `chain::DAMAGE`, roadmap phase 3).
- **`native_entity_died(entity, id, damage)`**: it dies, once, before its loot drops (Weft's `on_death`).
- **`native_entity_interact(entity, id, stack, claimed)`**: a player right clicks it, before the template's own
  interaction (Weft's `on_interact`); set `claimed` to take the click: the hand swings and nothing else happens. The
  acting player's state is swapped in, as for `native_use_item`.
- Helpers, by entity id, for mobs of the level the hook runs in: `native_entity(id)` (type, position, velocity,
  rotation, health, age, target, the animation playing), `native_entities_of(entity)`,
  `native_set_entity_velocity`, `native_teleport_entity`, `native_hurt_entity` (generic damage),
  `native_heal_entity`, `native_remove_entity` (no death, no loot) and `native_play_animation(id, name, repeat)`
  (Weft's `play_animation`: a keyframe animation of the entity's file in place of idle and walk, once or over and
  over, `""` to stop; every player who sees the entity sees it).

### Blocks

API 17, `crates/horizon-client/src/game/native_blocks.rs`: code for blocks, a content mod's
(`data/<id>/horizon/block/<name>.json`, [mods.md](mods.md#content-mods-api-02)) or any other, as a Weft mod's
`r.block(..)` callbacks and its World API's blocks are. They run where the world is simulated, for every player of it.

- **`native_use_block(stack, pos, face, state, claimed)`**: a player right clicks a block, where the block's own use
  starts (Weft's `on_use` for blocks, and `question::USE_BLOCK`: check the block is yours, or claim any block to
  protect it). Set `claimed` to take the click: the hand swings and neither the block's use (a door, a chest's menu)
  nor the item's runs. Not heard while the player sneaks with something in either hand, since vanilla's blocks
  don't react then either. The acting player's state is swapped in.
- **`native_block_tick(pos, state)`**: a scheduled tick of a content mod's block came due (Weft's `on_scheduled`),
  after its template's own tick, in vanilla's order of scheduled ticks.
- Helpers, for the level the hook runs in: `native_block(pos)` (name and properties, `None` where nothing is
  loaded), `native_set_block(pos, "ruby:lamp[lit=true]")` (as `/setblock`: neighbours update, block entities are
  replaced), `native_set_block_property(pos, "lit", "true")` (Weft's `set_prop`) and
  `native_schedule_block_tick(pos, delay)` (Weft's `schedule`, vanilla's `Level.scheduleTick`).
- Weft's `on_neighbor`, `on_placed` and `on_broken` are the hooks already there: `native_neighbor_changed` (API 6),
  `native_block_changed` (API 7, every committed change), `native_place_block` and `native_destroy_block` (API 14,
  a player's). Weft has no random ticks; vanilla's still reach a mod's block through its template.

### Levels, waves and boss bars

API 20: what a mod like Invasion Mod needs to run waves of mobs that walk to a target and dig through walls. All of
it runs where the world is simulated, for every player of it.

- **`native_level_tick(dimension)`**: once a tick for each level the server ticks (`minecraft:overworld`, a mod's
  dimension...), after its mobs ticked, with that level in place, so the helpers below act on it. Not called while
  the world is paused or frozen. Keep the wave's state in your bundle's own module.
- Mobs (`native_entities.rs`): `native_spawn_entity(kind, position)` (Weft's `spawn`: a persistent mob, finalized as a
  natural spawn is; `None` for a type that isn't a mob, or a monster on Peaceful), `native_navigate(id, target,
  speed)` (Weft's `navigate`, vanilla's `PathNavigation.moveTo`: true when a path was found; speed 0 stops it),
  `native_navigating(id)` (still walking; false once it arrived, ran out of path or got stuck) and
  `native_path_reaches(id)` (the path ends at the target; false when a wall is in the way and the path stops at the
  nearest block it can reach). A goal that moves the mob (melee, wander) can take its path over, as in vanilla: call
  `native_navigate` again every few ticks.
- Blocks (`native_blocks.rs`): `native_break_block(pos, drop)` (as a mob or an explosion breaks it:
  particles, sound, drops without a tool when `drop`), `native_block_hardness(pos)` (stone 1.5, obsidian 50; `None`
  for bedrock and barriers) and `native_block_crack(breaker, pos, progress)` (the crack stage 0 to 9 every player
  near it sees, as a zombie's on a door; any other number clears it; one crack per breaker id).
- Boss bars (`bossbars.rs`): `native_boss_bar(id, name, value, max, color)` makes or changes a custom bar shown to
  every player (the world saves it, as `/bossbar` bars are saved); `native_remove_boss_bar(id)` takes it away.
- Players: `native_server_players()` (API 8) gives their positions and dimensions; `native_broadcast` writes to all
  of them.

A digging mob: walk it to the target every 10 ticks; when `native_path_reaches` is false and `native_navigating`
turns false, read the block in front of it, raise its crack over a time that follows its hardness, then break it.

### Gear, effects, weather and projectiles

API 21, `crates/horizon-client/src/game/native_world.rs`. Call them from `native_level_tick` or another server hook;
they act on the level in place.

- **`native_summon(kind, position, nbt)`**: any entity from its saved data, as `/summon <kind> <pos> <nbt>`:
  `{equipment:{mainhand:{id:"minecraft:trident",count:1}}}` (a drowned with a trident), `{powered:1b}` (a charged
  creeper), `{Fire:2000s}`, `{active_effects:[...]}`, names, attributes, riders. Add `PersistenceRequired:1b` so it
  never despawns. Like `/summon` with data, there are no natural equipment or variant rolls. Its id, or `None`.
- **`native_mob_effect(id, effect, ticks, amplifier)`** and **`native_player_effect(uuid, effect, ticks,
  amplifier)`**: as `/effect give` (`ticks` -1 is for ever). `minecraft:glowing` outlines a mob through walls.
  Immune mobs refuse (a wither to wither).
- **`native_set_entity_fire(id, ticks)`**: sets a mob on fire. Fire hurts it and rain puts it out: for a mob that
  burns for ever, give it `fire_resistance` for -1 ticks and call this again every second.
- **`native_lightning(position, visual_only)`**: a bolt; `visual_only` keeps only its flash and thunder.
- **`native_set_weather(kind, ticks)`**: `clear`, `rain` or `thunder`, as `/weather`. Call it from the overworld's
  level tick: its weather is the world's.
- **`native_shoot(kind, from, direction, speed, owner, burning)`**: `arrow`, `trident`, `snowball`, `egg`,
  `small_fireball`, `fireball`, `wither_skull`, `wind_charge` or `llama_spit`, shot as a mob shoots it. `owner` is
  the mob it counts as shot by; `burning` lights it as Flame does (a burning arrow sets what it hits on fire).
- **`native_find_blocks(center, radius, block, limit)`**: positions of `minecraft:obsidian` or of a tag
  (`#minecraft:logs`) within `radius` (at most 32), nearest first. Only loaded chunks.

Tested in a game (2026-10-31): a drowned summoned with a trident killed the player with it, a charged creeper
glowed through the darkness the player was given, the thunder started, burning arrows flew, and the logs of a
nearby tree were found.

### Creaking hearts in trees

API 22, `native_world.rs`, for content mods' creaking variants ([mods.md](mods.md#creaking-variants)):

- **`native_plant_creaking_heart(center, radius, creaking)`**: the nearest trunk within `radius` (at most 32) of a
  tree whose wood grows a creaking, a log with logs of the same wood above and below, becomes a dormant heart that
  wakes at its creaking's hours. One heart to a tree. `creaking` (`invasion:spruce_creaking`) limits it to trunks
  that grow that one. Returns the heart's position and the creaking it grows.
- **`native_creaking_for_log(name)`**: the creaking a log grows: a mod's variant, `minecraft:creaking` for pale oak,
  or none.

Tested in a game (2026-10-31): in a dark oak forest, 4 hearts went into 4 trees (oak, birch, dark oak) and at
midnight the oak and birch creakings came out while the dark oak one, active by day, did not.

### Events and questions

API 18, `crates/horizon-client/src/game/native_events.rs`: Weft's built-in events (heard, nobody can stop them),
questions (any mod may say no: set `canceled`, and any `canceled` wins) and value chains (each mod changes the
amount in turn, seeing the change before it), for every player of the world where it is simulated. The acting
player's state is swapped in, so `native_player` and the item helpers read them.

| Weft | Hook | When |
|---|---|---|
| `question::PICKUP_ITEM` | `native_pickup_item(stack, canceled)` | a player touches an item on the ground it could take; asked each tick it touches it |
| `ITEM_PICKED_UP` | `native_item_picked_up(stack, count)` | after it went into their inventory: `count` of it, all or what fit |
| `PLAYER_JOINED` | `native_player_joined(uuid, name)` | the server added the player (the window's own player in single player too); `native_player_left` when they go, not when the server stops |
| `question::ATTACK` | `native_attack_entity(id, canceled)` | a player's hit on a mob or another player, before its damage and knockback; the arm still swings |
| `chain::ATTACK_DAMAGE` | `native_attack_damage(target, amount)` | a player's melee hit on an entity of type `target`: their attack damage before the charge, enchantments and critical hit scale it |
| `question::HURT` | `native_hurt(hit, canceled)` | any hit on a mob or a player, whatever caused it (attacks, arrows, falls, fire, `/damage`...); a stopped hit does nothing at all |
| `chain::DAMAGE` | `native_damage(hit, amount)` | what a hit nobody stopped deals, before armour, enchantments and effects reduce it; below zero deals none |
| `question::ENTER_DIMENSION` | `native_enter_dimension(dimension, canceled)` | a player is about to go through a Nether or End portal, or ride through one (a rider kept back keeps the vehicle too); asked again after the portal's cooldown |

`native_hurt` and `native_damage` have no `self`: they are asked inside the hit, while the game is busy with the
mob or player, so they decide from the `Hit` alone (`horizon_client::hits::Hit`: the entity's id and type, the
damage type, the amount, whether a player caused it and the mob that did). On a hit on a player, `by_player` and
`attacker` are not filled in yet. A dead mob or player is not asked, and a hit a mod lets through can still do
nothing (invulnerable time, a shield, a creative player: some of those checks come before the question, some after).

The others map to hooks already there: `BLOCK_BROKEN` and `question::BREAK_BLOCK` are `native_destroy_block`
(API 14), `BLOCK_PLACED` is `native_place_block` and `question::PLACE_BLOCK` claiming `native_use_item_on`,
`question::USE_BLOCK` is `native_use_block` (API 17), `question::USE_ITEM` and `ITEM_USED` are
`native_use_item` (API 13), `question::INTERACT_ENTITY` is `native_entity_interact` and `ENTITY_HURT` /
`ENTITY_DIED` are `native_entity_hurt` / `native_entity_died` (API 16, a content mod's entities). Not asked yet:
a player's hits on boats, minecarts, item frames, armour stands and the Ender Dragon (`native_attack_entity`),
and travel that isn't through a portal (respawning, the End's credits). Weft's own events (`w.emit`) and timed
attributes are not part of this API.
