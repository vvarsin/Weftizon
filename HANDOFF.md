# Handoff

Read [AGENTS.md](AGENTS.md) first, then [ROADMAP.md](ROADMAP.md) (the fork's only roadmap: what is done and what
is next in the Weft port). This page only records the state an agent picks up.

## Remotes

- `origin`: `git@github.com:vvarsin/Weftizon.git`, the fork. `main` is pushed here.
- `upstream`: `https://github.com/horizon-rust/horizon.git`. As of 2026-11-26 it answers 404 (private or moved),
  so `git fetch upstream` fails; the last upstream commit merged is `fa7924b3fb` (2026-11-08,
  "Merge pull request #12832 ... perf/paletted-pack-uniform").

## Last upstream sync (2026-11-26)

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
- Where upstream had already ported the same thing (content-mod recipes on the server, TNT `block_state`/power
  in saves, content entity textures in the HUD atlas, unsupported-feature logging), upstream's version was kept.

Verified: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, the mod crates' tests and
`cargo test -p horizon-client -p horizon-client-lib --lib --bins --tests`.

## Next sync

`git fetch upstream && git merge upstream/main`, then: keep `native-hooks.json` upstream's and put Weftizon hooks
in `tools/native-hook-fragments/weftizon.json`; check every file Weftizon changed since the merge base still
carries its change (or upstream's equivalent); run the commands above.
