# Hostile Neural Networks for Horizon — slice 1

The **Simulation Chamber → Prediction → Loot Fabricator** loop of Hostile Neural
Networks 6.5.1 (MIT, `Shadows-of-Fire/Hostile-Neural-Networks`), rewritten in
Rust for Horizon. Clean-room rewrite: the upstream Java was read for behaviour
only.

One flat SDK package (mod API `0.2`): the carried blocks, items, models,
textures, lang, menus, recipes and loot tables are the `assets/` and `data/`
here, and the behaviour is the one `impl mod_api::Hooks` in `src/lib.rs`.

- `src/runtime.rs` — the pure rules (model tiers, the chamber's run, the
  fabricator's production), byte-identical to the retired `.hnmod` bundle's
  `runtime.rs`, with its 16 tests.
- `src/support.rs` — the glue: each machine's block entity, tick, screen, draw,
  click and menu button as one **Part**, its slots/FE buffer/menu as
  **declarations**, the item components, and the `/hostilenetworks` harness
  commands. 10 tests pin the declarations, the saved-int codecs and the readout
  lines.
- `assets/`, `data/` — the carried content (2 blocks, 5 items, 3 item
  components, 2 menus, 3 recipes, 2 loot tables, the pickaxe tag, the creative
  tab, lang), moved unchanged from the retired bundle's `content/`.

What this slice implements: a Data Model trains through simulated combat into
Predictions (2,000,000 FE buffer, 240-tick inference runs), and the Loot
Fabricator turns a Prediction into the model's real drops (60 ticks, 256 FE a
tick, 16 outputs, a palette on its own screen). The zombie model's table is
carried.

Not yet ported: kill-driven model data, blank-model attunement, the 68 other
models, the Deep Learner, the Data Center, training-mode data upgrades and a
faithful custom GUI background. See [`notes.md`](notes.md) for the full gap
list, the deviations and the helper→hook map.

**Local-only** until the maintainer approves publishing (ARCHITECTURE.md §8);
MIT allows derivatives, approval is still required.

- Notes, deviations, gaps and the migration record: [`notes.md`](notes.md)
- Upstream findings and the next slices: [`FINDINGS.md`](FINDINGS.md)
- Jar provenance and every carry patch:
  [`../_shared/carry-work/hostilenetworks/SOURCE.md`](../_shared/carry-work/hostilenetworks/SOURCE.md)
- Evidence (ladder, package checks, the 0.1.0 in-game runs):
  [`../_shared/evidence/hostilenetworks/`](../_shared/evidence/hostilenetworks/)

Pinned to SDK `0b14f8acfd97fa2c750e42795249dcc981178ca4` / mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0` for game 26.3. Compiles and packages
through the campaign's build slot:

```sh
ports/tools/build-slot.sh run --job hostilenetworks-sdk --port ports/hostilenetworks --step test -- \
    bash -c 'cd ports/hostilenetworks && cargo test --offline'
ports/tools/build-slot.sh run --job hostilenetworks-sdk --port ports/hostilenetworks --step build -- \
    bash -c 'cd ports/hostilenetworks && rm -f Cargo.lock && cargo horizon-mod build'
```

The in-game check needs a game the launcher built with this package
(`horizon-launcher native-build ports/hostilenetworks --instance <i>
--trust-native-code`); no launcher is installed in this environment, so the
0.2.0 in-game leg is not claimed.
