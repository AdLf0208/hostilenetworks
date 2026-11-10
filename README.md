# hostilenetworks (Horizon port)

A Horizon port of [hostilenetworks](Shadows-of-Fire/Hostile-Neural-Networks) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- The **Simulation Chamber → Prediction → Loot Fabricator** loop, with a Data
- `hostilenetworks:sim_chamber` — 4 slots (model, input, base drop, prediction),
- a 2,000,000 FE buffer, a menu type with 6 data slots, and the run rules below.
- `hostilenetworks:loot_fabricator` — 17 slots (prediction + 16 outputs), a
- ,000,000 FE buffer, a menu type with 5 data slots, and the palette that picks
- the drop row.
- `hostilenetworks:data_model` — the model (zombie) plus `data` and
- `iterations`, in three registered item components.

## Not yet ported

- Kill/break model upgrades (`data_gained`, the `living_death` hook, the
- inventory/Deep Learner scan) — the tier table's `data_gained` column was
- removed from `runtime.rs` because nothing in the slice uses it.
- Blank-model attunement (`rightClickToAttune`) and the Model Framework's use.
- The other 58 entity and 10 block models, their optional drops and the
- silk-touch `upgrade_conditions`.
- Deep Learner (curio, menu, HUD) and Fabrication Directive.
- Data Center multiblock + Queue mode + redstone modes.

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: Shadows-of-Fire/Hostile-Neural-Networks. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
