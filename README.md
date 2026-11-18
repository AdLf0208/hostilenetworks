# Hostile Neural Networks — a Horizon port

**Hostile Neural Networks** is a Minecraft mod. A Minecraft Forge mod about defeating hostile simulations to produce loot drops. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [Shadows-of-Fire/Hostile-Neural-Networks](https://github.com/Shadows-of-Fire/Hostile-Neural-Networks) — licence **MIT**.

| | |
|---|---|
| Upstream mod | Hostile Neural Networks ([source](https://github.com/Shadows-of-Fire/Hostile-Neural-Networks)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `hostilenetworks.hmod` (install it with the Horizon launcher) |
| Licence | **MIT** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- `hostilenetworks:sim_chamber` — 4 slots (model, input, base drop, prediction),
- `hostilenetworks:loot_fabricator` — 17 slots (prediction + 16 outputs), a
- `hostilenetworks:data_model` — the model (zombie) plus `data` and
- `hostilenetworks:prediction` — carries the model it came from.
- `hostilenetworks:prediction_matrix`, `hostilenetworks:blank_data_model`,
- One model's table: `hostilenetworks:zombie` (sim cost 128, matrix input,
- **The embedded-content workaround is gone.** 0.1.0 wrote its content `.hmod` to `.horizon-cache/`
- **Block dispatch is by entity type in the tick.** Observable only as: a Part cannot be accidentally

## Not yet ported

- Kill/break model upgrades (`data_gained`, the `living_death` hook, the
- Blank-model attunement (`rightClickToAttune`) and the Model Framework's use.
- The other 58 entity and 10 block models, their optional drops and the
- Deep Learner (curio, menu, HUD) and Fabrication Directive.
- Data Center multiblock + Queue mode + redstone modes.
- Training mode's model-data upgrades.
- The generalized predictions and the `living_matter` recipes.
- The chamber/fabricator GUI textures and the model-rendering item.

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `hostilenetworks.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Hostile Neural Networks was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **MIT**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/Shadows-of-Fire/Hostile-Neural-Networks

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run hostilenetworks` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/hostilenetworks/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
