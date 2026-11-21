# Hostile Neural Networks → Horizon (slice 1: simulation → prediction → loot)

Ported from **Hostile Neural Networks 6.5.1** (`Shadows-of-Fire/Hostile-Neural-Networks`,
branch `1.21`, commit `787bced`, Minecraft 1.21.1 NeoForge), MIT, read for
behaviour only. The jar is the exact ATM10 file, fetched by the pack's own
SHA-1 (`5e5db7d3…31b4`); provenance and every carry patch are in
[`ports/_shared/carry-work/hostilenetworks/SOURCE.md`](../_shared/carry-work/hostilenetworks/SOURCE.md).
Upstream findings, the tier table and the next slices are in
[`FINDINGS.md`](FINDINGS.md).

**Format (2026-11-21):** one flat SDK package, `hostilenetworks.hmod` **0.2.3** (`api = "0.2"`), for
SDK **`0a875da`** / mixins `e59da90` (re-pinned from `0b14f8a` by the campaign repin). The `.hnmod`
container is retired; see [`## Migration to the SDK format`](#migration-to-the-sdk-format)
for the file map, the helper→hook table, the measured ladder and the traps.
Everything below the migration section that names `mixins/`, injections or
`tools/horizon.py` describes the **retired 0.1.0 bundle**, kept as history and
as evidence.

**Class + tier:** `content` · `machines` (ledger row `hostilenetworks`, source
map row 222). The approach from the pin's `docs/modpacks.md` §5 is the
**machines** shape: carried content (blocks, items, models, recipes, loot) plus
a native half with **block entities, item slots, menus/screens and FE in the
block entity's saved compound** — not a plain content block. Upstream's own
`saveAdditional`/`serverTick` is the model for the state machine.

**Licence:** MIT → **rank 0**. LOCAL-ONLY until the maintainer approves
publishing (ARCHITECTURE.md §8); MIT allows derivatives, approval is still
required. Ported now because it is a workable P0 machine core mod at the top
licence rank with no unported required parent (its `placebo` dependency is a
library this rewrite does not need).

## Slice 1 — what this delivers

The **Simulation Chamber → Prediction → Loot Fabricator** loop, with a Data
Model:

- `hostilenetworks:sim_chamber` — 4 slots (model, input, base drop, prediction),
  a 2,000,000 FE buffer, a menu type with 6 data slots, and the run rules below.
- `hostilenetworks:loot_fabricator` — 17 slots (prediction + 16 outputs), a
  1,000,000 FE buffer, a menu type with 5 data slots, and the palette that picks
  the drop row.
- `hostilenetworks:data_model` — the model (zombie) plus `data` and
  `iterations`, in three registered item components.
- `hostilenetworks:prediction` — carries the model it came from.
- `hostilenetworks:prediction_matrix`, `hostilenetworks:blank_data_model`,
  `hostilenetworks:overworld_prediction` — the model input, the framework and the
  chamber's base drop.
- One model's table: `hostilenetworks:zombie` (sim cost 128, matrix input,
  overworld prediction, four fabricator rows).

## Slice 2 — training mode upgrades the model (0.2.3, 2026-11-21)

**Gap 6** of the list below ("Training mode's model-data upgrades"): the mode switch already
existed and reset the run, but a completed training run still produced the inference loot and never
touched the model's data. 0.2.3 implements the other half of upstream's
`SimChamberTileEntity.serverTick` (read for behaviour, `227bced`→`787bced` era source):

- a finished **training** run adds **one** data to the model in slot 0 (`canUpgrade` →
  `currentModel.setData(getData() + 1)`, i.e. +1 per run, not the kill `data_gained`), adds **no**
  base drop and **no** prediction, and still counts one iteration;
- training is **refused** for a self-aware model (`ModelTier.isMax`) before the output/energy
  gates, with a new `FailureState.CANNOT_TRAIN` → `fail cannot_train`;
- the start gate order is upstream's: input → redstone → cannot-train → output → energy.

Unit tests (red first, `ports/_shared/evidence/hostilenetworks/sdk-test-red-training.log`):

| test | before the fix |
|---|---|
| `training_upgrades_the_model_and_produces_no_loot` | `left: Finish { base_drop: Some(overworld_prediction), predictions: 3, data_gained: 0, … }` |
| `a_self_aware_model_cannot_start_a_training_run` | `left: Start { runtime: 300, predictions: 1 } right: Idle(CannotTrain)` |
| `training_needs_its_input_before_the_tier_gate` | guard for the gate order (passes before and after) |

The runtime change also renamed the pure `Item` struct to **`ItemCount`**: `fits --build 0b14f8a`
scanned it as the SDK's `Item` and false-failed (`the type Item can no longer be built with a
literal`), the local-type collision recorded in HANDOVER §9d item 14. With the rename the same
`fits` run answers `0b14f8a  fits` (exit 0) and the pin run still answers `0a875da  made for`.

In game (`ports/_shared/evidence/hostilenetworks-e2e/e2e-launcher.log`): a basic zombie model at 20
data, the mode switched by the screen's own mode line, one Prediction Matrix and 1,000,000 FE →
after 300 ticks the readout is `… FE, runtime 0, predictions 0, mode training, fail input, tier
basic, data 21, slots [data_model x1]`, the base-drop and prediction slots are empty, and the
energy fell from 1,969,408 to 1,923,362 — exactly 299 × 154 FE (the finishing tick takes none).
`control 5` then puts a self-aware model (1254 data) into the same training chamber with an input:
`fail cannot_train`, `runtime 0`, the input is not consumed and the energy is unchanged. `NEGATIVE=1`
flips the data assertion and exits 1 (`training upgraded the model to 21, so the assertion is alive`).

### Coverage — the families the package ships

| family | status | evidence / phase |
|---|---|---|
| block | tested | both machines placed and read back (phases 1–5); a survival pickaxe break drops them (phase 9) |
| block_entity | tested | saved `energy`/`runtime`/`mode`/`fail`/`model` read back by `/hostilenetworks info`; the run and the fabrication are tick-driven |
| item | tested | five items given and read back, an unknown id refused (phase 8); the inventory's sixth entry, `data_model_base`, is a **model file**, not an item (see below) |
| loot_table | tested | survival pickaxe break: `sim_chamber 0→1`, `loot_fabricator 0→1`; the vanilla stone→cobblestone break is the differential |
| recipe:minecraft:crafting_shaped | tested | `tools/craft_check.py`: each of the three outputs' id + count, the stack in the inventory, ingredients consumed, plus comparator-alone and missing-clay controls |
| multiblock | not ported | the package ships none; the Data Center is gap 5 (7×7 shell, IO ports, 25-sim controller) |

### Coverage — other families (documented, not in the artefact inventory)

- **`item`'s sixth inventory entry, `hostilenetworks:data_model_base`**: **not an item** — a carried item
  model (`assets/hostilenetworks/models/item/data_model_base.json`), the framework texture the model
  item is drawn with; the `item` row above covers the five real items.
- **Item components** (`data_model`, `data`, `iterations`): **tested** — the typed model, data 20 → 21
  (training) and the iteration count survive chamber → player → fabricator.
- **Menu / screen** (4 and 17 slots): **tested** — `ctl screen` slot counts, the mode-line click and
  the fabricator palette click.
- **Tag `minecraft:mineable/pickaxe`**: **tested** — the pickaxe break drops both machines (membership
  itself is not readable through `ctl`).
- **Command family** (`info`, `energy`, `modelset`, `take`, `put`, `fab`): **tested** — every readout is
  parsed; an unknown machine position answers `No machine at that position.`
- **Creative tab**: **not ported** — carried data; `ctl` has no creative-tab read.
- **Lang / models / textures**: **tested (load only)** — the game loads them and all items render; the
  e2e asserts ids and states, not pixels.
- **Kill/break model data (gap 1)**: **not ported** — upstream upgrades models **inside a Deep Learner**
  only (`HostileEvents.forEachLearner`), so it needs gap 4's item container first.
- **Blank-model attunement (gap 2)**: **blocked(no-player-handle-on-entity-interact)** — the pin's
  `entity_interact_specific` / `item_interact_mob` events carry no player (`surface.json` arguments:
  entity, kind, stack, location), so the held stack cannot be rewritten.
- **The other 68 models (gap 3)**: **not ported** — one table row per model plus block-model variants.
- **Deep Learner + Fabrication Directive (gap 4)**: **not ported** — needs an item container, a menu
  and a HUD.
- **Queue mode, redstone modes (gap 5)**: **not ported** — a later slice.
- **Generalized predictions + `living_matter` (gap 7)**: **not ported** — the four prediction items and
  their conversion recipes.
- **GUI textures, model-rendering item (gap 8)**: **not ported** — client renderer and art; no renderer
  claim is made.

### Slice 2 ladder and artifact (0.2.3, pin `0a875da`)

| step | command | result |
|---|---|---|
| repin | `[native] horizon` → `0a875da…`, `.horizon/sdk` deleted so the tool refetches | `SDK 0a875da (build 0a875da91dbb0fdcb7ba8c42c11341aa229af51a) in ./.horizon/sdk` |
| tests | `cargo test --offline` (in a `build-slot.sh` lease, `BUILD_LOCAL=1`) | **31 passed, 0 failed** (`sdk-test.log`) |
| clippy | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, 0 warnings (`sdk-clippy.log`) |
| fmt | `cargo fmt --check` | `FMT_EXIT:0` (`sdk-fmt.log`) |
| build 1 | `rm -f Cargo.lock && cargo horizon-mod build` | 64,607 B, sha256 `8843ad11…c432` |
| build 2 | the same again | `CMP:identical`, no `Cargo.lock` packed |
| check | `cargo horizon-mod check hostilenetworks.hmod` | `CHECK_EXIT:0`; `hostilenetworks 0.2.3 (native-mod, format native-1/0a875da…/e59da90…, for 26.3)` (`sdk-check.log`) |
| fits, pin | `cargo horizon-mod fits … --build 0a875da` | `0a875da  made for`, `FITS_EXIT:0` (`sdk-fits.log`) |
| fits, other | `cargo horizon-mod fits … --build 0b14f8a` | `0b14f8a  fits`, `FITS_OTHER_EXIT:0` (`sdk-fits-other.log`) |
| gate | `python3 ports/tools/verify-sdk-package.py hostilenetworks` | **1/1** |
| in game | `launcher-game.sh build hostilenetworks` then one leased `tools/e2e.sh` | `Compiling hostilenetworks (1 of 1)`, `PASS` + `EXIT:0` |
| control | the same Nightly built **without** the package | `FAIL: the fabricator is minecraft:air` + `EXIT:1` |
| selftest | `SELFTEST=1` | `FAIL: expected 1 x hostilenetworks:overworld_prediction …` + `EXIT:1` |
| counterfactual | `NEGATIVE=1` | `FAIL: NEGATIVE: training upgraded the model to 21` + `EXIT:1` |
| logs | `python3 ports/tools/check-e2e-logs.py hostilenetworks-e2e --strict` | `logs=4  ok=4` |

Artifact: **64,607 B**, sha256 `8843ad1134f20f53917ad45fc873348b1cdf65cc4a260442ca09d65ee603c432`,
declared version **0.2.3**, **54 entries**, no `Cargo.lock`. Every `sdk-*.log` in
`ports/_shared/evidence/hostilenetworks/` prints the size and sha256 it ran against.

`src/runtime.rs` is no longer byte-identical to the retired bundle: the 0.2.1 review fixes changed
it first, and this slice adds `is_max`, the `CannotTrain` variant, the training finish rule and the
`Item` → `ItemCount` rename. The port `AGENTS.md`'s "runtime never changes" line describes the
migration, not later slices; `runtime.rs` stays the only home of gameplay rules and keeps its tests.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,verification-before-completion; BeautifulRust/rust-best-practices

## Behaviour, upstream vs this port

| Area | Upstream | This port |
|---|---|---|
| run length | `BASE_RUNTIME 300 / speed 1.25` = 240 ticks (inference) | `Mode::runtime` = `300 * 4 / 5` |
| energy per tick | `model.simCost()` (inference), `ceil(cost × 1.2)` (training) | `Mode::cost`, same rounding |
| finishing tick | the `--runtime == 0` branch returns before spending | `ChamberAction::Finish` carries no energy cost |
| start gate | input → redstone → training allowed → output room → energy | same order (`FailState` names the first failure) |
| faulty model | `can_sim == false` → `FAULTY`, runtime 0 | `Tier::Faulty`, same |
| prediction roll | `floor(accuracy) + (rand < frac)` | `roll_predictions(accuracy, sample)`; glued to `world.random` |
| accuracy | tier accuracy + lead × progress (continuous) | `accuracy_at`, same formula |
| prediction stacking | `isSameItemSameComponents` | the slot check compares the item **and** the model component |
| tier cut-offs | 0/6/54/354/1254 | `Tier::required_data` + `tier_for` |
| chamber slots | model, input, base out, prediction out (0..3) | same; container `inputs [0,1]`, `outputs [2,3]` |
| fabricator | 60 ticks × 256 FE, consume one prediction, 16 outputs | `FAB_RUNTIME`/`FAB_POWER_COST`, same |
| fabricator selection | per-model map, Fixed and Queue modes | one `selection` + `selModel` pair (Fixed only) |
| break | drops the whole inventory | the engine's mod-container removal drops `items`; energy is lost (as upstream) |
| redstone | three modes, default `IGNORED` | always `IGNORED` (gap 5: the mode buttons are not wired) |
| training mode | `+1 data` per finished run, no loot, refused at the max tier | same since 0.2.3 (`FailState::CannotTrain`) |
| `data_gained` (kills) | model data grows on kills/blocks | **not in this slice** (gap 1; needs the Deep Learner) |

The FE buffer is the pin's `native_block_entity_energy` (API 142): the amount is
the block entity's saved int `energy`, so it survives a save and the machine's
own tick reads and spends it. Nothing else on this pack inserts FE into it (no
energy network), which is why the harness command below exists.

## Hooks (0.1.0 bundle: 12 hooks — 13 injections with the embedded-content HEAD — all stamped at that pin)

| Hook (API) | Helper | What it does |
|---|---|---|
| `component_types` (65) | `component_types.rs` | Register `hostilenetworks:data_model`, `:data`, `:iterations`. |
| `block_entity` (15) | `block_entity.rs` | Give both machines a block entity. |
| `block_entity_ticks` (15) | `block_entity_ticks.rs` | They tick. |
| `block_entity_container` (16/17) | `block_entity_container.rs` | 4 / 17 slots, faces, inputs, outputs, menu type. |
| `block_entity_energy` (142) | `block_entity_energy.rs` | 2,000,000 / 1,000,000 FE, fill-only, all faces. |
| `block_entity_tick` (17) | `block_entity_tick.rs` | The run and the fabrication. |
| `screen` (19) | `screen.rs` | Slot layouts (176×166 / 176×186). |
| `screen_draw` (19) | `screen_draw.rs` | FE, progress, mode, failure, the drop palette. |
| `screen_click` (20) | `screen_click.rs` | Palette row → menu button; chamber mode line → button 100. |
| `menu_button` (20) | `menu_button.rs` | Applies the selection / the mode switch on the server. |
| `register_commands` (8) | `register_commands.rs` | `/hostilenetworks …` (harness). |
| `perform_command` (8) | `perform_command.rs` | Its bodies. |

No `destroy_block`/`loot_drops` hook is needed: the pin drops a mod
container's slots on removal (`spill::remove_entity`, `PreRemove::Contents`),
which is what upstream's `onRemove` does. No `state_for_placement` either: the
barrel template places the block facing the placer, as upstream's
`getHorizontalDirection().getOpposite()` does.

## Harness additions (recorded, not gameplay)

A headless scenario cannot work a screen's slots, type a component by hand or
charge a battery, so the mod adds one command family. All of it is op level 2
except `model give`/`model held` (any player) and is documented here as a test
surface:

| Command | Why |
|---|---|
| `/hostilenetworks info <x> <y> <z>` | Prints the machine's FE, runtime, predictions, mode, failure, model, tier, data and slots (and the fabricator's selection). |
| `/hostilenetworks energy <x> <y> <z> <amount>` | Adds FE (the only way to charge a machine with no energy network). |
| `/hostilenetworks model give <model> <data>` | Gives a typed Data Model (the slice has no attunement yet). |
| `/hostilenetworks model held <model> <data>` | Retypes the held Data Model. |
| `/hostilenetworks model set <x> <y> <z> <slot> <model> <data>` | Types a model already in a machine slot. |
| `/hostilenetworks fab <x> <y> <z> <drop>` | Sets the fabricator's selection without the palette click. |
| `/hostilenetworks take <x> <y> <z> <slot>` / `put …` | Moves a stack between a machine slot and the player (a menu drag without the click model). |

## Deliberate deviations

- The **Data Model's item model is the framework texture**, not a rendered mob
  (`builtin/entity` + a Java `BlockEntityWithoutLevelRenderer` upstream); the
  item's name is the plain `item.hostilenetworks.data_model` text (no `%s Data
  Model` argument, since the pin's item names are not parameterised).
- Model tables are **built in** (`runtime::MODELS`), not data-pack files: the
  pin has no registry for a mod's own data type. One model (zombie) is carried.
- The fabricator keeps **one selection** (`selection` + `selModel`) instead of
  upstream's per-model map; Queue mode and the Fixed/Queue button are gaps.
- The screen text is drawn as plain GUI text over the default panel; the
  carried GUI textures (`sim_chamber.png`, `loot_fabricator.png`) are **not**
  used (no `own_background` painter yet).
- The palette's hit test offers **four rows** (the carried model's table
  length) so a click works even if the menu's data slots have not reached the
  client; the labels still come from the data slots and the server rejects a row
  the model does not have.
- Redstone control is `IGNORED` only; training mode is implemented (0.2.3: +1 data per run, no
  loot, `cannot_train` at the top tier) with its 300-tick / 1.2× cost.
- The `occludes` block field the pin's docs describe is a *pending* schema field
  at this commit (`cargo horizon-mod build` rejects it), so the machines use
  the barrel template's default occlusion.

## Gaps (next slices)

> **0.2.3 adds the measured blockers to this list.** Gap 1 cannot be done alone: upstream's
> `HostileEvents.forEachLearner` upgrades models **inside a Deep Learner**, so it is gap 1 + gap 4.
> Gap 2 is `blocked(no-player-handle-on-entity-interact)`: the pin's `entity_interact_specific`
> and `item_interact_mob` events name the entity, the kind and the held stack but **no player**
> (`ports/_shared/sdk/0a875da/surface.json`), so a mod cannot rewrite the held stack.

1. Kill/break model upgrades (`data_gained`, the `living_death`/`mob_killed` hook, the
   **Deep Learner** scan) — needs gap 4's item container first; the killed/blocks model map is
   the same loop as gap 3.
2. Blank-model attunement (`rightClickToAttune`) and the Model Framework's use —
   `blocked(no-player-handle-on-entity-interact)` at `0a875da` (see the note above).
3. The other 58 entity and 10 block models, their optional drops and the
   silk-touch `upgrade_conditions`.
4. Deep Learner (curio, menu, HUD) and Fabrication Directive.
5. Data Center multiblock + Queue mode + redstone modes.
6. ~~Training mode's model-data upgrades~~ — **done in 0.2.3** (see Slice 2 above).
7. The generalized predictions and the `living_matter` recipes.
8. The chamber/fabricator GUI textures and the model-rendering item.

## Review fixes (2026-11-18)

The reviewer's source review of PR #1254 (`var/review-all/1254-hostilenetworks.md`) left two
findings; both are fixed in **0.2.1** (`63643 B`, sha256 `c76c0996af962e3d702c40760855aba385e82f954a4aec0e3339377a8f295522`).

- **`src/support.rs:1123` — the level-0 `info` command read machine contents at arbitrary loaded
  coordinates.** The readout is gated on the ordinary interaction rules: an operator reads any
  machine, a player only one within `runtime::READ_REACH` (6 blocks), any other non-operator source
  nothing. Test: `runtime::tests::only_an_operator_or_a_nearby_player_reads_a_machine`.
- **`src/support.rs:1350` — `take` cleared a slot before confirming a recipient, and the slot
  helpers accepted foreign mod containers.** `take` refuses without a recipient and without an own
  slot (`runtime::take_allowed`), `machine_slot`/`write_machine_slot` first prove the block is one of
  this mod's machines and that the slot index is in that machine's slot count. Test:
  `runtime::tests::a_slot_command_needs_a_recipient_and_an_own_slot`.

The in-game leg also exposed two command-surface defects (fixed here, not reviewer items): the
`model give|held|set` usages shared a `model` prefix that the dispatcher collapsed, and a `word`
argument could not carry the namespaced id (`hostilenetworks:zombie`). They are now
`modelgive|modelheld|modelset`, with the model as a trailing `text` argument.

**In-game leg:** `ports/tools/launcher-game.sh run hostilenetworks` → `PASS` + `E2E EXIT:0`, control
`FAIL: the fabricator is minecraft:air (did the package load?)` + `EXIT:1`, recorded in
`ports/_shared/evidence/hostilenetworks-e2e/`.

## Verification (0.1.0, retired `.hnmod` format — historical)

## Embedded content — one `.hnmod` deliverable (2026-11-10)

The content half now travels **inside** the native bundle (the AE2 pattern, proved
on `charginggadgets`): the built `content/hostilenetworks.hmod` is carried at
`mixins/hostilenetworks/data/hostilenetworks-content.hmod`; `embedded_content.rs` writes it to
`.horizon-cache/` at start and appends it to the dev list from one `content_root`
HEAD injection (`cancellable = true`; bindings `root`, `enabled`, `dev`). One
installed `.hnmod` installs both halves, so the split content+code blocker is
gone. `tools/package.py` packs `data/` and refuses a copy that differs from
`content/hostilenetworks.hmod`.

Verification (2026-11-10, pin `1d63e0a`, Minecraft 26.3):
- **ladder**: `cargo fmt --check`, `clippy --all-targets -- -D warnings` (0
  warnings), `cargo test` **16 passed / 0 failed**, `horizon.py stage / check / test /
  build` all `cargo_exit: 0` (embedded tests included).
- **artifacts**: `content/hostilenetworks.hmod` **28,237 B**, SHA-256 `4d89e61796557ac73aa29afaadf01072469238cfde5278e8625fa20a5379e737`,
  **46** entries; `hostilenetworks-0.1.0.hnmod` **37,986 B**, SHA-256 `96efe6ac6011e5ccc7cc84f63e3246e5a9b179be0fad1e9508cf2f55162eff80`, **18**
  entries (rebuilt locally from these sources with `tools/package.py`; a second
  run is byte-identical and `horizon-mod check --kind native-mod` passes);
  `mixins/hostilenetworks/data/hostilenetworks-content.hmod` is byte-identical to the built `.hmod`.
- **e2e with no `--dev-mod`** (`EMBEDDED=1`): `PASS` + `EXIT:0`; the run printed
  `embedded content: 28237 B, identical to content/hostilenetworks.hmod`.
- **control** (`e2e-control.log`, `.horizon-cache` made unusable): the sample id
  is unknown, `PASS` + `EXIT:0` - the content really comes from the bundle.
- **fail leg** (`NEGATIVE=1`): `FAIL` + `EXIT:1`.
- `check-e2e-logs.py hostilenetworks --strict` green (all logs end `EXIT:<n>`);
  `upstream_ready.py hostilenetworks` -> `READY`, exit 0.

The compile ladder ran on the second build machine; the in-game legs ran locally.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

Layout of the evidence: every log is in
`ports/_shared/evidence/hostilenetworks/`.

### Runtime (TDD, red then green)

| Step | Command | Result |
|---|---|---|
| red | `cargo test --locked --offline` with the five decision functions stubbed | **FAILED, 3 passed / 13 failed, exit 101** (`runtime-tests-red.log`, `not implemented: chamber_action` …) |
| green | the same tests against the implementation | **16 passed, 0 failed, exit 0** (`runtime-tests-green.log`) |
| fmt | `cargo fmt --all --check` | exit 0 (`fmt.log`) |
| clippy | `cargo clippy --all-targets --locked --offline -- -D warnings` | exit 0, **0 warnings** (`runtime-clippy.log`) |

The 16 tests pin: the tier cut-offs, only-faulty-cannot-sim, continuous accuracy
(20 data → 9.96 %), the prediction roll's fraction, the two modes' runtimes and
costs, every start gate, the mid-run drain and the finishing tick's free tick,
the pause states, the fabricator's 60/256 rule, its selection validity, output
room, and the zombie row's values.

### Ladder at the campaign pin `1d63e0a` (final sources)

| Step | Command | Result |
|---|---|---|
| stage | `horizon.py stage --horizon …/pins/horizon-1d63e0a` (`HORIZON_MIXINS=…/horizon-mixins-5a64af9`) | `stage_changed: true`, 12 injections, 2 modules |
| check | `horizon.py check` | **`cargo_exit: 0`**, `ladder-check.log` has **0 warnings** |
| embedded tests | `horizon.py test` (`--lib game::hostilenetworks::tests`) | **16 passed, 0 failed, 1240 filtered out** (`ladder-test.log`) |
| fast build | `horizon.py build` | **`cargo_exit: 0`**, 383.5 s, 0 warnings (`ladder-build.log`) |

### In-game e2e (`tools/e2e.sh`, one slot invocation with a rebuild)

`PASS`, **`EXIT:0`** (`e2e.log`). Quoted from the passing run:

```
== control 1: no model in the chamber ==
readout: Simulation Chamber: 0 FE, runtime 0, predictions 0, mode inference, fail model, model -, tier faulty, data 0, slots []
== put a zombie Data Model into slot 0 and type it ==
Slot 0: Zombie Data Model, 20 data
readout: Simulation Chamber: 0 FE, runtime 0, predictions 0, mode inference, fail input, model hostilenetworks:zombie, tier basic, data 20, slots [hostilenetworks:data_model x1]
== control 2: a model and an input, but no energy ==
readout: Simulation Chamber: 0 FE, runtime 0, predictions 0, mode inference, fail energy, model hostilenetworks:zombie, tier basic, data 20, slots [hostilenetworks:data_model x1, hostilenetworks:prediction_matrix x1]
== a self-aware model (accuracy 1.0) so the run always yields a prediction ==
readout: Simulation Chamber: 0 FE, runtime 0, predictions 0, mode inference, fail energy, model hostilenetworks:zombie, tier self_aware, data 1254, slots [hostilenetworks:data_model x1, hostilenetworks:prediction_matrix x1]
== charge the chamber and run one simulation ==
Energy: 2000000 FE
mid-run: Simulation Chamber: 1991936 FE, runtime 177, predictions 1, mode inference, fail none, model hostilenetworks:zombie, tier self_aware, data 1254, slots [hostilenetworks:data_model x1]
after the run: Simulation Chamber: 1969408 FE, runtime 0, predictions 1, mode inference, fail input, model hostilenetworks:zombie, tier self_aware, data 1254, slots [hostilenetworks:data_model x1, hostilenetworks:overworld_prediction x1, hostilenetworks:prediction x1]
  has(16,101,0) hostilenetworks:overworld_prediction x1: ok
  has(16,101,0) hostilenetworks:prediction x1: ok
  has(16,101,0) hostilenetworks:prediction_matrix x0: ok
== the chamber opens its 4-slot menu ==
menu slots: 4
after the mode click: training
after the second mode click: inference
== carry the prediction to the Loot Fabricator ==
  has(16,101,4) hostilenetworks:prediction x1: ok
== control 3: a charged fabricator with no selection produces nothing ==
readout: Loot Fabricator: 1000000 FE, runtime 0, selection -1, fail input, slots [hostilenetworks:prediction x1]
  has(16,101,4) minecraft:rotten_flesh x0: ok
== click palette row 2 (iron ingot x8) on the fabricator's own screen ==
fabricator menu slots: 17
prediction components: {"hostilenetworks:data_model":"hostilenetworks:zombie"}
gui size: [427.0,240.0]
slot-300 bounds: [187.0,47.0,16.0,16.0]
== each palette row selects its drop ==
  click at 135,98 -> 0
  click at 135,108 -> 1
  click at 135,118 -> 2
  click at 135,128 -> 3
== and row 2 selects iron ingot x8 ==
  click at 135,108 -> 1
== fabricate one iron ingot drop ==
after fabricating: Loot Fabricator: 972096 FE, runtime 0, selection -1, fail model, slots [minecraft:iron_ingot x8]
  has(16,101,0) minecraft:iron_ingot x8: ok
  has(16,101,0) hostilenetworks:prediction x0: ok
== control 4: a wrong item (a Prediction Matrix) in the prediction slot ==
readout: Loot Fabricator: 972096 FE, runtime 0, selection -1, fail model, slots [hostilenetworks:prediction_matrix x1, minecraft:iron_ingot x8]
  has(16,101,0) minecraft:rotten_flesh x0: ok
  has(16,101,0) hostilenetworks:prediction_matrix x1: ok
PASS: the chamber ran a zombie simulation with FE (no model / no energy controls held), produced a base drop and a typed prediction, and the fabricator turned that prediction into iron ingot x8 through its own palette click (no-selection and wrong-item controls held); a plain barrel is not a machine
```

What that transcript proves, and how each claim could fail:

- **FE cost is exactly 239 × 128**: `2,000,000 − 239 × 128 = 1,969,408`, and the
  mid-run energy equals `2,000,000 − (240 − runtime) × 128` — an arithmetic
  identity against the reported runtime, not a loose bound.
- **The four controls hold**: no model (`fail model`), a model with no energy
  (`fail energy`, nothing produced), a charged fabricator with no selection
  (`selection -1`, `runtime 0`, no output), and a wrong item in the prediction
  slot (a Prediction Matrix: `fail model`, nothing produced, the matrix kept).
- **Menus**: 4 and 17 slots at the mod screen; the prediction's components show
  `hostilenetworks:data_model: hostilenetworks:zombie` on the client, so the
  item kept its model across the chamber → player → fabricator move.
- **The palette is the machine's own code**: a synthetic press at four panel
  rows selects 0, 1, 2, 3 in turn, and row 2 then fabricates iron ingot ×8 and
  consumes the prediction.

Controls that make the scenario fail-capable:

| Run | Command | Result |
|---|---|---|
| negative | `NEGATIVE=1 tools/e2e.sh` | **exit 1**: `FAIL: expected 1 x hostilenetworks:overworld_prediction …; got 0` (the chamber really did not produce loot) — `negative.log` |
| counterfactual | `COUNTERFACTUAL=1 tools/e2e.sh` | **PASS, exit 0**: the same setup on a plain barrel answers `No machine at that position.` (`counterfactual.log`) |

### Packages

| Artifact | Size | SHA-256 | Entries |
|---|---:|---|---:|
| `content/hostilenetworks.hmod` | **28,237 B** | `4d89e61796557ac73aa29afaadf01072469238cfde5278e8625fa20a5379e737` | 46 |
| `hostilenetworks-0.1.0.hnmod` | 37,986 B | `96efe6ac6011e5ccc7cc84f63e3246e5a9b179be0fad1e9508cf2f55162eff80` | 18 |

`.hmod` load check (`tools/hmod-load.sh`, shared unmodded game for the pin,
`--dev-mod`): **PASS** — both machines register and all three items exist
(`hmod-load.log`). Negative control, the same game **without** `--dev-mod`:
`Unknown block type 'hostilenetworks:sim_chamber'` and
`unknown item hostilenetworks:data_model`, so the check is not vacuous.

### Mode B + AE2 embedding (2026-11-10) — one listable artifact, re-verified

Pin unchanged (`1d63e0a5…` / `5a64af93…`, schema 6, `game.rs`
`64813cf4…f80f`). The session rebuilt the port from its current sources and
applied the **AE2 embedding pattern** so one `.hnmod` installs the content and
the code (the registry keeps one file per project): `mixins/hostilenetworks/data/hostilenetworks-content.hmod`
is the built `content/hostilenetworks.hmod` byte for byte, `mixins/hostilenetworks/embedded_content.rs`
compiles it in with `native_data!`, and the `embedded-content` `[[injection]]`
(`target = "horizon_client::game::mods::content_root"`, `kind = "HEAD"`,
`cancellable = true`, bindings `root`/`enabled`/`dev`) writes it to the game's
`.horizon-cache` and appends it to the dev-mod list. `tools/package.py` refuses
an embedded copy that differs from the built hmod.

All compile/e2e steps ran through `ports/tools/build-slot.sh` with
`CARGO_INCREMENTAL=0`; the ladder is the shared `tools/ladder-embed.sh`
(hmod build → embed → hnmod → fmt → clippy → test → stage/check/test/build),
evidence `ports/_shared/evidence/hostilenetworks/ladder-*`.

| Step | Result |
|---|---|
| `cargo fmt --all --check` / clippy `-D warnings` | exit 0, **0 warnings** |
| `cargo test --locked --offline` | **16 passed**, 0 failed |
| `horizon.py stage/check/test/build` | `cargo_exit: 0` in every step |
| `content/hostilenetworks.hmod` | **28,237 B**, `4d89e617…e737`, **46 entries**, "no code: content only" |
| `hostilenetworks.hnmod` (single artifact, carries the content) | **37,986 B**, `96efe6ac…ff80`, **18 entries** |
| e2e (with `EMBEDDED=1`, the shipped shape) | default **PASS + `EXIT:0`** / counterfactual **PASS + `EXIT:0`** / negative **FAIL + `EXIT:1`** |
| `check-e2e-logs.py hostilenetworks --strict` | green (`logs=… ok=…`, exit 0) |
| `upstream_ready.py --copyleft hostilenetworks` | **READY**, exit 0 |

Fail-capability: the selftest/negative legs flip a real assertion and exit
non-zero; the embedded shape is the one the artifact ships (no `--dev-mod`), and
the e2e checks the bundle wrote its content out (or the loader reports
`content from 1 mod(s)` when the game consumes the file).

## Review repairs (PR #604)

1. Staging is the orchestrator's (horizon-mods#686).
2. *"The upstream link in the entry is a relative `Shadows-of-Fire/Hostile-Neural-Networks`"*
   — the entry links the absolute GitHub URL and credits Shadows_of_Fire.
3. *"`hostilenetworks model give` and `model held` use unrestricted
   `NativeCommand::new`, whereas other mutating commands use `NativeCommand::op(…, 2)`"*
   — both are now operator-only (level 2); only `info` is open to players.
4. *"`loot_fabricator.json` declares only two data slots, but `fabricator_tick`
   emits five and `on_screen_draw` reads model index at 2 and energy at 3/4"* —
   the menu declares **five** data slots.
5. *"Neither outer bundle nor embedded content includes upstream MIT
   license/copyright notices"* — `mixins/hostilenetworks/LICENSE` and
   `NOTICE.md` (with the upstream copyright) travel inside the `.hnmod`, and
   both sit at the embedded content archive's root.

### Verification after the repairs

- Artifact: `hostilenetworks-0.1.0.hnmod`, 40,808 B, SHA-256 `f5e2f1b49062f87d40e0ff57e0ac03421b9772e588c7ce7fe1b9fb74b2cd431e` (rebuilt twice, identical bytes; `tools/package.py`; the embedded content equals `content/hostilenetworks.hmod`).
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --locked --offline`: clean, 16 passed; the embedded run (`python3 tools/horizon.py test --horizon ports/_shared/pins/horizon-1d63e0a`) is 16 passed, 0 failed (`target/horizon-test.log`).
- `build-slot.sh e2e --job … --port ports/hostilenetworks -- tools/e2e.sh`: PASS, log `ports/_shared/evidence/hostilenetworks/e2e-repair.log` ends `EXIT:0` (`check-e2e-logs.py hostilenetworks --strict` exit 0).
- `python3 ports/tools/upstream_ready.py hostilenetworks` and `check-skills.py --notes ports/hostilenetworks`: exit 0.

## Migration to the SDK format

*Version **0.2.0**, first publication. Migrated 2026-11-18 from the retired `.hnmod` container
(Horizon's mod-API merge #16941, launcher ≥ 0.2.70; PR #17992 cleans up the old files) to the flat
SDK package at nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** / mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, `package_format` 6, `content_api` "0.2", for game
26.3. Recipe followed: `ports/imfast/notes.md` → "Migration to the SDK format" (the original
12-step recipe), with `ports/enderstorage/notes.md` (containers, screens, block entities) and
`ports/naturesaura/notes.md` (Parts, `Registrar` declarations) as the second and third references.*

**One flat `hostilenetworks.hmod` now carries the code *and* the content.** `src/runtime.rs` is
**byte-identical** to the retired file (sha256 `4f00bd874d5fcd1ddaf50fa0e83a22a66111c4e65510beb7e8cf6625cc3fa723`,
23,819 B) — `cmp` against the retired copy and `unzip -p hostilenetworks.hmod src/runtime.rs | sha256sum`
against the same file. Only the glue, the manifest and the packaging are new.

### What moved, and where the retired tree is

The whole retired layout is kept as evidence in
`ports/_shared/evidence/hostilenetworks/retired-hnmod-0.1.0/` and was **moved before the first
build** (`cargo horizon-mod build` refuses a root holding retired package paths).

| 0.1.0 path | Now | Evidence copy |
|---|---|---|
| `mixins/hostilenetworks/runtime.rs` | `src/runtime.rs`, copied with `cp`, `cmp`-checked | `retired-hnmod-0.1.0/mixins/hostilenetworks/runtime.rs` |
| `mixins/hostilenetworks/support.rs` (1,079 lines) | `src/support.rs` (the glue: Parts, declarations, commands) + `src/lib.rs` (the one `Hooks`) | `retired-hnmod-0.1.0/mixins/hostilenetworks/support.rs` |
| `mixins/hostilenetworks/horizon-mixin.toml` (2 `[[module]]`, 13 `[[injection]]`) | no plan: typed `Hooks`/Part methods and `Registrar` declarations | `retired-hnmod-0.1.0/mixins/` |
| 13 per-hook `.rs` helpers | methods of the Parts, the two `Hooks` and `Registrar` calls; no per-hook files | `retired-hnmod-0.1.0/mixins/hostilenetworks/*.rs` |
| `mixins/hostilenetworks/data/hostilenetworks-content.hmod` + `embedded_content.rs` (AE2 pattern) | **gone**: the package carries `assets/` and `data/` itself | `retired-hnmod-0.1.0/mixins/` |
| `content/` (`horizon-mod.toml`, `assets/`, `data/`, its `hostilenetworks.hmod`, `target/`) | package root: `assets/`, `data/`; the old content manifest merged into the root one | `retired-hnmod-0.1.0/content/` |
| `hostilenetworks-0.1.0.hnmod` (40,808 B) | `hostilenetworks.hmod` (62,111 B, native + content) | `retired-hnmod-0.1.0/hostilenetworks-0.1.0.hnmod` |
| `compatibility.json`, `rust-toolchain.toml` | `[native]` in `horizon-mod.toml`; the pinned commit and the launcher decide the toolchain | `retired-hnmod-0.1.0/` |
| `tools/horizon.py`, `package.py`, `test_stage.py`, `test_template.py`, `add-notices.py`, `audit/`, `hmod-load.sh`, `ladder-embed.sh`, `__pycache__`, `.github/workflows/ci.yml` | deleted: staging, `.hnmod` packaging and the template CI belong to the retired format | `retired-hnmod-0.1.0/tools/`, `retired-hnmod-0.1.0/.github/` |
| `AGENTS.md` (the retired format's repo instructions), `VALIDATION.md` (0.1.0 ladder) | rewritten for the SDK package; the old files are evidence | `retired-hnmod-0.1.0/` |
| `Cargo.lock` | deleted, never packed (it changes the artifact) | not kept |

**Kept unchanged:** `tools/e2e.sh` (rewritten only where the launch has to name a launcher-built
game — its scenario and assertions are format-independent, see the gaps), `notes.md`,
`FINDINGS.md`, `LICENSE`, `LICENSE-NOTICE.md`.

### Carried content: 45 files, unchanged

`content/assets/` → `assets/` and `content/data/` → `data/`, copied with `cp -a`; every file was
diffed against the retired copy (sha256 of all 45: **0 differences**, same path set). 29 files under
`assets/hostilenetworks/` (2 blockstates, 6 item definitions, 6 models, 11 textures incl. 2
`.mcmeta`, lang), 16 under `data/` (2 blocks, 5 items, 2 menus, 1 creative tab, 3 recipes, 2 loot
tables, the `minecraft:mineable/pickaxe` tag). The old `content/horizon-mod.toml` (id, name,
version, api, side, capabilities) merged into the root manifest.

### File map (0.2.0)

| Path | What it is |
|---|---|
| `horizon-mod.toml` | the only manifest and identity: `id = "hostilenetworks"`, `version = "0.2.0"`, `api = "0.2"`, `side = "both"`, `capabilities = ["registries"]`, `[native]` pinned to `0b14f8a`/`e59da90` |
| `Cargo.toml` | one crate, `edition = "2024"`; its only dependency is `horizon-mod-api` from `.horizon/sdk/` |
| `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks`: `register`, `component_types`, `register_commands`, `run_mod_command` |
| `src/support.rs` | the glue: two `BlockEntityPart`s, four declarations, item components, screens, the eight command bodies. 10 tests |
| `src/runtime.rs` | the pure rules and their 16 tests, **byte-identical** |
| `assets/`, `data/` | the carried content (45 files, unchanged) |
| `LICENSE`, `LICENSE-NOTICE.md`, `README.md` | inside the package |
| `tools/e2e.sh` | the in-game scenario (needs a launcher-built game) |
| `hostilenetworks.hmod` | the deliverable |

### Helper → hook table

Each former injection is now a typed method, a Part method or a declaration; the helper file is gone.

| 0.1.0 injection (hook, API) | 0.2.0 |
|---|---|
| `embedded-content` (`content_root` HEAD, AE2 pattern) | **none** — one `.hmod` carries code and content |
| `component-types` (`component_types` 65) | `Hooks::component_types` → `support::component_ids()` (3 ids) |
| `block-entity` (`block_entity` 15, returned the initial compound) | `BlockEntityPart::block_entity` on both Parts → `Some(Compound::new())` |
| `block-entity-ticks` (15) | `BlockEntityPart::block_entity_ticks` → `event.ticks = true` |
| `block-entity-container` (16/17, built a `ModContainer`) | `Registrar::block_container` with `chamber_container()` / `fabricator_container()` |
| `block-entity-energy` (142, built a `ModEnergyBuffer`) | `Registrar::block_energy` with `chamber_battery()` / `fabricator_battery()` |
| `block-entity-tick` (17, `game`/`pos`/`entity`) | `BlockEntityPart::block_entity_tick` on `ChamberPart` / `FabricatorPart` (dispatch is by block entity type now, so each Part owns one machine and no `is_chamber` test is needed) |
| `screen` (19) | `BlockEntityPart::screen` → `Some(chamber_screen())` / `Some(fabricator_screen())` |
| `screen-draw` (19) | `BlockEntityPart::screen_draw` → `event.draw.text(…)` from `event.data` |
| `screen-click` (20) | `BlockEntityPart::screen_click` → `Option<i32>` menu button |
| `menu-button` (20) | `BlockEntityPart::menu_button` → `event.context.id`, `event.entity` |
| `register-commands` (8) | `Hooks::register_commands` → `support::declare_commands` (same 8 usages) |
| `perform-command` (8, string words) | `Hooks::run_mod_command` with typed arguments: `<pos:block_pos>` → `BlockArgumentValue::BlockPos([i32;3])`, `<amount:integer>`/`<slot:integer>`/`<drop:integer>`/`<data:integer>` → `Integer`, `<model:word>` → `String`; `Flow::Cancel` = handled |
| `[[module]] runtime` | `pub mod runtime;` — an ordinary Rust module |
| `[[module]] state` (support) | `pub mod support;` |

**Readout text is byte-identical to 0.1.0** (`Simulation Chamber: … FE, runtime …, predictions …,
mode …, fail …, model …, tier …, data …, slots […]` / `Loot Fabricator: … FE, runtime …,
selection …, fail …, slots […]`), so the existing in-game assertions still match; the glue tests pin
both exact lines.

What changed in the *plumbing* (not the rules): `game.world.random.next_float()` →
`LevelRef::with_random(host, |r| r.next_float())` on the level in place; `game.native_block_entity_sync`
→ `host.block_entity_sync`; `game.native_component`/`native_set_component` →
`ItemResource::value` / `ItemStack::apply_components`; `game.native_with_mod_block_entity` →
`host.block_entity(level, pos)` handles (`item_slot`/`set_item_slot` inside `host.transaction`,
`merge_data`); `game.held()`/`native_give_stack` → `player.inventory_slots`/`set_inventory_slot`
with the same "same-item slot with room, then the lowest empty slot" order.

### The ladder (all inside `build-slot.sh` leases, `CARGO_INCREMENTAL=0`, no `--target-dir`)

There is no staging step and no staged tree for an SDK package: the crate is built in place against
`.horizon/sdk/`. Every step ran with `BUILD_LOCAL=1` (the slot runner as the lease, the SDK ladder
driven directly; the automatic forward has no copy of `.horizon/sdk`).

| Step | Command | Result |
|---|---|---|
| runtime first (before any glue) | `cargo test --offline` | **16 passed**, 0 failed — `evidence/hostilenetworks/sdk-test-runtime.log` |
| glue red | the same, with the glue's functions stubbed | **16 passed / 10 failed** (`not yet implemented`) — `sdk-test-glue-red.log` |
| tests | `cargo test --offline` on the final sources | **26 passed**, 0 failed (16 `runtime::`, 10 `support::`) — `sdk-test.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** — `sdk-clippy.log` |
| format | `cargo fmt --check` (**not** `--all`) | `FMT_EXIT:0` — `sdk-fmt.log`; `cmp src/runtime.rs` against the retired copy still clean after `cargo fmt` |
| package | `rm -f Cargo.lock && cargo horizon-mod build` | `SDK 0b14f8a … in ./.horizon/sdk`, `Built ./target/horizon-mod/hostilenetworks.hmod` — `sdk-build.log` |
| second build | the same again, lock removed | **identical bytes** (`cmp` clean), sha256 `dfeeb23f…9ba7c` — `sdk-build-second.log` |
| registry check | `cargo horizon-mod check hostilenetworks.hmod` | see below, **CHECK_EXIT:0** — `sdk-check.log` |
| machine form | `cargo horizon-mod check … --json` | `{"ok":true,…,"kind":"native-mod","size":62111,"version":"0.2.0"}` — `sdk-check-json.log` |
| fits the pinned build | `cargo horizon-mod fits hostilenetworks.hmod --build nightly` | `Hostile Neural Networks:` / `0b14f8a  made for`, **FITS_EXIT:0** — `sdk-fits.log` |
| what it uses | `cargo horizon-mod check … --uses` | 337 entries — `sdk-uses.log`, `sdk-uses-from-package.json`, `sdk-uses-summary.txt` |
| runtime untouched | `unzip -p hostilenetworks.hmod src/runtime.rs \| sha256sum` | `4f00bd87…a723` = the retired file, byte for byte |

`check` output, verbatim:

```
hostilenetworks 0.2.0 (native-mod, format native-1/0b14f8acfd97fa2c750e42795249dcc981178ca4/e59da9094f0c2fdda6c627fe9eff22dec6a906f0, for 26.3)
  62111 bytes, SHA-256 dfeeb23f39b8dd1c5047042b2425c7f3ec3d258b41659ce8df215a18c097ba7c, unsigned
  native source: unsandboxed; not executed by the registry check. Compile and test against the exact pinned Horizon and Mixins revisions before review.
```

`fits` output, verbatim:

```
Hostile Neural Networks:
0b14f8a  made for
```

### The package

- **62,111 B**, sha256 `dfeeb23f39b8dd1c5047042b2425c7f3ec3d258b41659ce8df215a18c097ba7c`, **54 entries**.
- Layout: 6 root entries (`horizon-mod.toml`, `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`,
  `README.md`, `uses.json`), 29 under `assets/`, 16 under `data/`, 3 under `src/` (`lib.rs`,
  `runtime.rs`, `support.rs`).
- **No `Cargo.lock`** in the archive; `LICENSE` and `LICENSE-NOTICE.md` are inside it.
- Reproducible: the second build is byte-identical (`cmp` clean).

`uses.json` (337 entries) is the migration's best summary: 10 hook families
(`block_entity`, `block_entity_ticks`, `block_entity_tick`, `screen`, `screen_draw`, `screen_click`,
`menu_button`, `component_types`, `register_commands`, `run_mod_command`), 7 `BlockEntityPart`
methods, 4 `Registrar` declarations (`block_key`, `item_key`, `block_container`, `block_energy`),
the `registries` capability, both macros, 25 traits and 157 types. Full breakdown in
`sdk-uses-summary.txt`.

### Manifest mapping

- `[mod] id` → `id` (also the resource namespace), `[mod] version` → `0.2.0` (the port's version, not
  upstream's 6.5.1), `[mod] priority = 100` → gone (ordering is `#[listener(priority = …)]`; nothing
  here needs it).
- `[compatibility] horizon_revision` / the bundle's mixins revision → `[native] horizon` /
  `[native] mixins`, **exact 40-hex** (`0b14f8acfd97fa2c750e42795249dcc981178ca4` /
  `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`; the mixins commit moved from the old `5a64af93`).
- `api = "0.2"`, `side = "both"` (an integrated server belongs to the client installation, so single
  player and a LAN host keep the machines), `capabilities = ["registries"]` — the schema's one shared
  value, used here because the content declares blocks, items and menus.
- No `license`/`homepage`/`issues` fields: the schema refuses them and `build` strips them; the
  licence travels as `LICENSE` + `LICENSE-NOTICE.md` inside the package.

### Traps this migration cost (beyond the recipe's)

1. **A block on a vanilla template gets a mod block entity type named after the block.** The retired
   `block_entity` hook gave any block id an entity; here the type must be *declared* and attached,
   and `.blocks(&["sim_chamber"])` names `hostilenetworks:sim_chamber` (a bare name is this mod's).
   One Part per machine is therefore the natural shape: `block_entity_tick` and `menu_button`
   dispatch by **entity type**, while `block_entity`, `block_entity_ticks`, `screen*` dispatch by
   **block** — a single Part on both types would have needed an id test in every tick.
2. **`Tag::Int` binds by value from `ItemResource::value`.** `value()` returns `Option<Tag>`
   (owned), so `Some(Tag::Int(v)) => v`, while `Tag::get(&str)` returns `Option<&Tag>` and the same
   pattern needs `*v`. Mixing them is two `E0614`s.
3. **`DimensionKey::id(host)` is the handle, not `host.dimension_key_id(key)`.** The latter is a
   backend read; `/hostilenetworks …` resolves its level with
   `invocation.dimension.id(&*host)` → `host.level(&id)`, so `execute in <dim> run …` reads the right
   level.
4. **`host.transaction(…)` needs the `HostTransaction` trait in scope** (`no method named
   'transaction'`), and `BlockEntityRef::set_item_slot` only exists inside it.
5. **`cargo test` writes `Cargo.lock`, and a packed lock changes the artifact.** Removed immediately
   before each `cargo horizon-mod build`; `.gitignore` carries `/Cargo.lock`.
6. **`cargo fmt` must not touch `runtime.rs`.** It did not (the retired file was already rustfmt
   output), but the `cmp` is run after every format pass regardless.
7. **`build-slot.sh run` reports the exit of the last pipeline element.** A `cargo test | tee` leg
   returned `RUNNER_EXIT:0` on a red run until the wrapped command was given `set -o pipefail` and
   printed `CARGO_EXIT:${PIPESTATUS[0]}`; every ladder claim above quotes that value, not the
   runner's.

### Behaviour that changed with the format (deliberate)

- **The embedded-content workaround is gone.** 0.1.0 wrote its content `.hmod` to `.horizon-cache/`
  and appended it to the dev list because the registry keeps one file per project. The flat package
  carries `assets/` + `data/`, so that injection, its static `native_data!` blob and its control leg
  are deleted. The 0.1.0 control (`EMBEDDED=1` vs `.horizon-cache` made unusable) is therefore
  historical.
- **Block dispatch is by entity type in the tick.** Observable only as: a Part cannot be accidentally
  shared between the two machines. The saved keys, slot indices, energy amounts and readout text are
  unchanged, so an existing world keeps its machines' state.
- **The random draw goes through the level's own stream** (`LevelRef::with_random`), which is what
  `level.random.nextFloat()` wrote back in Java; a copy would have desynced the level's stream.
- **Model, data and iterations are written by rewriting the stack** (`apply_components`) rather than
  mutating a component in place. Same components, same ids; the stack is replaced in its slot.

### Gaps after the migration

Everything in the 0.1.0 feature gap list above still stands (kill-driven model data, attunement, the
other 58 entity and 10 block models, the Deep Learner, the Data Center, Queue mode, the redstone
modes, training-mode data upgrades, the GUI background and the model-rendering item). Added or
confirmed by this format:

1. **No 0.2.0 in-game run.** The e2e needs a game the launcher built with this package
   (`horizon-launcher native-build ports/hostilenetworks --instance <i> --trust-native-code`) and a
   `horizon-ctl` of the same build; neither is installed in this environment and the campaign's
   shared binaries are pinned at `1d63e0a5`/`41f822b6`, not `0b14f8a`. `tools/e2e.sh` is updated for
   a launcher-built game (`GAME=`, optional `CONTROL_GAME=`, `SELFTEST=1` intact) and its assertions
   are unchanged; the 0.1.0 runs in `ports/_shared/evidence/hostilenetworks/` remain the behaviour's
   evidence for a `runtime.rs` that did not change. **No in-game claim is made for 0.2.0.**
2. **The barrel template's block entity is assumed replaced by the declared one.** 0.1.0's
   `block_entity` hook gave the block a mod entity at the pin; the SDK declares a type for the block
   instead. That the declared type (rather than a template-provided vanilla block entity) is the one
   the world creates is exactly what the missing in-game run would prove.
3. **`Registrar::block_container` replaces the hook's container declaration**, so the 4/17 slots and
   their menu ids are declared the same way, but whether the vanilla barrel's own container path
   interferes is likewise unproven here (the 0.1.0 run answered no).
4. **No logger.** A failed slot write or a refused component write is silent; the readout command is
   the only diagnostic (the SDK exports no `log` crate and a native mod may not add one).

`## Upstream defects fixed`: none found in this migration. The carried recipes, tags, loot tables
and model table are the 0.1.0 ones, unchanged (the 0.1.0 defects section, if any, still applies).

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

### Changelog for the first publication (0.2.0)

> **0.2.0 — first publication as an SDK `.hmod`.** Horizon's mod-API merge (launcher ≥ 0.2.70)
> retired the `.hnmod` container, so this is the first version of Hostile Neural Networks for
> Horizon published in the current format: one flat `hostilenetworks.hmod` carrying the code *and*
> the carried content (2 blocks, 5 items, 3 item components, 2 menus, 3 recipes, 2 loot tables, the
> pickaxe tag, the creative tab, lang, models and textures), `api = "0.2"`, `[native]` pinned to
> nightly `0b14f8acfd97fa2c750e42795249dcc981178ca4` and mixins
> `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, for game 26.3. The retired bundle's twelve hook helpers
> became two block entity Parts (`block_entity`, `block_entity_ticks`, `block_entity_tick`, `screen`,
> `screen_draw`, `screen_click`, `menu_button`) plus four declarations (two containers, two FE
> buffers), `component_types` for the three item components, and the typed
> `register_commands`/`run_mod_command` pair (8 usages, unchanged text); the embedded-content
> injection is gone because the package carries its content itself. `src/runtime.rs` and its 16
> tests are unchanged, byte for byte; the glue adds 10 tests. Implements the Simulation Chamber →
> Prediction → Loot Fabricator loop with the zombie model. Not yet: kill-driven model data,
> attunement, the other 68 models, the Deep Learner, the Data Center, Queue mode, redstone modes and
> the custom GUI background. No in-game run is claimed for this version — the launcher is not
> installed here (the 0.1.0 runs remain the behaviour's evidence).

### Changelog 0.2.3 (2026-11-21)

> **0.2.3 — training mode is real, and the package is re-pinned.** (0.2.2 was a local intermediate, never published: it stamped an explicit `hostilenetworks:data` 0 on a model after an **inference** run, which upstream never does; 0.2.3 gates the data write on `data_gained > 0`.) Training mode now implements
> the other half of `SimChamberTileEntity.serverTick`: a finished 300-tick / 1.2×-energy run adds
> one data to the model in slot 0, adds **no** base drop and **no** prediction, and still counts an
> iteration; a self-aware model is refused with the new `FailureState.CANNOT_TRAIN`
> (`fail cannot_train`) before the output/energy gates, in upstream's gate order (input → redstone
> → cannot-train → output → energy). 0.2.1's training run wrongly produced the inference loot and
> left the model's data untouched. Re-pinned from `0b14f8a` to the campaign pin **`0a875da`**
> (mixins unchanged, `package_format` 6, `content_api` 0.2), and the pure `Item` struct renamed
> `ItemCount` so `fits --build 0b14f8a` stops matching it to the SDK's `Item` and false-failing.
> 31 unit tests. In game: a basic model went 20 → 21 data with no loot and exactly 299 × 154 FE
> spent, a self-aware model answered `cannot_train` without consuming its input or its energy, all
> three carried recipes crafted and consumed their ingredients, every carried item was given and
> read back with an unknown id refused, and both machines dropped themselves to a survival
> pickaxe; control `EXIT:1`, `SELFTEST=1` `EXIT:1`, `NEGATIVE=1` `EXIT:1`.
