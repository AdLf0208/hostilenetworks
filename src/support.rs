//! The machines' glue: their block entity Parts, container, buffer, menu and screen declarations,
//! the item components a Data Model and a Prediction carry, and the `/hostilenetworks` commands.
//!
//! Every gameplay rule lives in [`crate::runtime`] (byte-identical to the retired bundle's
//! `runtime.rs`, with its own tests); this file reads the game, calls it and writes the result
//! back. The tests at the bottom pin what it *declares* (slots, faces, menus, screens, buffers),
//! the saved-int codecs, and the exact readout lines the in-game check asserts.
//!
//! The retired bundle's twelve injected helpers are here three shapes: each machine's block
//! entity, tick, screen, draw, click and menu button are one **Part** attached to the type
//! declared for its block; its slots, FE buffer and menu are **declarations**; the harness
//! command is the typed `register_commands`/`run_mod_command` pair. Its embedded-content helper
//! has no successor: one `.hmod` carries the content and the code.
use std::collections::BTreeMap;

use mod_api::{
    self, BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent,
    BlockPart, ClientHost, Command, CommandArgumentValue, CommandInvocation, CommandSource,
    ComponentId, Compound, Flow, Host, HostTransaction, Identifier, ItemResource, ItemStack,
    LevelRef, LevelSide, MenuButtonEvent, ModContainer, ModEnergyBuffer, PlayerRef, RandomSource,
    RegisterCommandsEvent, Registrar, RegistrationContext, RunModCommandEvent, Screen,
    ScreenClickEvent, ScreenDrawEvent, ScreenEvent, ServerHost, Tag, Text,
};

use crate::runtime::{
    ChamberAction, ChamberView, DATA_COMPONENT, DATA_MODEL_COMPONENT, DATA_MODEL_ITEM,
    FAB_POWER_CAP, FAB_RUNTIME, FabAction, FabView, FailState, ITERATIONS_COMPONENT, Item, MODELS,
    Mode, PREDICTION_ITEM, SIM_POWER_CAP, chamber_action, data_cap_for, fab_action, fab_drop,
    is_chamber, is_fabricator, model, tier_for,
};

/// The two machines' block ids.
pub(crate) const CHAMBER: &str = crate::runtime::SIM_CHAMBER;
pub(crate) const FABRICATOR: &str = crate::runtime::LOOT_FABRICATOR;
/// Their block entity type paths: the type id is `<namespace>:<path>`, the block's own id.
const CHAMBER_TYPE: &str = "sim_chamber";
const FABRICATOR_TYPE: &str = "loot_fabricator";
/// The menu types their content declares (`data/hostilenetworks/horizon/menu/`).
pub(crate) const CHAMBER_MENU: &str = "hostilenetworks:sim_chamber";
pub(crate) const FAB_MENU: &str = "hostilenetworks:loot_fabricator";

/// The chamber's slots: model, input, base drop, prediction.
pub(crate) const CHAMBER_SLOTS: usize = 4;
/// The fabricator's slots: prediction, then its 4x4 outputs.
pub(crate) const FAB_SLOTS: usize = 17;

/// A full stack at this pin's default (`minecraft:max_stack_size` unset).
const MAX_STACK: u32 = 64;

/// The saved keys, named as upstream's `saveAdditional` names them.
const KEY_ENERGY: &str = "energy";
const KEY_RUNTIME: &str = "runtime";
const KEY_PRED: &str = "pred";
const KEY_FAIL: &str = "fail";
const KEY_MODE: &str = "mode";
const KEY_MODEL: &str = "model";
const KEY_SELECTION: &str = "selection";
const KEY_SEL_MODEL: &str = "selModel";

/// The menu button id that switches the chamber's mode.
pub(crate) const CHAMBER_MODE_BUTTON: i32 = 100;

// ---------------------------------------------------------------------------
// Saved-state codecs
// ---------------------------------------------------------------------------

/// An int field of a saved compound, `0` when it is absent or another type.
pub(crate) fn int(saved: &Tag, key: &str) -> i32 {
    match saved.get(key) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

/// An int field, or `default` when it is absent.
pub(crate) fn int_or(saved: &Tag, key: &str, default: i32) -> i32 {
    match saved.get(key) {
        Some(Tag::Int(value)) => *value,
        _ => default,
    }
}

pub(crate) fn set_int(saved: &mut Tag, key: &str, value: i32) {
    if let Tag::Compound(nbt) = saved {
        nbt.insert(key.into(), Tag::Int(value));
    }
}

/// A string field of a saved compound.
fn string(saved: &Tag, key: &str) -> Option<String> {
    match saved.get(key) {
        Some(Tag::String(value)) => Some(value.to_string_lossy()),
        _ => None,
    }
}

fn set_string(saved: &mut Tag, key: &str, value: &str) {
    if let Tag::Compound(nbt) = saved {
        nbt.insert(key.into(), Tag::String(value.into()));
    }
}

/// The mode a saved int names (`0` inference, `1` training).
pub(crate) fn mode_of(value: i32) -> Mode {
    if value == 1 {
        Mode::Training
    } else {
        Mode::Inference
    }
}

pub(crate) fn mode_index(mode: Mode) -> i32 {
    if matches!(mode, Mode::Training) { 1 } else { 0 }
}

/// The fail state a saved int names.
pub(crate) fn fail_of(value: i32) -> FailState {
    match value {
        1 => FailState::Output,
        2 => FailState::Energy,
        3 => FailState::Input,
        4 => FailState::Model,
        5 => FailState::Faulty,
        6 => FailState::EnergyMidCycle,
        7 => FailState::Redstone,
        _ => FailState::None,
    }
}

pub(crate) fn fail_index(fail: FailState) -> i32 {
    match fail {
        FailState::None => 0,
        FailState::Output => 1,
        FailState::Energy => 2,
        FailState::Input => 3,
        FailState::Model => 4,
        FailState::Faulty => 5,
        FailState::EnergyMidCycle => 6,
        FailState::Redstone => 7,
    }
}

// ---------------------------------------------------------------------------
// Items and their components
// ---------------------------------------------------------------------------

/// The three item components, in the order `component_types` declares them.
pub(crate) fn component_ids() -> [&'static str; 3] {
    [DATA_MODEL_COMPONENT, DATA_COMPONENT, ITERATIONS_COMPONENT]
}

/// One of the three component ids, as the reads and writes name it.
fn component(id: &str) -> ComponentId {
    ComponentId::new(id).expect("the mod's component ids are namespaced")
}

/// The model a stack's `hostilenetworks:data_model` component names.
fn stack_model(host: &dyn Host, stack: &ItemStack) -> Option<&'static crate::runtime::Model> {
    match ItemResource::of(stack).value(host, &component(DATA_MODEL_COMPONENT))? {
        Tag::String(id) => model(&id.to_string_lossy()),
        _ => None,
    }
}

/// The data a model stack carries (`hostilenetworks:data`).
fn stack_data(host: &dyn Host, stack: &ItemStack) -> i32 {
    match ItemResource::of(stack).value(host, &component(DATA_COMPONENT)) {
        Some(Tag::Int(data)) => data,
        _ => 0,
    }
}

/// The iterations a model stack carries (`hostilenetworks:iterations`).
fn stack_iterations(host: &dyn Host, stack: &ItemStack) -> i32 {
    match ItemResource::of(stack).value(host, &component(ITERATIONS_COMPONENT)) {
        Some(Tag::Int(iters)) => iters,
        _ => 0,
    }
}

/// `stack` with one component set to a value: the write `applyComponents` performs, which rewrites
/// the stack instead of the item, so a stack of one never leaks the value onto its neighbours.
fn with_component(stack: &ItemStack, id: &str, value: Tag) -> Option<ItemStack> {
    let mut typed = stack.clone();
    let mut patch = Compound::new();
    patch.insert(id.into(), value);
    typed.apply_components(&patch).ok()?;
    Some(typed)
}

fn with_stack_model(stack: &ItemStack, model_id: &str) -> Option<ItemStack> {
    with_component(stack, DATA_MODEL_COMPONENT, Tag::String(model_id.into()))
}

fn with_stack_data(stack: &ItemStack, data: i32) -> Option<ItemStack> {
    with_component(stack, DATA_COMPONENT, Tag::Int(data))
}

fn with_stack_iterations(stack: &ItemStack, iterations: i32) -> Option<ItemStack> {
    with_component(stack, ITERATIONS_COMPONENT, Tag::Int(iterations))
}

/// A stack of `count` of item `id`, or none when this world has no such item.
fn new_stack(host: &dyn Host, id: &str, count: u32) -> Option<ItemStack> {
    let key = Identifier::parse(id)
        .ok()
        .and_then(|id| host.item_key(&id))?;
    ItemStack::new(key, count)
}

/// Whether `slot` can take `count` more of item `id` (`canStack`): empty, or the same item with
/// room for the whole stack.
fn slot_takes(host: &dyn Host, slot: Option<&ItemStack>, id: &str, count: u32) -> bool {
    match slot {
        None => true,
        Some(stack) => stack.item().id(host) == id && stack.count() + count <= MAX_STACK,
    }
}

/// Whether the chamber's prediction output can take `model_id`'s prediction: an empty slot, or
/// the same model's prediction with room for one more.
fn prediction_slot_takes(host: &dyn Host, slot: Option<&ItemStack>, model_id: &str) -> bool {
    match slot {
        None => true,
        Some(stack) => {
            stack.item().id(host) == PREDICTION_ITEM
                && stack.count() < MAX_STACK
                && stack_model(host, stack).is_some_and(|held| held.id == model_id)
        }
    }
}

/// Adds `item` (and, for a prediction, its model component) to a slot `slot_takes` accepted.
fn add_to_slot(host: &dyn Host, slot: &mut Option<ItemStack>, item: Item, model_id: Option<&str>) {
    match slot {
        Some(stack) => {
            let count = stack.count() + item.count;
            let _ = stack.set_count(count.min(MAX_STACK));
        }
        None => {
            let Some(mut stack) = new_stack(host, item.id, item.count) else {
                return;
            };
            if let Some(model_id) = model_id
                && let Some(typed) = with_stack_model(&stack, model_id)
            {
                stack = typed;
            }
            *slot = Some(stack);
        }
    }
}

// ---------------------------------------------------------------------------
// Declarations
// ---------------------------------------------------------------------------

/// The chamber's container: four slots every face reaches — model 0, input 1, then the two
/// outputs — and the chamber menu. Only the two inputs take automation's items, so a hopper never
/// fills an output.
pub(crate) fn chamber_container() -> ModContainer {
    ModContainer {
        slots: CHAMBER_SLOTS,
        faces: std::array::from_fn(|_| vec![0, 1, 2, 3]),
        inputs: vec![0, 1],
        outputs: vec![2, 3],
        menu: Some(CHAMBER_MENU.to_owned()),
    }
}

/// The fabricator's container: its prediction slot and its 4x4 outputs, every face reaching
/// every slot, and only slot 0 an input.
pub(crate) fn fabricator_container() -> ModContainer {
    ModContainer {
        slots: FAB_SLOTS,
        faces: std::array::from_fn(|_| (0..FAB_SLOTS).collect()),
        inputs: vec![0],
        outputs: (1..FAB_SLOTS).collect(),
        menu: Some(FAB_MENU.to_owned()),
    }
}

/// The chamber's FE buffer (`simPowerCap`): upstream's `ModifiableEnergyStorage(cap, cap)` takes
/// energy and gives none back (`setMaxExtract(0)`), so every side fills and none drains.
pub(crate) fn chamber_battery() -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity: SIM_POWER_CAP,
        max_insert: SIM_POWER_CAP,
        max_extract: 0,
        insert_faces: [true; 6],
        extract_faces: [false; 6],
    }
}

/// The fabricator's FE buffer (`fabPowerCap`), filled only, as the chamber's is.
pub(crate) fn fabricator_battery() -> ModEnergyBuffer {
    ModEnergyBuffer {
        capacity: FAB_POWER_CAP,
        max_insert: FAB_POWER_CAP,
        max_extract: 0,
        insert_faces: [true; 6],
        extract_faces: [false; 6],
    }
}

/// Where the chamber's four slots sit inside its panel.
const CHAMBER_SLOTS_AT: [[f32; 2]; CHAMBER_SLOTS] =
    [[26.0, 20.0], [62.0, 20.0], [116.0, 20.0], [152.0, 20.0]];

/// Where the fabricator's slot `slot` sits: the prediction, then the 4x4 outputs.
fn fabricator_slot_at(slot: usize) -> [f32; 2] {
    if slot == 0 {
        return [62.0, 20.0];
    }
    let cell = slot - 1;
    let column = cell % 4;
    let row = cell / 4;
    [100.0 + column as f32 * 18.0, 20.0 + row as f32 * 18.0]
}

/// The chamber's menu (`native_screen`): upstream's 176 x 166 panel with its four slots.
pub(crate) fn chamber_screen() -> Screen {
    Screen {
        width: 176.0,
        height: 166.0,
        slots: CHAMBER_SLOTS_AT.iter().map(|at| Some(*at)).collect(),
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        ..Screen::default()
    }
}

/// The fabricator's menu: upstream's 176 x 186 panel, tall enough for the 4x4 grid.
pub(crate) fn fabricator_screen() -> Screen {
    Screen {
        width: 176.0,
        height: 186.0,
        slots: (0..FAB_SLOTS)
            .map(|slot| Some(fabricator_slot_at(slot)))
            .collect(),
        inventory_y: 104.0,
        inventory_x: 8.0,
        own_labels: false,
        own_background: false,
        ..Screen::default()
    }
}

/// Declare the two machines: their block entity types, Parts, containers, buffers and menus.
/// A block on a vanilla template has a block entity type of its own, named after the block, so
/// the type declared here is the one the block already has; `.blocks` binds it explicitly.
pub(crate) fn declare(registrar: &mut dyn Registrar, context: &RegistrationContext) {
    let chamber_block = registrar.block_key(CHAMBER);
    registrar.block_container(&chamber_block, chamber_container());
    registrar.block_energy(&chamber_block, chamber_battery());
    let chamber = registrar
        .block_entity_type(CHAMBER_TYPE)
        .blocks(&[CHAMBER_TYPE])
        .build();
    if let Err(error) = registrar.block_entity_part(&chamber, ChamberPart) {
        context.refuse(&error.to_string());
    }

    let fabricator_block = registrar.block_key(FABRICATOR);
    registrar.block_container(&fabricator_block, fabricator_container());
    registrar.block_energy(&fabricator_block, fabricator_battery());
    let fabricator = registrar
        .block_entity_type(FABRICATOR_TYPE)
        .blocks(&[FABRICATOR_TYPE])
        .build();
    if let Err(error) = registrar.block_entity_part(&fabricator, FabricatorPart) {
        context.refuse(&error.to_string());
    }
}

// ---------------------------------------------------------------------------
// The Simulation Chamber's Part
// ---------------------------------------------------------------------------

/// The Simulation Chamber: a block entity that ticks on the server, with its menu and screen.
pub(crate) struct ChamberPart;

#[mod_api::horizon_part]
impl BlockPart for ChamberPart {}

#[mod_api::horizon_part]
impl BlockEntityPart for ChamberPart {
    /// `EntityBlock.newBlockEntity`: an empty saved compound; the tick fills it.
    fn block_entity(
        &self,
        _level: LevelSide,
        _event: &mut BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    /// `EntityBlock.getTicker`: it ticks. Never clear a value an earlier mod set.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick of `SimChamberTileEntity.serverTick`.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        chamber_tick(host, event);
    }

    /// The chamber's screen layout.
    fn screen(&self, _host: &dyn Host, _event: &mut ScreenEvent<'_>) -> Option<Screen> {
        Some(chamber_screen())
    }

    /// The readouts over it: FE, progress, mode and failure.
    fn screen_draw(&self, _host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        chamber_draw(event);
    }

    /// A press on the mode line names its menu button.
    fn screen_click(
        &self,
        _host: &dyn ClientHost,
        event: &mut ScreenClickEvent<'_>,
    ) -> Option<i32> {
        chamber_click(event)
    }

    /// The server applies the mode switch.
    fn menu_button(&self, _host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
        chamber_button(event);
    }
}

/// `SimChamberTileEntity.serverTick`, applied. The whole state lives in the block entity's saved
/// compound (`energy`, `runtime`, `pred`, `fail`, `mode`, `model`) and its slots, so a save and a
/// load keep it, as upstream's `saveAdditional` does.
fn chamber_tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    event.entity.items.resize(CHAMBER_SLOTS, None);
    let mut saved = Tag::Compound(std::mem::take(&mut event.entity.nbt));

    let model_stack = event.entity.items[0].clone();
    let found = model_stack
        .as_ref()
        .and_then(|stack| stack_model(&*host, stack));
    let data = model_stack
        .as_ref()
        .map_or(0, |stack| stack_data(&*host, stack));
    // A different model in the slot restarts the run, as upstream's
    // `oldModel != this.currentModel` does.
    let model_id = found.map(|found| found.id);
    if string(&saved, KEY_MODEL).as_deref() != model_id {
        set_int(&mut saved, KEY_RUNTIME, 0);
        set_string(&mut saved, KEY_MODEL, model_id.unwrap_or("null"));
    }
    let energy = int(&saved, KEY_ENERGY).min(SIM_POWER_CAP);
    let runtime = int(&saved, KEY_RUNTIME);
    let prediction_success = int(&saved, KEY_PRED);
    let mode = mode_of(int(&saved, KEY_MODE));
    let has_input = found.is_some_and(|found| {
        event.entity.items[1]
            .as_ref()
            .is_some_and(|stack| stack.item().id(&*host) == found.input)
    });
    let output_room = found.is_some_and(|found| {
        slot_takes(
            &*host,
            event.entity.items[2].as_ref(),
            found.base_drop.id,
            1,
        ) && prediction_slot_takes(&*host, event.entity.items[3].as_ref(), found.id)
    });
    let sample = random_float(host);
    let view = ChamberView {
        model: found,
        data,
        has_input,
        output_room,
        // Redstone control (upstream's `RedstoneState::IGNORED` default) is a gap in this slice:
        // the chamber always runs when it can.
        powered: true,
        energy,
        runtime,
        prediction_success,
        mode,
        sample,
    };
    let mut fail = FailState::None;
    match chamber_action(&view) {
        ChamberAction::Idle(reason) => {
            fail = reason;
            if matches!(reason, FailState::Model | FailState::Faulty) {
                set_int(&mut saved, KEY_RUNTIME, 0);
            }
        }
        ChamberAction::Start {
            runtime,
            predictions,
        } => {
            set_int(&mut saved, KEY_RUNTIME, runtime);
            set_int(&mut saved, KEY_PRED, predictions);
            if let Some(input) = event.entity.items[1].as_mut() {
                let left = input.count().saturating_sub(1);
                match left {
                    0 => event.entity.items[1] = None,
                    left => {
                        let _ = input.set_count(left);
                    }
                }
            }
        }
        ChamberAction::Step { energy_cost } => {
            set_int(&mut saved, KEY_RUNTIME, runtime - 1);
            set_int(&mut saved, KEY_ENERGY, (energy - energy_cost).max(0));
        }
        ChamberAction::Finish {
            base_drop,
            predictions,
            iterations,
        } => {
            add_to_slot(&*host, &mut event.entity.items[2], base_drop, None);
            if predictions > 0
                && let Some(found) = found
            {
                add_to_slot(
                    &*host,
                    &mut event.entity.items[3],
                    Item::new(PREDICTION_ITEM, predictions),
                    Some(found.id),
                );
            }
            set_int(&mut saved, KEY_RUNTIME, 0);
            if let Some(stack) = event.entity.items[0].as_ref() {
                let count = stack_iterations(&*host, stack) + iterations as i32;
                if let Some(typed) = with_stack_iterations(stack, count) {
                    event.entity.items[0] = Some(typed);
                }
            }
            host.block_entity_sync(pos);
        }
    }
    set_int(&mut saved, KEY_FAIL, fail_index(fail));
    let stored = int(&saved, KEY_ENERGY);
    let data_slots = vec![
        int(&saved, KEY_RUNTIME),
        int(&saved, KEY_PRED),
        fail_index(fail),
        int(&saved, KEY_MODE),
        stored & 0xffff,
        stored >> 16,
    ];
    if let Tag::Compound(nbt) = saved {
        event.entity.nbt = nbt;
    }
    if event.entity.data != data_slots {
        event.entity.data = data_slots;
    }
}

/// Draws the chamber's readouts from the menu's data slots `[runtime, pred, fail, mode, energy
/// low, energy high]`. The text is plain GUI text at fixed spots; a full custom background is a
/// gap (the carried GUI texture is not painted).
fn chamber_draw(event: &mut ScreenDrawEvent<'_>) {
    let data = event.data;
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    let runtime = data.first().copied().unwrap_or(0);
    let predictions = data.get(1).copied().unwrap_or(0);
    let fail = fail_of(data.get(2).copied().unwrap_or(0));
    let mode = mode_of(data.get(3).copied().unwrap_or(0));
    let energy = energy_of(data, 4, 5);
    draw.text(
        &format!("{energy} / {SIM_POWER_CAP} FE"),
        [x + 8.0, y + 44.0],
        0xFF40_4040,
        false,
    );
    let run = mode.runtime();
    draw.text(
        &format!("{}/{} ticks ({})", run - runtime, run, mode.name()),
        [x + 8.0, y + 54.0],
        0xFF40_4040,
        false,
    );
    draw.text(
        &format!("Predictions this run: {predictions}"),
        [x + 8.0, y + 64.0],
        0xFF40_4040,
        false,
    );
    if fail != FailState::None {
        draw.text(fail.name(), [x + 8.0, y + 74.0], 0xFF8B_0000, false);
    }
}

/// A press on the mode line (the progress row, left of the text) names the mode button.
fn chamber_click(event: &mut ScreenClickEvent<'_>) -> Option<i32> {
    let at = event.at;
    if at[0] < 8.0 || at[0] > 96.0 {
        return None;
    }
    if (44.0..54.0).contains(&at[1]) && at[0] < 60.0 {
        return Some(CHAMBER_MODE_BUTTON);
    }
    None
}

/// The server applies the chamber's mode button: `SimMode.next`, and the run restarts.
fn chamber_button(event: &mut MenuButtonEvent<'_>) {
    if event.context.id != CHAMBER_MODE_BUTTON {
        return;
    }
    let mut saved = Tag::Compound(std::mem::take(&mut event.entity.nbt));
    let mode = mode_of(int(&saved, KEY_MODE)).next();
    set_int(&mut saved, KEY_MODE, mode_index(mode));
    set_int(&mut saved, KEY_RUNTIME, 0);
    if let Tag::Compound(nbt) = saved {
        event.entity.nbt = nbt;
    }
}

// ---------------------------------------------------------------------------
// The Loot Fabricator's Part
// ---------------------------------------------------------------------------

/// The Loot Fabricator: a block entity that ticks on the server, with its menu and screen.
pub(crate) struct FabricatorPart;

#[mod_api::horizon_part]
impl BlockPart for FabricatorPart {}

#[mod_api::horizon_part]
impl BlockEntityPart for FabricatorPart {
    fn block_entity(
        &self,
        _level: LevelSide,
        _event: &mut BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One server tick of `LootFabTileEntity.serverTick`.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        fabricator_tick(host, event);
    }

    fn screen(&self, _host: &dyn Host, _event: &mut ScreenEvent<'_>) -> Option<Screen> {
        Some(fabricator_screen())
    }

    /// The readouts and the drop palette.
    fn screen_draw(&self, _host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        fabricator_draw(event);
    }

    /// A press on a palette row names its menu button.
    fn screen_click(
        &self,
        _host: &dyn ClientHost,
        event: &mut ScreenClickEvent<'_>,
    ) -> Option<i32> {
        fabricator_click(event)
    }

    /// The server applies the palette pick.
    fn menu_button(&self, host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
        fabricator_button(host, event);
    }
}

/// `LootFabTileEntity.serverTick`, applied. The selection is kept per model (`selModel`);
/// upstream keeps a map, which a single carried model makes a pair of fields here.
fn fabricator_tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    event.entity.items.resize(FAB_SLOTS, None);
    let mut saved = Tag::Compound(std::mem::take(&mut event.entity.nbt));

    let found = event.entity.items[0]
        .as_ref()
        .and_then(|stack| stack_model(&*host, stack));
    let model_id = found.map(|found| found.id);
    if string(&saved, KEY_SEL_MODEL).as_deref() != model_id {
        set_int(&mut saved, KEY_SELECTION, -1);
        set_string(&mut saved, KEY_SEL_MODEL, model_id.unwrap_or("null"));
        set_int(&mut saved, KEY_RUNTIME, 0);
    }
    let selection = int_or(&saved, KEY_SELECTION, -1);
    let energy = int(&saved, KEY_ENERGY).min(FAB_POWER_CAP);
    let runtime = int(&saved, KEY_RUNTIME);
    let drop = found.and_then(|found| fab_drop(found, selection));
    let output_room = drop.is_some_and(|drop| {
        (1..FAB_SLOTS).any(|slot| {
            slot_takes(
                &*host,
                event.entity.items[slot].as_ref(),
                drop.id,
                drop.count,
            )
        })
    });
    let view = FabView {
        model: found,
        selection,
        output_room,
        powered: true,
        energy,
        runtime,
    };
    let mut fail = FailState::None;
    match fab_action(&view) {
        FabAction::Idle(reason) => fail = reason,
        FabAction::Step { energy_cost } => {
            set_int(&mut saved, KEY_ENERGY, (energy - energy_cost).max(0));
            set_int(&mut saved, KEY_RUNTIME, runtime + 1);
        }
        FabAction::Produce { drop } => {
            if let Some(slot) = (1..FAB_SLOTS).find(|slot| {
                slot_takes(
                    &*host,
                    event.entity.items[*slot].as_ref(),
                    drop.id,
                    drop.count,
                )
            }) {
                add_to_slot(&*host, &mut event.entity.items[slot], drop, None);
            }
            if let Some(input) = event.entity.items[0].as_mut() {
                let left = input.count().saturating_sub(1);
                match left {
                    0 => event.entity.items[0] = None,
                    left => {
                        let _ = input.set_count(left);
                    }
                }
            }
            set_int(&mut saved, KEY_RUNTIME, 0);
            host.block_entity_sync(pos);
        }
    }
    set_int(&mut saved, KEY_FAIL, fail_index(fail));
    let model_index = found
        .and_then(|found| MODELS.iter().position(|entry| entry.id == found.id))
        .map_or(-1, |index| index as i32);
    let stored = int(&saved, KEY_ENERGY);
    let data_slots = vec![
        int(&saved, KEY_RUNTIME),
        int_or(&saved, KEY_SELECTION, -1),
        model_index,
        stored & 0xffff,
        stored >> 16,
    ];
    if let Tag::Compound(nbt) = saved {
        event.entity.nbt = nbt;
    }
    if event.entity.data != data_slots {
        event.entity.data = data_slots;
    }
}

/// Draws the fabricator's readouts and its palette from the menu's data slots `[runtime,
/// selection, model index, energy low, energy high]`.
fn fabricator_draw(event: &mut ScreenDrawEvent<'_>) {
    let data = event.data;
    let [x, y] = event.origin;
    let draw = &mut *event.draw;
    let runtime = data.first().copied().unwrap_or(0);
    let selection = data.get(1).copied().unwrap_or(-1);
    let found = usize::try_from(data.get(2).copied().unwrap_or(-1))
        .ok()
        .and_then(|index| MODELS.get(index));
    let energy = energy_of(data, 3, 4);
    draw.text(
        &format!("{energy} / {FAB_POWER_CAP} FE"),
        [x + 8.0, y + 44.0],
        0xFF40_4040,
        false,
    );
    draw.text(
        &format!("{runtime}/{FAB_RUNTIME} ticks"),
        [x + 8.0, y + 54.0],
        0xFF40_4040,
        false,
    );
    match found {
        Some(found) => {
            for (index, (id, count)) in found.fab_drops.iter().enumerate() {
                let colour = if index as i32 == selection {
                    0xFF00_A000
                } else {
                    0xFF40_4040
                };
                let short = id.trim_start_matches("minecraft:");
                draw.text(
                    &format!("{}. {short} x{count}", index + 1),
                    [x + 8.0, y + 66.0 + index as f32 * PALETTE_LINE],
                    colour,
                    false,
                );
            }
        }
        None => draw.text(
            "Insert a Prediction",
            [x + 8.0, y + 66.0],
            0xFF8B_0000,
            false,
        ),
    }
}

/// The palette's first row and its line height, shared with the click test.
const PALETTE_TOP: f32 = 66.0;
const PALETTE_LINE: f32 = 10.0;
/// The rows the palette always offers: the carried model's table length. The click is a
/// coordinate-to-row map, so it does not depend on the menu's data slots having reached this
/// client (the draw does, and shows the names).
const PALETTE_ROWS: usize = 4;

/// A press on a palette row names it as the menu button.
fn fabricator_click(event: &mut ScreenClickEvent<'_>) -> Option<i32> {
    let at = event.at;
    if at[0] < 8.0 || at[0] > 96.0 {
        return None;
    }
    let rows = usize::try_from(event.data.get(2).copied().unwrap_or(-1))
        .ok()
        .and_then(|index| MODELS.get(index))
        .map_or(PALETTE_ROWS, |found| found.fab_drops.len());
    (0..rows).find_map(|index| {
        let top = PALETTE_TOP + index as f32 * PALETTE_LINE;
        (top..top + PALETTE_LINE)
            .contains(&at[1])
            .then_some(index as i32)
    })
}

/// The server applies a palette pick: the row must name a drop the prediction's model has.
fn fabricator_button(host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
    let id = event.context.id;
    let found = event.entity.items[0]
        .as_ref()
        .and_then(|stack| stack_model(&*host, stack));
    let Some(found) = found else {
        return;
    };
    if fab_drop(found, id).is_none() {
        return;
    }
    let mut saved = Tag::Compound(std::mem::take(&mut event.entity.nbt));
    set_int(&mut saved, KEY_SELECTION, id);
    set_int(&mut saved, KEY_RUNTIME, 0);
    if let Tag::Compound(nbt) = saved {
        event.entity.nbt = nbt;
    }
}

// ---------------------------------------------------------------------------
// Machines as the commands read them
// ---------------------------------------------------------------------------

/// Which machine stands at a position, from the block id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Machine {
    Chamber,
    Fabricator,
}

fn machine_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<Machine> {
    let block = level.block(host, pos)?.id(host);
    if is_chamber(&block) {
        Some(Machine::Chamber)
    } else if is_fabricator(&block) {
        Some(Machine::Fabricator)
    } else {
        None
    }
}

/// The chamber as `/hostilenetworks info` reads it.
pub(crate) struct ChamberSight {
    pub(crate) energy: i32,
    pub(crate) runtime: i32,
    pub(crate) predictions: i32,
    pub(crate) mode: Mode,
    pub(crate) fail: FailState,
    pub(crate) model: String,
    pub(crate) tier: &'static str,
    pub(crate) data: i32,
    pub(crate) slots: Vec<(String, u32)>,
}

impl ChamberSight {
    pub(crate) fn line(&self) -> String {
        let slots = self
            .slots
            .iter()
            .map(|(id, count)| format!("{id} x{count}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Simulation Chamber: {} FE, runtime {}, predictions {}, mode {}, fail {}, model {}, \
             tier {}, data {}, slots [{}]",
            self.energy,
            self.runtime,
            self.predictions,
            self.mode.name(),
            self.fail.name(),
            self.model,
            self.tier,
            self.data,
            slots
        )
    }
}

/// The fabricator as `/hostilenetworks info` reads it.
pub(crate) struct FabSight {
    pub(crate) energy: i32,
    pub(crate) runtime: i32,
    pub(crate) selection: i32,
    pub(crate) fail: FailState,
    pub(crate) slots: Vec<(String, u32)>,
}

impl FabSight {
    pub(crate) fn line(&self) -> String {
        let slots = self
            .slots
            .iter()
            .map(|(id, count)| format!("{id} x{count}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Loot Fabricator: {} FE, runtime {}, selection {}, fail {}, slots [{}]",
            self.energy,
            self.runtime,
            self.selection,
            self.fail.name(),
            slots
        )
    }
}

/// The stacks in a machine's slots, non-empty ones only, as the readout prints them.
fn slot_list(host: &dyn Host, entity: mod_api::BlockEntityRef, slots: usize) -> Vec<(String, u32)> {
    (0..slots)
        .filter_map(|slot| entity.item_slot(host, slot as u32).ok().flatten())
        .map(|stack| (stack.item().id(host), stack.count()))
        .collect()
}

/// The level a command executes in, after `execute in`.
fn command_level(host: &dyn Host, invocation: &CommandInvocation) -> Option<LevelRef> {
    let id = invocation.dimension.id(host)?;
    host.level(&id)
}

/// `/hostilenetworks info`: the machine's readout, or `None` when no machine stands there.
fn info_line(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    let machine = machine_at(&*host, level, pos)?;
    let entity = host.block_entity(level, pos)?;
    let saved = Tag::Compound(entity.data(&*host)?);
    match machine {
        Machine::Chamber => {
            let model = string(&saved, KEY_MODEL).unwrap_or_else(|| "-".to_owned());
            let data = entity
                .item_slot(&*host, 0)
                .ok()
                .flatten()
                .map_or(0, |stack| stack_data(&*host, &stack));
            Some(
                ChamberSight {
                    energy: int(&saved, KEY_ENERGY),
                    runtime: int(&saved, KEY_RUNTIME),
                    predictions: int(&saved, KEY_PRED),
                    mode: mode_of(int(&saved, KEY_MODE)),
                    fail: fail_of(int(&saved, KEY_FAIL)),
                    model,
                    tier: tier_for(data).name(),
                    data,
                    slots: slot_list(&*host, entity, CHAMBER_SLOTS),
                }
                .line(),
            )
        }
        Machine::Fabricator => Some(
            FabSight {
                energy: int(&saved, KEY_ENERGY),
                runtime: int(&saved, KEY_RUNTIME),
                selection: int_or(&saved, KEY_SELECTION, -1),
                fail: fail_of(int(&saved, KEY_FAIL)),
                slots: slot_list(&*host, entity, FAB_SLOTS),
            }
            .line(),
        ),
    }
}

/// A machine's slot as a stack, if it holds one.
fn machine_slot(
    host: &dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    slot: u32,
) -> Option<ItemStack> {
    host.block_entity(level, pos)?
        .item_slot(host, slot)
        .ok()
        .flatten()
}

/// Write a machine's slot. The write joins a transaction so a failed one puts nothing back.
fn write_machine_slot(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    slot: u32,
    stack: Option<ItemStack>,
) -> bool {
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    host.transaction(|host, tx| -> Result<bool, ()> {
        Ok(entity
            .set_item_slot(host, tx, slot, stack.clone())
            .unwrap_or(false))
    })
    .unwrap_or(false)
}

/// The player's held (selected hotbar) stack and the slot it is in.
fn held(host: &dyn ServerHost, player: PlayerRef) -> Option<(u32, ItemStack)> {
    let slot = u32::try_from(player.selected_slot(host)?).ok()?;
    player
        .inventory_slots(host)?
        .into_iter()
        .find(|(at, _)| *at == slot)
        .map(|(_, stack)| (slot, stack))
}

/// Give `stack` to `player`: a same-item slot with room first, then the lowest empty inventory
/// slot. `None` when all of it landed, else what did not fit.
fn give_stack(host: &mut dyn ServerHost, player: PlayerRef, stack: ItemStack) -> Option<ItemStack> {
    let max = host
        .max_stack_size(stack.item())
        .unwrap_or(MAX_STACK)
        .max(1);
    let occupied: BTreeMap<u32, ItemStack> = player
        .inventory_slots(&*host)
        .unwrap_or_default()
        .into_iter()
        .filter(|(slot, _)| *slot < 36)
        .collect();
    let mut left = stack;
    for (slot, there) in &occupied {
        if left.count() == 0 {
            break;
        }
        if !there.is_stackable_with(&left) {
            continue;
        }
        let room = max.saturating_sub(there.count());
        if room == 0 {
            continue;
        }
        let moved = room.min(left.count());
        let mut merged = there.clone();
        if merged.set_count(there.count() + moved).is_err() {
            continue;
        }
        if player
            .set_inventory_slot(host, *slot, Some(merged))
            .is_err()
        {
            continue;
        }
        left = left.shrink(moved)?;
    }
    for slot in 0..36u32 {
        if occupied.contains_key(&slot) {
            continue;
        }
        if player
            .set_inventory_slot(host, slot, Some(left.clone()))
            .is_ok()
        {
            return None;
        }
    }
    Some(left)
}

/// A tick's prediction roll: a float in `0..1` from the level's own random stream, as upstream's
/// `level.random.nextFloat()` draws it.
fn random_float(host: &mut dyn ServerHost) -> f32 {
    let level = host.level(host.dimension());
    level
        .and_then(|level| level.with_random(host, |random| random.next_float()))
        .unwrap_or(0.0)
}

/// The energy split back out of the menu's two 16-bit data slots.
fn energy_of(data: &[i32], low: usize, high: usize) -> i32 {
    let low = data.get(low).copied().unwrap_or(0) as u16 as u32;
    let high = data.get(high).copied().unwrap_or(0) as u16 as u32;
    (high << 16 | low) as i32
}

// ---------------------------------------------------------------------------
// The harness commands
// ---------------------------------------------------------------------------

/// `/hostilenetworks ...` (`register_commands`). `info` reads a machine, `energy` charges it,
/// `model` gives or retypes a Data Model, `fab` selects a drop, `take`/`put` move a stack between
/// a machine and the player. They exist because a headless harness cannot work a screen's slots
/// or an energy network; they are recorded as a harness addition, not gameplay.
pub(crate) fn declare_commands(event: &mut RegisterCommandsEvent<'_>) {
    event
        .commands
        .push(Command::new("hostilenetworks info <pos:block_pos>"));
    event.commands.push(Command::op(
        "hostilenetworks energy <pos:block_pos> <amount:integer>",
        2,
    ));
    event.commands.push(Command::op(
        "hostilenetworks model give <model:word> <data:integer>",
        2,
    ));
    event.commands.push(Command::op(
        "hostilenetworks model held <model:word> <data:integer>",
        2,
    ));
    event.commands.push(Command::op(
        "hostilenetworks model set <pos:block_pos> <slot:integer> <model:word> <data:integer>",
        2,
    ));
    event.commands.push(Command::op(
        "hostilenetworks fab <pos:block_pos> <drop:integer>",
        2,
    ));
    event.commands.push(Command::op(
        "hostilenetworks take <pos:block_pos> <slot:integer>",
        2,
    ));
    event.commands.push(Command::op(
        "hostilenetworks put <pos:block_pos> <slot:integer>",
        2,
    ));
}

fn block_pos(argument: Option<&CommandArgumentValue>) -> Option<[i32; 3]> {
    match argument? {
        CommandArgumentValue::BlockPos(pos) => Some(*pos),
        _ => None,
    }
}

fn integer(argument: Option<&CommandArgumentValue>) -> Option<i32> {
    match argument? {
        CommandArgumentValue::Integer(value) => Some(*value),
        _ => None,
    }
}

fn word(argument: Option<&CommandArgumentValue>) -> Option<&str> {
    match argument? {
        CommandArgumentValue::String(value) => Some(value.as_str()),
        _ => None,
    }
}

fn say(host: &mut dyn ServerHost, line: String) {
    let _ = host.commands().feedback(Text::literal(line));
}

/// `/hostilenetworks` bodies. The arguments are parsed and resolved by the server's dispatcher,
/// so this only reads the machine and answers. The readout text is byte-identical to 0.1.0's so
/// the existing in-game assertions still match.
pub(crate) fn run_command(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    let invocation = event.command;
    let literals: Vec<&str> = invocation.literals.iter().map(String::as_str).collect();
    let pos = block_pos(invocation.argument("pos"));
    let level = command_level(&*host, invocation);
    let line = match literals.as_slice() {
        ["hostilenetworks", "info"] => {
            let Some((level, pos)) = level.zip(pos) else {
                say(host, "Usage: /hostilenetworks info <x> <y> <z>".to_owned());
                return Flow::Cancel;
            };
            info_line(host, level, pos).unwrap_or_else(|| "No machine at that position.".to_owned())
        }
        ["hostilenetworks", "energy"] => {
            let amount = integer(invocation.argument("amount"));
            let Some((level, pos, amount)) =
                level.zip(pos).zip(amount).map(|((l, p), a)| (l, p, a))
            else {
                say(
                    host,
                    "Usage: /hostilenetworks energy <x> <y> <z> <amount>".to_owned(),
                );
                return Flow::Cancel;
            };
            match set_energy(host, level, pos, amount) {
                Some(energy) => format!("Energy: {energy} FE"),
                None => "No machine at that position.".to_owned(),
            }
        }
        ["hostilenetworks", "model", "give"] => {
            let (found, data) = (
                word(invocation.argument("model")),
                integer(invocation.argument("data")),
            );
            let Some((found, data)) = found.and_then(model).zip(data) else {
                let carried = MODELS
                    .iter()
                    .map(|entry| entry.id)
                    .collect::<Vec<_>>()
                    .join(", ");
                say(host, format!("Unknown model; carried: {carried}"));
                return Flow::Cancel;
            };
            let Some(stack) = new_stack(&*host, DATA_MODEL_ITEM, 1)
                .and_then(|stack| with_stack_model(&stack, found.id))
                .and_then(|stack| with_stack_data(&stack, data.clamp(0, 100_000)))
            else {
                say(host, "No room for the Data Model.".to_owned());
                return Flow::Cancel;
            };
            let player = match event.source {
                CommandSource::Player { player, .. } => Some(*player),
                _ => None,
            };
            let Some(player) = player else {
                say(host, "Run this as a player.".to_owned());
                return Flow::Cancel;
            };
            match give_stack(host, player, stack) {
                None => format!("Gave a {} Data Model at {data} data", found.name),
                Some(_) => "No room for the Data Model.".to_owned(),
            }
        }
        ["hostilenetworks", "model", "held"] => {
            let (found, data) = (
                word(invocation.argument("model")),
                integer(invocation.argument("data")),
            );
            let player = match event.source {
                CommandSource::Player { player, .. } => Some(*player),
                _ => None,
            };
            let Some((found, data, player)) = found
                .and_then(model)
                .zip(data)
                .zip(player)
                .map(|((m, d), p)| (m, d, p))
            else {
                say(host, "Unknown model or data value.".to_owned());
                return Flow::Cancel;
            };
            let Some((slot, stack)) = held(&*host, player) else {
                say(host, "Hold the Data Model to retype.".to_owned());
                return Flow::Cancel;
            };
            if stack.item().id(&*host) != DATA_MODEL_ITEM {
                say(host, "Hold the Data Model to retype.".to_owned());
                return Flow::Cancel;
            }
            let typed = with_stack_model(&stack, found.id)
                .and_then(|stack| with_stack_data(&stack, data.clamp(0, 100_000)));
            let Some(typed) = typed else {
                say(host, "Could not retype the Data Model.".to_owned());
                return Flow::Cancel;
            };
            let _ = player.set_inventory_slot(host, slot, Some(typed));
            format!("Held Data Model set to {} with {data} data", found.name)
        }
        ["hostilenetworks", "model", "set"] => {
            let slot = integer(invocation.argument("slot"));
            let (found, data) = (
                word(invocation.argument("model")),
                integer(invocation.argument("data")),
            );
            let Some((level, pos, slot, found, data)) = level
                .zip(pos)
                .zip(slot)
                .zip(found.and_then(model))
                .zip(data)
                .map(|((((l, p), s), m), d)| (l, p, s, m, d))
            else {
                say(
                    host,
                    "Usage: /hostilenetworks model set <x> <y> <z> <slot> <model> <data>"
                        .to_owned(),
                );
                return Flow::Cancel;
            };
            let Ok(slot) = u32::try_from(slot) else {
                say(host, "No Data Model in that slot.".to_owned());
                return Flow::Cancel;
            };
            let typed = machine_slot(&*host, level, pos, slot)
                .and_then(|stack| with_stack_model(&stack, found.id))
                .and_then(|stack| with_stack_data(&stack, data.clamp(0, 100_000)));
            let Some(typed) = typed else {
                say(host, "No Data Model in that slot.".to_owned());
                return Flow::Cancel;
            };
            if write_machine_slot(host, level, pos, slot, Some(typed)) {
                format!("Slot {slot}: {} Data Model, {data} data", found.name)
            } else {
                "Could not write that slot.".to_owned()
            }
        }
        ["hostilenetworks", "fab"] => {
            let drop = integer(invocation.argument("drop"));
            let Some((level, pos, drop)) = level.zip(pos).zip(drop).map(|((l, p), d)| (l, p, d))
            else {
                say(
                    host,
                    "Usage: /hostilenetworks fab <x> <y> <z> <drop>".to_owned(),
                );
                return Flow::Cancel;
            };
            match select_drop(host, level, pos, drop) {
                Some(selection) => format!("Loot Fabricator selection: {selection}"),
                None => "No Loot Fabricator at that position.".to_owned(),
            }
        }
        ["hostilenetworks", "take"] => {
            let slot = integer(invocation.argument("slot"));
            let Some((level, pos, slot)) = level.zip(pos).zip(slot).map(|((l, p), s)| (l, p, s))
            else {
                say(
                    host,
                    "Usage: /hostilenetworks take <x> <y> <z> <slot>".to_owned(),
                );
                return Flow::Cancel;
            };
            let player = match event.source {
                CommandSource::Player { player, .. } => Some(*player),
                _ => None,
            };
            let Ok(slot) = u32::try_from(slot) else {
                say(host, "That slot is empty.".to_owned());
                return Flow::Cancel;
            };
            let taken = machine_slot(&*host, level, pos, slot)
                .filter(|_| write_machine_slot(host, level, pos, slot, None));
            let (Some(stack), Some(player)) = (taken, player) else {
                say(host, "That slot is empty.".to_owned());
                return Flow::Cancel;
            };
            let (item, count) = (stack.item().id(&*host), stack.count());
            match give_stack(host, player, stack) {
                None => format!("Took {item} x{count}"),
                Some(left) => {
                    let _ = write_machine_slot(host, level, pos, slot, Some(left));
                    "No room in the inventory.".to_owned()
                }
            }
        }
        ["hostilenetworks", "put"] => {
            let slot = integer(invocation.argument("slot"));
            let Some((level, pos, slot)) = level.zip(pos).zip(slot).map(|((l, p), s)| (l, p, s))
            else {
                say(
                    host,
                    "Usage: /hostilenetworks put <x> <y> <z> <slot>".to_owned(),
                );
                return Flow::Cancel;
            };
            let player = match event.source {
                CommandSource::Player { player, .. } => Some(*player),
                _ => None,
            };
            let Ok(slot) = u32::try_from(slot) else {
                say(host, "Hold the stack to put in the slot.".to_owned());
                return Flow::Cancel;
            };
            let Some((held_slot, held_stack)) = player.and_then(|player| held(&*host, player))
            else {
                say(host, "Hold the stack to put in the slot.".to_owned());
                return Flow::Cancel;
            };
            if machine_slot(&*host, level, pos, slot).is_some() {
                say(host, "That slot is not empty.".to_owned());
                return Flow::Cancel;
            }
            let (item, count) = (held_stack.item().id(&*host), held_stack.count());
            if !write_machine_slot(host, level, pos, slot, Some(held_stack)) {
                say(host, "Could not write that slot.".to_owned());
                return Flow::Cancel;
            }
            if let Some(player) = player {
                let _ = player.set_inventory_slot(host, held_slot, None);
            }
            format!("Put {item} x{count} in slot {slot}")
        }
        _ => return Flow::Continue,
    };
    say(host, line);
    Flow::Cancel
}

/// `/hostilenetworks energy`: add `amount` FE to a machine's buffer, clamped to its capacity.
fn set_energy(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    amount: i32,
) -> Option<i32> {
    let block = level.block(&*host, pos)?.id(&*host);
    let cap = data_cap_for(&block);
    if cap == 0 {
        return None;
    }
    let entity = host.block_entity(level, pos)?;
    let saved = Tag::Compound(entity.data(&*host)?);
    let energy = int(&saved, KEY_ENERGY).saturating_add(amount).clamp(0, cap);
    let mut patch = Compound::new();
    patch.insert(KEY_ENERGY.into(), Tag::Int(energy));
    entity.merge_data(host, patch).ok().map(|_| energy)
}

/// `/hostilenetworks fab`: select a drop row without the palette click.
fn select_drop(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    drop: i32,
) -> Option<i32> {
    if machine_at(&*host, level, pos) != Some(Machine::Fabricator) {
        return None;
    }
    let entity = host.block_entity(level, pos)?;
    let mut patch = Compound::new();
    patch.insert(KEY_SELECTION.into(), Tag::Int(drop));
    patch.insert(KEY_RUNTIME.into(), Tag::Int(0));
    entity.merge_data(host, patch).ok()?;
    Some(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Tier;

    #[test]
    fn the_chamber_declares_four_slots_and_its_menu() {
        let container = chamber_container();
        assert_eq!(
            container.slots, CHAMBER_SLOTS,
            "model, input, base drop, prediction"
        );
        assert_eq!(
            container.inputs,
            vec![0, 1],
            "only the model and the input are inputs"
        );
        assert_eq!(container.outputs, vec![2, 3]);
        assert_eq!(
            container.faces,
            std::array::from_fn(|_| vec![0, 1, 2, 3]),
            "automation reaches every slot from every face"
        );
        assert_eq!(container.menu.as_deref(), Some(CHAMBER_MENU));
    }

    #[test]
    fn the_fabricator_declares_seventeen_slots_and_its_menu() {
        let container = fabricator_container();
        assert_eq!(
            container.slots, FAB_SLOTS,
            "prediction, then the 4x4 outputs"
        );
        assert_eq!(container.inputs, vec![0], "a hopper never fills an output");
        assert_eq!(container.outputs, (1..FAB_SLOTS).collect::<Vec<_>>());
        assert_eq!(
            container.faces,
            std::array::from_fn(|_| (0..FAB_SLOTS).collect::<Vec<_>>())
        );
        assert_eq!(container.menu.as_deref(), Some(FAB_MENU));
    }

    #[test]
    fn each_machine_declares_an_insert_only_buffer_of_its_own_capacity() {
        let chamber = chamber_battery();
        assert_eq!(chamber.capacity, SIM_POWER_CAP);
        assert_eq!(chamber.max_insert, SIM_POWER_CAP);
        assert_eq!(chamber.max_extract, 0, "upstream's setMaxExtract(0)");
        assert_eq!(chamber.insert_faces, [true; 6]);
        assert_eq!(chamber.extract_faces, [false; 6]);

        let fabricator = fabricator_battery();
        assert_eq!(fabricator.capacity, FAB_POWER_CAP);
        assert_eq!(fabricator.max_insert, FAB_POWER_CAP);
        assert_eq!(fabricator.max_extract, 0);
    }

    #[test]
    fn the_chamber_screen_is_upstream_s_panel() {
        let screen = chamber_screen();
        assert_eq!([screen.width, screen.height], [176.0, 166.0]);
        assert_eq!(
            screen.slots,
            vec![
                Some([26.0, 20.0]),
                Some([62.0, 20.0]),
                Some([116.0, 20.0]),
                Some([152.0, 20.0]),
            ]
        );
        assert_eq!([screen.inventory_x, screen.inventory_y], [8.0, 84.0]);
        assert!(!screen.own_labels && !screen.own_background);
    }

    #[test]
    fn the_fabricator_screen_lays_its_prediction_then_four_by_four() {
        let screen = fabricator_screen();
        assert_eq!([screen.width, screen.height], [176.0, 186.0]);
        assert_eq!(screen.slots.len(), FAB_SLOTS);
        assert_eq!(screen.slots[0], Some([62.0, 20.0]));
        assert_eq!(screen.slots[1], Some([100.0, 20.0]));
        assert_eq!(screen.slots[4], Some([154.0, 20.0]));
        assert_eq!(screen.slots[5], Some([100.0, 38.0]));
        assert_eq!(screen.slots[16], Some([154.0, 74.0]));
        assert_eq!([screen.inventory_x, screen.inventory_y], [8.0, 104.0]);
    }

    #[test]
    fn the_saved_ints_read_back_and_the_enums_round_trip() {
        let mut saved = Tag::Compound(Compound::new());
        assert_eq!(int(&saved, KEY_ENERGY), 0, "an absent key reads as zero");
        set_int(&mut saved, KEY_ENERGY, 1_234_567);
        assert_eq!(int(&saved, KEY_ENERGY), 1_234_567);
        assert_eq!(saved.get(KEY_ENERGY), Some(&Tag::Int(1_234_567)));
        assert_eq!(
            int_or(&saved, KEY_SELECTION, -1),
            -1,
            "the fabricator's sentinel"
        );

        for fail in [
            FailState::None,
            FailState::Output,
            FailState::Energy,
            FailState::Input,
            FailState::Model,
            FailState::Faulty,
            FailState::EnergyMidCycle,
            FailState::Redstone,
        ] {
            assert_eq!(fail_of(fail_index(fail)), fail, "{fail:?} must round-trip");
        }
        assert_eq!(
            fail_of(99),
            FailState::None,
            "an unknown index is no failure"
        );
        for mode in [Mode::Inference, Mode::Training] {
            assert_eq!(mode_of(mode_index(mode)), mode, "{mode:?} must round-trip");
        }
        assert_eq!(mode_of(2), Mode::Inference, "an unknown mode is inference");
    }

    #[test]
    fn the_three_components_are_declared_with_upstream_s_names() {
        assert_eq!(
            component_ids(),
            [DATA_MODEL_COMPONENT, DATA_COMPONENT, ITERATIONS_COMPONENT]
        );
        assert_eq!(component_ids()[0], "hostilenetworks:data_model");
    }

    #[test]
    fn the_chamber_readout_is_the_line_the_e2e_asserts() {
        let sight = ChamberSight {
            energy: 1_969_408,
            runtime: 0,
            predictions: 1,
            mode: Mode::Inference,
            fail: FailState::Input,
            model: "hostilenetworks:zombie".to_owned(),
            tier: tier_for(1254).name(),
            data: 1254,
            slots: vec![
                ("hostilenetworks:data_model".to_owned(), 1),
                ("hostilenetworks:overworld_prediction".to_owned(), 1),
                ("hostilenetworks:prediction".to_owned(), 1),
            ],
        };
        assert_eq!(
            sight.line(),
            "Simulation Chamber: 1969408 FE, runtime 0, predictions 1, mode inference, fail input, \
             model hostilenetworks:zombie, tier self_aware, data 1254, slots \
             [hostilenetworks:data_model x1, hostilenetworks:overworld_prediction x1, \
             hostilenetworks:prediction x1]"
        );
        assert_eq!(Tier::SelfAware.name(), "self_aware");
    }

    #[test]
    fn the_chamber_readout_with_no_model_is_the_line_the_e2e_asserts() {
        let sight = ChamberSight {
            energy: 0,
            runtime: 0,
            predictions: 0,
            mode: Mode::Inference,
            fail: FailState::Model,
            model: "-".to_owned(),
            tier: tier_for(0).name(),
            data: 0,
            slots: Vec::new(),
        };
        assert_eq!(
            sight.line(),
            "Simulation Chamber: 0 FE, runtime 0, predictions 0, mode inference, fail model, \
             model -, tier faulty, data 0, slots []"
        );
    }

    #[test]
    fn the_fabricator_readout_is_the_line_the_e2e_asserts() {
        let sight = FabSight {
            energy: 972_096,
            runtime: 0,
            selection: -1,
            fail: FailState::Model,
            slots: vec![
                ("minecraft:iron_ingot".to_owned(), 8),
                ("hostilenetworks:prediction_matrix".to_owned(), 1),
            ],
        };
        assert_eq!(
            sight.line(),
            "Loot Fabricator: 972096 FE, runtime 0, selection -1, fail model, slots \
             [minecraft:iron_ingot x8, hostilenetworks:prediction_matrix x1]"
        );
    }
}
