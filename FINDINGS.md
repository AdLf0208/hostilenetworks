# Hostile Neural Networks 6.5.1 — findings (slice 1)

Read from `Shadows-of-Fire/Hostile-Neural-Networks`, branch `1.21`, commit
`787bced`, version 6.5.1 (`gradle.properties`), plus the mod's own data files in
the pack's jar. The repo was read for behaviour only; this port is a clean-room
Rust rewrite.

## The loop upstream implements

1. A **Data Model** item names one entity (or block) type and carries an int
   `data` count and an `iterations` count, in data components
   (`Hostile.Components`: `data_model`, `data`, `iterations`).
2. **Simulation Chamber** (`SimChamberTileEntity`): 4 slots — model, input,
   base-drop output, prediction output. With a model and the model's input item
   (the Prediction Matrix) it runs a simulation for
   `SimMode.getRuntime()` ticks, spending `model.simCost()` FE per tick from a
   buffer capped at `SimPowerCap` (2,000,000). On completion it adds one
   `base_drop` and `predictionSuccess` prediction items.
3. **Prediction** item: the same `data_model` component, so it names the model
   it came from.
4. **Loot Fabricator** (`LootFabTileEntity`): 17 slots — one prediction slot,
   16 outputs. It fabricates the **selected** row of that model's
   `fabricator_drops` for 60 ticks at `LootFabPowerCost` FE/t (256), consumes one
   prediction, and outputs the row's stack (count included).

## Model tiers (`data/hostilenetworks/model_tiers/*.json`)

| Tier | `required_data` | `data_gained` | `accuracy` | `can_sim` |
|---|---:|---:|---:|---|
| faulty | 0 | 1 | 0.00 | **false** |
| basic | 6 | 4 | 0.05 | true |
| advanced | 54 | 10 | 0.22 | true |
| superior | 354 | 18 | 0.65 | true |
| self_aware | 1254 | 0 | 1.00 | true |

`ModelTierRegistry.getByData` picks the highest tier whose `required_data` the
model's data reaches; `DataModelInstance.getAccuracy` interpolates towards the
next tier's accuracy when `Continuous Accuracy` is on (default): the value is
the tier's accuracy plus the next tier's lead scaled by the progress through
the tier. `rollPredictions`: each whole 100 % yields that many predictions, the
fraction is the chance of one more.

`data_gained` is what a kill (or a block break) adds to a model *item the player
holds or has equipped* — the Deep Learner curio — not to a chamber. It is out of
this slice (see gaps).

## The two machines' constants

| Constant | Value | Where |
|---|---:|---|
| `SimMode.BASE_RUNTIME` | 300 ticks | `SimChamberTileEntity.SimMode` |
| inference speed / cost | 1.25× / 1.0× | `SimMode.INFERENCE` → 240 ticks, base cost |
| training speed / cost | 1.0× / 1.2× | `SimMode.TRAINING` → 300 ticks, `ceil(cost × 1.2)` |
| `SimPowerCap` | 2,000,000 FE | `HostileConfig` |
| `LootFabPowerCap` | 1,000,000 FE | `HostileConfig` |
| `LootFabPowerCost` | 256 FE/t | `HostileConfig` |
| fabrication length | 60 ticks | `LootFabTileEntity.serverTick` (`runtime >= 60`) |
| `rightClickToAttune` | true | `HostileConfig` (blank model on a mob) |
| `simModelUpgrade` | 1 (on) | training mode upgrades the model's data |
| `actionUpgradesModel` | true | kills/blocks add data to a held model |

`FailureState`: none · output · energy · input · model · faulty ·
energy_mid_cycle · redstone (plus data-center ones).

`SimChamberTileEntity.serverTick` order, which the port mirrors:

1. no model → `MODEL`, runtime 0;
2. model's tier `can_sim == false` → `FAULTY`, runtime 0;
3. `runtime == 0` → `canStartSimulation()`: input matches (`INPUT`), redstone
   (`REDSTONE`), training allowed (`CANNOT_TRAIN`), outputs can take the drops
   (`OUTPUT`), energy for one tick (`ENERGY`); if all pass, `runtime = mode
   runtime`, roll predictions, consume one input;
4. otherwise, with energy for the tick: redstone mismatch → `REDSTONE`;
   `--runtime`; at 0 add the base drop and the predictions (inference) or the
   model's data (training), and `iterations += 1`; **the finishing tick spends no
   energy**;
5. no energy mid-run → `ENERGY_MID_CYCLE`, the runtime freezes.

The Loot Fabricator pauses (without resetting progress) on redstone, requires
the selected row and output room, and takes 256 FE per progress tick; the
completing tick takes none.

## Loot tables (zombie, `data_models/zombie.json`)

`sim_cost` 128, input `hostilenetworks:prediction_matrix`, base drop
`hostilenetworks:overworld_prediction`, `fabricator_drops`:
`minecraft:rotten_flesh ×64`, `minecraft:iron_ingot ×8`, `minecraft:carrot ×16`,
`minecraft:potato ×16`, and an **optional** `reliquary:zombie_heart ×2` (the
Reliquary mod is not in this pack's port list — left out, as upstream's
`optional` flag intends).

## Loot tables, all carried models (for the next slices)

The jar carries 58 entity models (vanilla + allthemodium, ars_nouveau,
artifacts, thermal, twilightforest) and 10 block models (ores), each with its own
sim cost, base drop and drop table. The tiers and the two machines are shared,
so a second model is a table row plus (for blocks) the block-model variant and
the silk-touch condition; the data-centre multiblock is a separate machine.

## Edge cases worth keeping

- A model item with an unbound/unknown model id is unusable: no simulation, no
  fabrication (upstream calls the holder "unbound").
- The prediction output only stacks with the *same model's* prediction (the
  component is part of `isSameItemSameComponents`).
- A model's `data` is capped by nothing but the top tier; `self_aware` gains no
  more data (`data_gained` 0).
- A faulty model (data < 6) never simulates: it is crafted/attuned and must be
  trained by kills first (or by training mode in a chamber).
- Breaking a machine drops its whole inventory (upstream `onRemove` →
  `Containers.dropContents`); the energy is lost.
- The chamber's `iterations` on the model item increments per completed run and
  has no gameplay effect.
- Continuous accuracy means a basic model is not a flat 5 %: at 20 data it is
  ~9.96 %.

## Next slices (planned gaps)

1. **Kill/break upgrades** (`HostileEvents`): a kill of the model's entity (or a
   variant) adds `tier.data_gained` data to the matching model in the player's
   inventory or Deep Learner; block models add data on a break. Needs a
   `living_death` hook and an inventory scan; the whole tier table is already in
   `runtime.rs` except `data_gained` (removed as unused — re-add with the slice).
2. **Blank-model attunement** (`rightClickToAttune`): a Model Framework clicked
   on a mob becomes that model's Data Model. Needs `interact_entity`/`use_item`
   and the per-entity model map (58 rows).
3. **Deep Learner** (curio that trains models you carry, its own menu, the HUD
   renderer) and the **Fabrication Directive** (copy a fabricator's selection).
4. **The other models**: all 58 entity + 10 block rows, their optional drops and
   the block model's silk-touch `upgrade_conditions`.
5. **Data Center** multiblock (7×7 shell, IO ports, 8,000,000 FE, 25 concurrent
   self-aware sims) + the fabricator's Queue mode and the redstone control
   options of both machines.
6. **Training mode's model-data upgrades** (the mode switch exists and resets the
   run, but the data write on completion is not implemented in slice 1).
7. **The generalized predictions** (overworld/nether/end/twilight items and the
   `living_matter` crafting recipes) — the items exist upstream and feed the
   conversion recipes; only the typed `hostilenetworks:prediction` is carried.
8. **The chamber/fabricator screens' textures** (`sim_chamber.png`,
   `loot_fabricator.png`) and the model-rendering item and tooltips.
