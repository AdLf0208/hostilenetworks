// Game-side glue for the Hostile Neural Networks slice: the two machines'
// block entities, the item components a Data Model and a Prediction carry,
// their menus' screens and the `/hostilenetworks` harness commands.
//
// Injected into `crate::game` as `hostilenetworks_state`; it needs the game, so
// it is not part of the standalone library (only `runtime.rs` is).
//
// The chamber's and the fabricator's whole state lives in the block entity's
// saved compound (`energy`, `runtime`, `pred`, `fail`, `mode`, `model`,
// `selection`, `selModel`) and its item slots, so a save and a load keep it, as
// upstream's `saveAdditional` does. The energy amount is the int `energy` the
// pin's energy buffer reads (`native_block_entity_energy`, API 142).
//
// A tick reads the compound, asks the pure module for a decision
// (`runtime::chamber_action`/`fab_action`) and applies it: that keeps every
// gameplay rule in `runtime.rs` under its tests, and leaves the glue to move
// items and write fields.

use horizon_client::containers::{BlockEntity, ModBlockEntity, ModContainer, ModEnergyBuffer};
use horizon_core::random::RandomSource;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};

use crate::game::Game;
use crate::game::hostilenetworks::{
    ChamberAction, ChamberView, DATA_COMPONENT, DATA_MODEL_COMPONENT, DATA_MODEL_ITEM, FAB_POWER_CAP,
    FAB_RUNTIME, FabAction, FabView, FailState, ITERATIONS_COMPONENT, Item, MODELS, Mode,
    PREDICTION_ITEM, SIM_POWER_CAP, chamber_action, data_cap_for, fab_action, fab_drop, is_chamber,
    is_fabricator, model, tier_for,
};
use crate::game::native_hooks::NativeScreen;
use crate::game::native_server::{NativeCommand, NativeServerPlayer};

/// The saved keys, named as upstream's `saveAdditional` names them.
const KEY_ENERGY: &str = "energy";
const KEY_RUNTIME: &str = "runtime";
const KEY_PRED: &str = "pred";
const KEY_FAIL: &str = "fail";
const KEY_MODE: &str = "mode";
const KEY_MODEL: &str = "model";
const KEY_SELECTION: &str = "selection";
const KEY_SEL_MODEL: &str = "selModel";

/// The chamber's menu type (`data/hostilenetworks/horizon/menu/sim_chamber.json`).
pub const CHAMBER_MENU: &str = "hostilenetworks:sim_chamber";
/// The fabricator's menu type.
pub const FAB_MENU: &str = "hostilenetworks:loot_fabricator";

/// The chamber's slots: model, input, base drop, prediction.
const CHAMBER_SLOTS: usize = 4;
/// The fabricator's slots: prediction, then its 4x4 outputs.
const FAB_SLOTS: usize = 17;

/// A stack's item id.
fn stack_id(stack: &ItemStack) -> &str {
    stack.item.as_ref()
}

/// A stack of `item` (no components).
fn stack_of(item: &str, count: u32) -> ItemStack {
    ItemStack {
        item: item.into(),
        count,
        damage: 0,
        extra: None,
    }
}

/// An int field of a saved compound, `0` when it is absent or another type.
fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

/// An int field, or `default` when it is absent.
fn int_or(nbt: &Compound, key: &str, default: i32) -> i32 {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::Int(value)) => *value,
        _ => default,
    }
}

fn set_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(JavaString::from(key), Tag::Int(value));
}

/// A string field of a saved compound.
fn string(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&JavaString::from(key)) {
        Some(Tag::String(value)) => Some(value.to_string_lossy()),
        _ => None,
    }
}

fn set_string(nbt: &mut Compound, key: &str, value: &str) {
    nbt.insert(
        JavaString::from(key),
        Tag::String(JavaString::from(value)),
    );
}

/// The model a stack's `hostilenetworks:data_model` component names.
fn stack_model(game: &Game<'_>, stack: &ItemStack) -> Option<&'static crate::game::hostilenetworks::Model> {
    match game.native_component(stack, DATA_MODEL_COMPONENT).ok().flatten()? {
        Tag::String(id) => model(&id.to_string_lossy()),
        _ => None,
    }
}

/// The data a model stack carries (`hostilenetworks:data`).
fn stack_data(game: &Game<'_>, stack: &ItemStack) -> i32 {
    match game.native_component(stack, DATA_COMPONENT).ok().flatten() {
        Some(Tag::Int(data)) => *data,
        _ => 0,
    }
}

/// The iterations a model stack carries (`hostilenetworks:iterations`).
fn stack_iterations(game: &Game<'_>, stack: &ItemStack) -> i32 {
    match game.native_component(stack, ITERATIONS_COMPONENT).ok().flatten() {
        Some(Tag::Int(iters)) => *iters,
        _ => 0,
    }
}

/// Writes the model id (`hostilenetworks:data_model`) onto a stack.
fn set_stack_model(game: &Game<'_>, stack: &mut ItemStack, model_id: &str) -> bool {
    game.native_set_component(
        stack,
        DATA_MODEL_COMPONENT,
        Tag::String(JavaString::from(model_id)),
    )
    .is_ok()
}

fn set_stack_data(game: &Game<'_>, stack: &mut ItemStack, data: i32) -> bool {
    game.native_set_component(stack, DATA_COMPONENT, Tag::Int(data)).is_ok()
}

fn set_stack_iterations(game: &Game<'_>, stack: &mut ItemStack, iterations: i32) -> bool {
    game.native_set_component(stack, ITERATIONS_COMPONENT, Tag::Int(iterations))
        .is_ok()
}

/// The mode a saved int names (`0` inference, `1` training).
fn mode_of(value: i32) -> Mode {
    if value == 1 { Mode::Training } else { Mode::Inference }
}

/// The fail state a saved int names.
fn fail_of(value: i32) -> FailState {
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

fn fail_index(fail: FailState) -> i32 {
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

/// Whether `slot` can take `id` x `count` (`canStack`): empty, or the same item
/// with room for the whole stack.
fn slot_takes(slot: Option<&ItemStack>, id: &str, count: u32, max: u32) -> bool {
    match slot {
        None => true,
        Some(stack) => stack_id(stack) == id && stack.count + count <= max,
    }
}

/// Whether the chamber's prediction output can take `model`'s prediction: an
/// empty slot, or the same model's prediction with room for one more.
fn prediction_slot_takes(
    game: &Game<'_>,
    slot: Option<&ItemStack>,
    model_id: &str,
) -> bool {
    match slot {
        None => true,
        Some(stack) => {
            stack_id(stack) == PREDICTION_ITEM
                && stack.count < 64
                && stack_model(game, stack).is_some_and(|held| held.id == model_id)
        }
    }
}

/// Adds `count` of `item` (and, for a prediction, the model component) to a
/// slot that `slot_takes` accepted.
fn add_to_slot(
    game: &Game<'_>,
    slot: &mut Option<ItemStack>,
    item: Item,
    model_id: Option<&str>,
) {
    match slot {
        Some(stack) => stack.count += item.count,
        None => {
            let mut stack = stack_of(item.id, item.count);
            if let Some(model_id) = model_id {
                let _ = set_stack_model(game, &mut stack, model_id);
            }
            *slot = Some(stack);
        }
    }
}

/// The block id at `pos` in the level in place.
fn block_at(game: &Game<'_>, pos: [i32; 3]) -> String {
    game.res
        .facts()
        .get(game.world.block(pos[0], pos[1], pos[2]))
        .name
        .to_string()
}

// ---------------------------------------------------------------------------
// Declaration hooks
// ---------------------------------------------------------------------------

/// Before content loads: register the three components (`native_component_types`,
/// API 65). A content item sets one with a JSON value, and the helpers below
/// read and write them.
#[allow(private_interfaces)]
pub fn register_component_types(types: &mut Vec<String>) {
    for id in [DATA_MODEL_COMPONENT, DATA_COMPONENT, ITERATIONS_COMPONENT] {
        types.push(id.to_owned());
    }
}

/// Give each machine a block entity (`native_block_entity`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if is_chamber(block) || is_fabricator(block) {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// The machines tick (`native_block_entity_ticks`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
    if is_chamber(block) || is_fabricator(block) {
        *ticks = true;
    }
}

/// The chamber's 4 slots and the fabricator's 17, with their menu types
/// (`native_block_entity_container`, API 16/17). Hoppers reach every slot; the
/// result slots are not inputs, so a hopper never fills an output.
#[allow(private_interfaces)]
pub fn on_container(block: &str, container: &mut Option<ModContainer>) {
    let all: [Vec<usize>; 6] = std::array::from_fn(|_| (0..FAB_SLOTS).collect());
    if is_chamber(block) {
        *container = Some(ModContainer {
            slots: CHAMBER_SLOTS,
            faces: std::array::from_fn(|_| vec![0, 1, 2, 3]),
            inputs: vec![0, 1],
            outputs: vec![2, 3],
            menu: Some(CHAMBER_MENU.to_owned()),
        });
    } else if is_fabricator(block) {
        *container = Some(ModContainer {
            slots: FAB_SLOTS,
            faces: all,
            inputs: vec![0],
            outputs: (1..FAB_SLOTS).collect(),
            menu: Some(FAB_MENU.to_owned()),
        });
    }
}

/// Each machine's FE buffer (`native_block_entity_energy`, API 142): upstream's
/// `ModifiableEnergyStorage(cap, cap)` takes energy and gives none back
/// (`setMaxExtract(0)`), so every side fills and none drains.
#[allow(private_interfaces)]
pub fn on_energy(block: &str, energy: &mut Option<ModEnergyBuffer>) {
    let capacity = if is_chamber(block) {
        SIM_POWER_CAP
    } else if is_fabricator(block) {
        FAB_POWER_CAP
    } else {
        return;
    };
    *energy = Some(ModEnergyBuffer {
        capacity,
        max_insert: capacity,
        max_extract: 0,
        insert_faces: [true; 6],
        extract_faces: [false; 6],
    });
}

// ---------------------------------------------------------------------------
// The tick
// ---------------------------------------------------------------------------

/// One tick of either machine (`native_block_entity_tick`, API 17).
#[allow(private_interfaces)]
pub fn on_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if is_chamber(&entity.id) {
        chamber_tick(game, pos, entity);
    } else if is_fabricator(&entity.id) {
        fabricator_tick(game, pos, entity);
    }
}

/// `SimChamberTileEntity.serverTick`, applied.
fn chamber_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    entity.items.resize(CHAMBER_SLOTS, None);
    let model_stack = entity.items[0].clone();
    let model = model_stack.as_ref().and_then(|stack| stack_model(game, stack));
    let data = model_stack.as_ref().map_or(0, |stack| stack_data(game, stack));
    // A different model in the slot restarts the run, as upstream's
    // `oldModel != this.currentModel` does.
    let model_id = model.map(|model| model.id);
    if string(&entity.nbt, KEY_MODEL).as_deref() != model_id {
        set_int(&mut entity.nbt, KEY_RUNTIME, 0);
        set_string(&mut entity.nbt, KEY_MODEL, model_id.unwrap_or("null"));
    }
    let energy = int(&entity.nbt, KEY_ENERGY).min(SIM_POWER_CAP);
    let runtime = int(&entity.nbt, KEY_RUNTIME);
    let prediction_success = int(&entity.nbt, KEY_PRED);
    let mode = mode_of(int(&entity.nbt, KEY_MODE));
    let has_input = model.is_some_and(|model| {
        entity.items[1]
            .as_ref()
            .is_some_and(|stack| stack_id(stack) == model.input)
    });
    let output_room = model.is_some_and(|model| {
        slot_takes(entity.items[2].as_ref(), model.base_drop.id, 1, 64)
            && prediction_slot_takes(game, entity.items[3].as_ref(), model.id)
    });
    let sample = game.world.random.next_float();
    let view = ChamberView {
        model,
        data,
        has_input,
        output_room,
        // Redstone control (upstream's `RedstoneState::IGNORED` default) is a
        // gap in this slice: the chamber always runs when it can.
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
                set_int(&mut entity.nbt, KEY_RUNTIME, 0);
            }
        }
        ChamberAction::Start {
            runtime,
            predictions,
        } => {
            set_int(&mut entity.nbt, KEY_RUNTIME, runtime);
            set_int(&mut entity.nbt, KEY_PRED, predictions);
            if let Some(input) = entity.items[1].as_mut() {
                input.count = input.count.saturating_sub(1);
                if input.count == 0 {
                    entity.items[1] = None;
                }
            }
        }
        ChamberAction::Step { energy_cost } => {
            set_int(&mut entity.nbt, KEY_RUNTIME, runtime - 1);
            set_int(&mut entity.nbt, KEY_ENERGY, (energy - energy_cost).max(0));
        }
        ChamberAction::Finish {
            base_drop,
            predictions,
            iterations,
        } => {
            add_to_slot(game, &mut entity.items[2], base_drop, None);
            if predictions > 0
                && let Some(model) = model
            {
                add_to_slot(
                    game,
                    &mut entity.items[3],
                    Item::new(PREDICTION_ITEM, predictions),
                    Some(model.id),
                );
            }
            set_int(&mut entity.nbt, KEY_RUNTIME, 0);
            if let Some(stack) = entity.items[0].as_mut() {
                let count = stack_iterations(game, stack) + iterations as i32;
                let _ = set_stack_iterations(game, stack, count);
            }
            game.native_block_entity_sync(pos);
        }
    }
    set_int(&mut entity.nbt, KEY_FAIL, fail_index(fail));
    let stored = int(&entity.nbt, KEY_ENERGY);
    let data_slots = vec![
        int(&entity.nbt, KEY_RUNTIME),
        int(&entity.nbt, KEY_PRED),
        fail_index(fail),
        int(&entity.nbt, KEY_MODE),
        stored & 0xffff,
        stored >> 16,
    ];
    if entity.data != data_slots {
        entity.data = data_slots;
    }
}

/// `LootFabTileEntity.serverTick`, applied. The selection is kept per model
/// (`selModel`); upstream keeps a map, which a single carried model makes a
/// pair of fields here.
fn fabricator_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    entity.items.resize(FAB_SLOTS, None);
    let model = entity.items[0]
        .as_ref()
        .and_then(|stack| stack_model(game, stack));
    let model_id = model.map(|model| model.id);
    if string(&entity.nbt, KEY_SEL_MODEL).as_deref() != model_id {
        set_int(&mut entity.nbt, KEY_SELECTION, -1);
        set_string(&mut entity.nbt, KEY_SEL_MODEL, model_id.unwrap_or("null"));
        set_int(&mut entity.nbt, KEY_RUNTIME, 0);
    }
    let selection = int_or(&entity.nbt, KEY_SELECTION, -1);
    let energy = int(&entity.nbt, KEY_ENERGY).min(FAB_POWER_CAP);
    let runtime = int(&entity.nbt, KEY_RUNTIME);
    let drop = model.and_then(|model| fab_drop(model, selection));
    let output_room = drop.is_some_and(|drop| {
        (1..FAB_SLOTS).any(|slot| slot_takes(entity.items[slot].as_ref(), drop.id, drop.count, 64))
    });
    let view = FabView {
        model,
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
            set_int(&mut entity.nbt, KEY_ENERGY, (energy - energy_cost).max(0));
            set_int(&mut entity.nbt, KEY_RUNTIME, runtime + 1);
        }
        FabAction::Produce { drop } => {
            if let Some(slot) = (1..FAB_SLOTS)
                .find(|slot| slot_takes(entity.items[*slot].as_ref(), drop.id, drop.count, 64))
            {
                add_to_slot(game, &mut entity.items[slot], drop, None);
            }
            if let Some(input) = entity.items[0].as_mut() {
                input.count = input.count.saturating_sub(1);
                if input.count == 0 {
                    entity.items[0] = None;
                }
            }
            set_int(&mut entity.nbt, KEY_RUNTIME, 0);
            game.native_block_entity_sync(pos);
        }
    }
    set_int(&mut entity.nbt, KEY_FAIL, fail_index(fail));
    let model_index = model
        .and_then(|model| MODELS.iter().position(|entry| entry.id == model.id))
        .map_or(-1, |index| index as i32);
    let stored = int(&entity.nbt, KEY_ENERGY);
    let data_slots = vec![
        int(&entity.nbt, KEY_RUNTIME),
        int_or(&entity.nbt, KEY_SELECTION, -1),
        model_index,
        stored & 0xffff,
        stored >> 16,
    ];
    if entity.data != data_slots {
        entity.data = data_slots;
    }
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------

/// Where the chamber's four slots sit inside its panel.
const CHAMBER_SLOTS_AT: [[f32; 2]; 4] = [[26.0, 20.0], [62.0, 20.0], [116.0, 20.0], [152.0, 20.0]];
/// The fabricator's prediction slot, then its 4x4 outputs.
fn fab_slot_at(slot: usize) -> [f32; 2] {
    if slot == 0 {
        return [62.0, 20.0];
    }
    let cell = slot - 1;
    let column = cell % 4;
    let row = cell / 4;
    [100.0 + column as f32 * 18.0, 20.0 + row as f32 * 18.0]
}

/// The screen layout of each machine (`native_screen`, API 19).
#[allow(private_interfaces)]
pub fn on_screen(block: &str, screen: &mut Option<NativeScreen>) {
    if is_chamber(block) {
        let mut slots: Vec<Option<[f32; 2]>> =
            CHAMBER_SLOTS_AT.iter().map(|at| Some(*at)).collect();
        slots.resize(CHAMBER_SLOTS, None);
        *screen = Some(NativeScreen {
            width: 176.0,
            height: 166.0,
            slots,
            inventory_y: 84.0,
            inventory_x: 8.0,
            own_labels: false,
            own_background: false,
        });
    } else if is_fabricator(block) {
        let mut slots: Vec<Option<[f32; 2]>> = (0..FAB_SLOTS).map(|slot| Some(fab_slot_at(slot))).collect();
        slots.resize(FAB_SLOTS, None);
        *screen = Some(NativeScreen {
            width: 176.0,
            height: 186.0,
            slots,
            inventory_y: 104.0,
            inventory_x: 8.0,
            own_labels: false,
            own_background: false,
        });
    }
}

/// The energy split back out of the menu's two 16-bit data slots.
fn energy_of(data: &[i32], low: usize, high: usize) -> i32 {
    let low = data.get(low).copied().unwrap_or(0) as u16 as u32;
    let high = data.get(high).copied().unwrap_or(0) as u16 as u32;
    (high << 16 | low) as i32
}

/// Draws the machines' readouts (`native_screen_draw`, API 19). The text is
/// plain GUI text at fixed spots; a full custom background is a gap.
#[allow(private_interfaces)]
pub fn on_screen_draw(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut HudBatch,
    origin: [f32; 2],
) {
    let mut text = |game: &Game<'_>, line: &str, row: f32, colour: u32| {
        game.native_text(batch, line, [origin[0] + 8.0, origin[1] + row], colour, false);
    };
    if is_chamber(block) {
        let runtime = data.first().copied().unwrap_or(0);
        let predictions = data.get(1).copied().unwrap_or(0);
        let fail = fail_of(data.get(2).copied().unwrap_or(0));
        let mode = mode_of(data.get(3).copied().unwrap_or(0));
        let energy = energy_of(data, 4, 5);
        text(
            game,
            &format!("{energy} / {SIM_POWER_CAP} FE"),
            44.0,
            0x404040,
        );
        let run = mode.runtime();
        text(
            game,
            &format!("{}/{} ticks ({})", run - runtime, run, mode.name()),
            54.0,
            0x404040,
        );
        text(
            game,
            &format!("Predictions this run: {predictions}"),
            64.0,
            0x404040,
        );
        if fail != FailState::None {
            text(game, fail.name(), 74.0, 0x8b0000);
        }
    } else if is_fabricator(block) {
        let runtime = data.first().copied().unwrap_or(0);
        let selection = data.get(1).copied().unwrap_or(-1);
        let model = usize::try_from(data.get(2).copied().unwrap_or(-1))
            .ok()
            .and_then(|index| MODELS.get(index));
        let energy = energy_of(data, 3, 4);
        text(
            game,
            &format!("{energy} / {FAB_POWER_CAP} FE"),
            44.0,
            0x404040,
        );
        text(
            game,
            &format!("{runtime}/{FAB_RUNTIME} ticks"),
            54.0,
            0x404040,
        );
        if let Some(model) = model {
            for (index, (id, count)) in model.fab_drops.iter().enumerate() {
                let colour = if index as i32 == selection {
                    0x00a000
                } else {
                    0x404040
                };
                let short = id.trim_start_matches("minecraft:");
                text(
                    game,
                    &format!("{}. {short} x{count}", index + 1),
                    66.0 + index as f32 * 10.0,
                    colour,
                );
            }
        } else {
            text(game, "Insert a Prediction", 66.0, 0x8b0000);
        }
    }
}

/// The palette's first row and its line height, shared with the click test.
const PALETTE_TOP: f32 = 66.0;
const PALETTE_LINE: f32 = 10.0;
/// The rows the palette always offers: the carried model's table length. The
/// click is a coordinate-to-row map, so it does not depend on the menu's data
/// slots having reached this client (the draw does, and shows the names).
const PALETTE_ROWS: usize = 4;

/// A press on a machine's own screen names the menu button to send
/// (`native_screen_click`, API 20). The fabricator's palette row `i` sends `i`;
/// the chamber's mode line sends [`CHAMBER_MODE_BUTTON`].
#[allow(private_interfaces)]
pub fn on_screen_click(
    block: &str,
    data: &[i32],
    at: [f32; 2],
    _right: bool,
    button: &mut Option<i32>,
) {
    if !is_fabricator(block) && !is_chamber(block) {
        return;
    }
    if at[0] < 8.0 || at[0] > 96.0 {
        return;
    }
    if is_chamber(block) {
        if (44.0..54.0).contains(&at[1]) && at[0] < 60.0 {
            *button = Some(CHAMBER_MODE_BUTTON);
        }
        return;
    }
    let rows = usize::try_from(data.get(2).copied().unwrap_or(-1))
        .ok()
        .and_then(|index| MODELS.get(index))
        .map_or(PALETTE_ROWS, |model| model.fab_drops.len());
    for index in 0..rows {
        let top = PALETTE_TOP + index as f32 * PALETTE_LINE;
        if (top..top + PALETTE_LINE).contains(&at[1]) {
            *button = Some(index as i32);
            return;
        }
    }
}

/// The menu button id that switches the chamber's mode.
pub const CHAMBER_MODE_BUTTON: i32 = 100;

/// The server applies a screen click (`native_menu_button`, API 20).
#[allow(private_interfaces)]
pub fn on_menu_button(
    _game: &mut Game<'_>,
    _pos: [i32; 3],
    entity: &mut ModBlockEntity,
    id: i32,
) {
    if is_chamber(&entity.id) && id == CHAMBER_MODE_BUTTON {
        let mode = mode_of(int(&entity.nbt, KEY_MODE)).next();
        set_int(
            &mut entity.nbt,
            KEY_MODE,
            if mode == Mode::Training { 1 } else { 0 },
        );
        set_int(&mut entity.nbt, KEY_RUNTIME, 0);
    } else if is_fabricator(&entity.id) {
        let Some(model) = entity.items[0]
            .as_ref()
            .and_then(|stack| model_for(entity, stack))
        else {
            return;
        };
        if fab_drop(model, id).is_none() {
            return;
        }
        set_int(&mut entity.nbt, KEY_SELECTION, id);
        set_int(&mut entity.nbt, KEY_RUNTIME, 0);
    }
}

/// The model a stack names, for a hook that has no game handle: the component
/// is a plain string, so it is read from the stack's patch when it has one and
/// from the model's own item default otherwise. The tick (which has the game)
/// is the authority; this keeps a menu click from needing one.
fn model_for<'a>(
    _entity: &'a ModBlockEntity,
    stack: &'a ItemStack,
) -> Option<&'static crate::game::hostilenetworks::Model> {
    let data = stack.data()?;
    let tag = data.other(DATA_MODEL_COMPONENT)?;
    match tag {
        Tag::String(id) => model(&id.to_string_lossy()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Harness commands
// ---------------------------------------------------------------------------

/// `/hostilenetworks ...` (`register_commands`, API 8). `info` reads a machine,
/// `energy` charges it, `model` gives or retypes a Data Model, `fab` selects a
/// drop. They exist because a headless harness cannot work a screen's slots or
/// an energy network, and are recorded as a harness addition, not gameplay.
#[allow(private_interfaces)]
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new("hostilenetworks info <pos:block_pos>"));
    commands.push(NativeCommand::op(
        "hostilenetworks energy <pos:block_pos> <amount:integer>",
        2,
    ));
    commands.push(NativeCommand::new(
        "hostilenetworks model give <model:word> <data:integer>",
    ));
    commands.push(NativeCommand::new(
        "hostilenetworks model held <model:word> <data:integer>",
    ));
    commands.push(NativeCommand::op(
        "hostilenetworks fab <pos:block_pos> <drop:integer>",
        2,
    ));
    commands.push(NativeCommand::op(
        "hostilenetworks model set <pos:block_pos> <slot:integer> <model:word> <data:integer>",
        2,
    ));
    commands.push(NativeCommand::op(
        "hostilenetworks take <pos:block_pos> <slot:integer>",
        2,
    ));
    commands.push(NativeCommand::op(
        "hostilenetworks put <pos:block_pos> <slot:integer>",
        2,
    ));
}

/// The stack a machine's slot holds.
fn machine_slot(game: &Game<'_>, pos: [i32; 3], slot: i32) -> Option<ItemStack> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(entity)) => entity
            .items
            .get(usize::try_from(slot).ok()?)
            .cloned()
            .flatten(),
        _ => None,
    }
}

fn parse_pos(words: &[&str]) -> Option<[i32; 3]> {
    Some([
        words.first()?.parse().ok()?,
        words.get(1)?.parse().ok()?,
        words.get(2)?.parse().ok()?,
    ])
}

/// The chamber's or the fabricator's readout, as `/hostilenetworks info` says
/// it.
fn machine_info(game: &Game<'_>, pos: [i32; 3]) -> Option<String> {
    let entity = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(entity)) => entity,
        _ => return None,
    };
    let energy = int(&entity.nbt, KEY_ENERGY);
    let runtime = int(&entity.nbt, KEY_RUNTIME);
    let fail = fail_of(int(&entity.nbt, KEY_FAIL));
    let slots: Vec<String> = entity
        .items
        .iter()
        .filter_map(|slot| slot.as_ref())
        .map(|stack| format!("{} x{}", stack_id(stack), stack.count))
        .collect();
    if is_chamber(&entity.id) {
        let model = string(&entity.nbt, KEY_MODEL).unwrap_or_else(|| "-".to_owned());
        let data = entity
            .items
            .first()
            .and_then(Option::as_ref)
            .map_or(0, |stack| stack_data(game, stack));
        return Some(format!(
            "Simulation Chamber: {energy} FE, runtime {runtime}, predictions {}, mode {}, fail {}, model {model}, tier {}, data {data}, slots [{}]",
            int(&entity.nbt, KEY_PRED),
            mode_of(int(&entity.nbt, KEY_MODE)).name(),
            fail.name(),
            tier_for(data).name(),
            slots.join(", ")
        ));
    }
    if is_fabricator(&entity.id) {
        return Some(format!(
            "Loot Fabricator: {energy} FE, runtime {runtime}, selection {}, fail {}, slots [{}]",
            int_or(&entity.nbt, KEY_SELECTION, -1),
            fail.name(),
            slots.join(", ")
        ));
    }
    None
}

/// Runs `/hostilenetworks` (`perform_command`, API 8).
#[allow(private_interfaces)]
pub fn on_perform_command(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        ["hostilenetworks", "info", x, y, z] => {
            *ran = true;
            let Some(pos) = parse_pos(&[*x, *y, *z]) else {
                game.native_feedback(Text::literal("Usage: /hostilenetworks info <x> <y> <z>"));
                return;
            };
            match machine_info(game, pos) {
                Some(line) => game.native_feedback(Text::literal(line)),
                None => game.native_feedback(Text::literal("No machine at that position.")),
            }
        }
        ["hostilenetworks", "energy", x, y, z, amount] => {
            *ran = true;
            let (Some(pos), Ok(amount)) = (parse_pos(&[*x, *y, *z]), amount.parse::<i32>()) else {
                game.native_feedback(Text::literal(
                    "Usage: /hostilenetworks energy <x> <y> <z> <amount>",
                ));
                return;
            };
            let block = block_at(game, pos);
            let cap = data_cap_for(&block);
            if cap == 0 {
                game.native_feedback(Text::literal("No machine at that position."));
                return;
            }
            let stored = game
                .native_with_mod_block_entity(pos, |entity| {
                    let energy = int(&entity.nbt, KEY_ENERGY).saturating_add(amount).clamp(0, cap);
                    set_int(&mut entity.nbt, KEY_ENERGY, energy);
                    energy
                });
            match stored {
                Some(energy) => game.native_feedback(Text::literal(format!("Energy: {energy} FE"))),
                None => game.native_feedback(Text::literal("No machine at that position.")),
            }
        }
        ["hostilenetworks", "model", "give", id, data] => {
            *ran = true;
            let (Some(model), Ok(data)) = (model(id), data.parse::<i32>()) else {
                game.native_feedback(Text::literal(format!(
                    "Unknown model {id}; carried: {}",
                    MODELS.iter().map(|m| m.id).collect::<Vec<_>>().join(", ")
                )));
                return;
            };
            let mut stack = stack_of(DATA_MODEL_ITEM, 1);
            let _ = set_stack_model(game, &mut stack, model.id);
            let _ = set_stack_data(game, &mut stack, data.clamp(0, 100_000));
            let left = game.native_give_stack(player.uuid, stack);
            match left {
                None => game.native_feedback(Text::literal(format!(
                    "Gave a {} Data Model at {} data",
                    model.name, data
                ))),
                Some(_) => game.native_feedback(Text::literal("No room for the Data Model.")),
            }
        }
        ["hostilenetworks", "model", "held", id, data] => {
            *ran = true;
            let (Some(model), Ok(data)) = (model(id), data.parse::<i32>()) else {
                game.native_feedback(Text::literal("Unknown model or data value."));
                return;
            };
            let Some(mut stack) = game.held().cloned() else {
                game.native_feedback(Text::literal("Hold the Data Model to retype."));
                return;
            };
            if stack_id(&stack) != DATA_MODEL_ITEM {
                game.native_feedback(Text::literal("Hold the Data Model to retype."));
                return;
            }
            let _ = set_stack_model(game, &mut stack, model.id);
            let _ = set_stack_data(game, &mut stack, data.clamp(0, 100_000));
            let slot = game.held_slot();
            *game.held_mut() = Some(stack);
            if game.lan_acting.is_none() {
                game.hand_stack_replaced(slot);
            }
            game.native_feedback(Text::literal(format!(
                "Held Data Model set to {} with {data} data",
                model.name
            )));
        }
        ["hostilenetworks", "fab", x, y, z, drop] => {
            *ran = true;
            let (Some(pos), Ok(drop)) = (parse_pos(&[*x, *y, *z]), drop.parse::<i32>()) else {
                game.native_feedback(Text::literal(
                    "Usage: /hostilenetworks fab <x> <y> <z> <drop>",
                ));
                return;
            };
            let answer = game
                .native_with_mod_block_entity(pos, |entity| {
                    if !is_fabricator(&entity.id) {
                        return None;
                    }
                    set_int(&mut entity.nbt, KEY_SELECTION, drop);
                    set_int(&mut entity.nbt, KEY_RUNTIME, 0);
                    Some(int_or(&entity.nbt, KEY_SELECTION, -1))
                })
                .flatten();
            match answer {
                Some(selection) => game.native_feedback(Text::literal(format!(
                    "Loot Fabricator selection: {selection}"
                ))),
                None => game.native_feedback(Text::literal("No Loot Fabricator at that position.")),
            }
        }
        ["hostilenetworks", "model", "set", x, y, z, slot, id, data] => {
            *ran = true;
            let (Some(pos), Ok(slot), Some(model), Ok(data)) = (
                parse_pos(&[*x, *y, *z]),
                slot.parse::<usize>(),
                model(id),
                data.parse::<i32>(),
            ) else {
                game.native_feedback(Text::literal(
                    "Usage: /hostilenetworks model set <x> <y> <z> <slot> <model> <data>",
                ));
                return;
            };
            // The component write needs the game's item defaults, which the
            // block-entity closure cannot borrow; type a copy and put it back.
            let Some(mut stack) = machine_slot(game, pos, slot as i32) else {
                game.native_feedback(Text::literal("No Data Model in that slot."));
                return;
            };
            let _ = set_stack_model(game, &mut stack, model.id);
            let _ = set_stack_data(game, &mut stack, data.clamp(0, 100_000));
            let written = game
                .native_with_mod_block_entity(pos, |entity| {
                    if let Some(target) = entity.items.get_mut(slot) {
                        *target = Some(stack.clone());
                        true
                    } else {
                        false
                    }
                })
                .is_some_and(|written| written);
            if written {
                game.native_feedback(Text::literal(format!(
                    "Slot {slot}: {} Data Model, {data} data",
                    model.name
                )));
            } else {
                game.native_feedback(Text::literal("Could not write that slot."));
            }
        }
        ["hostilenetworks", "take", x, y, z, slot] => {
            *ran = true;
            let (Some(pos), Ok(slot)) = (parse_pos(&[*x, *y, *z]), slot.parse::<usize>()) else {
                game.native_feedback(Text::literal(
                    "Usage: /hostilenetworks take <x> <y> <z> <slot>",
                ));
                return;
            };
            let taken = game
                .native_with_mod_block_entity(pos, |entity| {
                    entity.items.get_mut(slot).and_then(Option::take)
                })
                .flatten();
            match taken {
                Some(stack) => {
                    let item = stack_id(&stack).to_owned();
                    let count = stack.count;
                    match game.native_give_stack(player.uuid, stack) {
                        None => game.native_feedback(Text::literal(format!(
                            "Took {item} x{count}"
                        ))),
                        Some(left) => {
                            let _ = game.native_with_mod_block_entity(pos, |entity| {
                                if let Some(target) = entity.items.get_mut(slot) {
                                    *target = Some(left);
                                }
                            });
                            game.native_feedback(Text::literal("No room in the inventory."));
                        }
                    }
                }
                None => game.native_feedback(Text::literal("That slot is empty.")),
            }
        }
        ["hostilenetworks", "put", x, y, z, slot] => {
            *ran = true;
            let (Some(pos), Ok(slot)) = (parse_pos(&[*x, *y, *z]), slot.parse::<usize>()) else {
                game.native_feedback(Text::literal("Usage: /hostilenetworks put <x> <y> <z> <slot>"));
                return;
            };
            let Some(held) = game.held().cloned() else {
                game.native_feedback(Text::literal("Hold the stack to put in the slot."));
                return;
            };
            let occupied = machine_slot(game, pos, slot as i32).is_some();
            if occupied {
                game.native_feedback(Text::literal("That slot is not empty."));
                return;
            }
            let item = stack_id(&held).to_owned();
            let count = held.count;
            let placed = game
                .native_with_mod_block_entity(pos, |entity| {
                    if let Some(target) = entity.items.get_mut(slot) {
                        *target = Some(held.clone());
                        true
                    } else {
                        false
                    }
                })
                .is_some_and(|placed| placed);
            if placed {
                consume_held_slot(game, count);
                game.native_feedback(Text::literal(format!("Put {item} x{count} in slot {slot}")));
            } else {
                game.native_feedback(Text::literal("Could not write that slot."));
            }
        }
        _ => {}
    }
}

/// Takes `count` out of the acting player's held stack.
fn consume_held_slot(game: &mut Game<'_>, count: u32) {
    let slot = game.held_slot();
    let left = game
        .held()
        .map(|stack| stack.count.saturating_sub(count))
        .unwrap_or(0);
    let mut stack = game.held().cloned();
    if let Some(stack) = stack.as_mut() {
        stack.count = left;
    }
    *game.held_mut() = stack.filter(|stack| stack.count > 0);
    if game.lan_acting.is_none() {
        game.hand_stack_replaced(slot);
    }
}
