# Handoff

Read [AGENTS.md](AGENTS.md) first, then [ROADMAP.md](ROADMAP.md) (the fork's only roadmap: what is done and what
is next in the Weft port). This page only records the state an agent picks up.

## Remotes

- `origin`: `git@github.com:vvarsin/Weftizon.git`, the fork. `main` is pushed here.
- `upstream`: `https://github.com/horizon-rust/horizon.git`. As of 2026-11-26 it answers 404 (private or moved),
  so `git fetch upstream` fails. The last upstream commit merged is `4626921895` (2026-11-15 22:13,
  "tooling(claims): an issue's author has the first hour to claim it (#17142)"), fetched from a public fork
  that still carries upstream's history: `https://gitlab.com/aaravmain.org/related/horizon-web.git` (a web port;
  only upstream's own commit was merged, none of the port's). Local branch `upstream-snapshot-2026-11-15`.
  Other public forks with upstream history: GitHub `Lciarallo/horizon-nx` (to 2026-11-15 12:49, then GPL),
  `Rayan-XOO/horizon` (to 2026-11-13).

## Second upstream sync (2026-11-26, to `4626921895`)

6 224 upstream commits. What kept the modding layer working, beyond the first sync's notes below:

- Upstream split many files; Weftizon's hooks moved with the code: content block ticks in
  `game/block_ticks.rs`, `native_use_blocks` in `game/use_hand.rs`, mod recipe ids in `game/menu_clicks.rs`,
  `native_attack_amount` in `game/melee.rs`, `native_attack_refused` in `game/lan_actions/guest_attack.rs`, custom
  mob steps in `mobs/ai/step.rs`, `Dim::load_custom` in `game/startup_menu.rs` (`startup_resource_root`).
  `game/discord.rs` became upstream's `horizon-discord` crate and `game/activity.rs` (custom dimensions report as
  the overworld).
- Custom dimensions use the overworld's `DimensionFacts` (upstream's `LevelKey::builtin_or_overworld`).
- `crates/horizon-world/src/worldgen/feature_graph.rs`: the TheAfterdark feature types decode in a non-inlined
  `decode_mod_kind`, so deep nesting does not overflow the stack (upstream's depth-limit test).
- The user does not want features that force an account: upstream's 1e789800af ("launcher: remove offline
  names; play only as an account that owns the game") is reverted. Keep it reverted on later syncs, and drop any
  other upstream change that makes an account, sign-in or online check mandatory to play.
- InvasionV2 native mod repinned to this tree; NATIVE_HOOKS_API is now 645 (bundles at 413 still load).
- Single player runs on the server thread, where the window's player is a guest. `guest_view`
  (`game/lan_mobs.rs`) used to clear the guest's team allies, so mobs on the player's team (InvasionV2's
  Skeleton Piglins) still hunted them; it now keeps them (492782d058). InvasionV2 is pinned to that commit.
- Testing a game window on DP-1 without it flashing on the main monitor: copy the Save The City
  `tools/desktop.py`, change its catcher's `indexOf("polytoria")` to `indexOf("horizon")`, `catch-start` before
  launching and `catch-stop` after.

## First upstream sync (2026-11-26, to `fa7924b3fb`)

`main` merged upstream/main (13 224 commits) on top of Weftizon's 81 commits. What kept the modding layer working:

- Weftizon's own native hooks (17: `entity_tick`, `entity_hurt`, `entity_died`, `entity_interact`, `use_block`,
  `block_tick`, `pickup_item`, `items_picked_up`, `player_joined`, `player_left`, `player_attack`,
  `attack_damage`, `hurt`, `damage`, `enter_dimension`, `level_tick`, `machine_tick`) moved out of
  `native-hooks.json` (upstream's catalogue, taken as is) into `tools/native-hook-fragments/weftizon.json`, which
  the build-time generator reads like upstream's own fragments. Their `native_*` bodies in
  `crates/horizon-client/src/game/native_hooks.rs` dispatch to the mods again.
- `hits` and `mod_machines` are declared in `crates/horizon-client/src/client_lib.rs` (upstream moved the lib root).
- A content mod entity's declared `loot_table` is honoured by `mobs::drops::death_loot_table` (after an NBT
  `DeathLootTable`, before `<ns>:entities/<path>`).
- Upstream added five helpers under names Weftizon already used, with other signatures; upstream's keep the names
  and Weftizon's are now `native_mob_view` (was `native_entity`), `native_hurt_mob` (was `native_hurt_entity`),
  `native_light_at` (was `native_light`), `native_explode_here` (was `native_explode`) and `native_world_boss_bar`
  (was `native_boss_bar`) (docs/native-mods.md, "Explosions and hunted players"). The user's InvasionV2 native mod
  (`~/Documents/Playground/MinecraftMod/mods-port/invasionv2/native`) was renamed to match and repinned.
- Where upstream had already ported the same thing (content-mod recipes on the server, TNT `block_state`/power
  in saves, content entity textures in the HUD atlas, unsupported-feature logging), upstream's version was kept.

Verified: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, the mod crates' tests and
`cargo test -p horizon-client -p horizon-client-lib --lib --bins --tests`.

Native mods: the launcher's `native-build` downloads source and horizon-mixins from `horizon-rust/*`, which is no
longer reachable. To check a bundle against this tree locally, use the horizon-mixins checkout at
`~/.hermes/cache/scratch/horizon-mixins` (`target/debug/horizon-mixins`, commit `07dab17`): make a
`git worktree add --detach <tmp>/ws HEAD`, delete its symlinks (`CLAUDE.md`; the tool refuses them), then
`horizon-mixins apply --workspace <tmp>/ws --mods <bundle>/mixins --out <tmp>/out` and
`cargo build --profile fast -p horizon-client` in `<tmp>/out`.

## Next sync

Fetch the newest upstream commit from wherever it is public (see Remotes), merge it on a branch
`upstream-snapshot-<date>`, revert any account-forcing change again, then: keep `native-hooks.json` upstream's and put Weftizon hooks
in `tools/native-hook-fragments/weftizon.json`; check every file Weftizon changed since the merge base still
carries its change (or upstream's equivalent); run the commands above.
