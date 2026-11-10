// Pure logic for the Hostile Neural Networks slice: model tiers, the
// Simulation Chamber's run and the Loot Fabricator's production.
//
// This file is both the standalone Cargo library and the injected
// `crate::game::hostilenetworks` module; no generated copy exists. Everything
// here is free of game state, so the behaviour is unit-tested without a
// running world; `support.rs` maps it onto block entities and items.
//
// Behaviour read from Hostile Neural Networks 1.21.1-6.5.1 (MIT) for the
// rewrite; the values are its data files (`model_tiers/*.json`) and its
// `SimChamberTileEntity`/`LootFabTileEntity`.

/// What a simulation run produces or consumes (`Item` as the port needs it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: &'static str,
    pub count: u32,
}

impl Item {
    /// The stack this item makes.
    #[must_use]
    pub const fn new(id: &'static str, count: u32) -> Self {
        Self { id, count }
    }
}

/// A model tier's data, from `data/hostilenetworks/model_tiers/*.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Faulty,
    Basic,
    Advanced,
    Superior,
    SelfAware,
}

impl Tier {
    /// Every tier, lowest first, as `ModelTierRegistry::sorted`.
    pub const VALUES: [Tier; 5] = [
        Tier::Faulty,
        Tier::Basic,
        Tier::Advanced,
        Tier::Superior,
        Tier::SelfAware,
    ];

    /// `required_data`: the data a model needs to be of this tier.
    #[must_use]
    pub const fn required_data(self) -> i32 {
        match self {
            Tier::Faulty => 0,
            Tier::Basic => 6,
            Tier::Advanced => 54,
            Tier::Superior => 354,
            Tier::SelfAware => 1254,
        }
    }

    /// `accuracy`: the guaranteed predictions per run at this tier.
    #[must_use]
    pub const fn accuracy(self) -> f32 {
        match self {
            Tier::Faulty => 0.0,
            Tier::Basic => 0.05,
            Tier::Advanced => 0.22,
            Tier::Superior => 0.65,
            Tier::SelfAware => 1.0,
        }
    }

    /// `can_sim`: only a faulty model cannot be simulated.
    #[must_use]
    pub const fn can_sim(self) -> bool {
        !matches!(self, Tier::Faulty)
    }

    /// The `hostilenetworks.tier.*` key's path.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Tier::Faulty => "faulty",
            Tier::Basic => "basic",
            Tier::Advanced => "advanced",
            Tier::Superior => "superior",
            Tier::SelfAware => "self_aware",
        }
    }

    /// The next tier, or this one at the top (`ModelTierRegistry::next`).
    #[must_use]
    pub fn next(self) -> Tier {
        let index = Self::VALUES
            .iter()
            .position(|tier| *tier == self)
            .unwrap_or(0);
        Self::VALUES.get(index + 1).copied().unwrap_or(self)
    }
}

/// `ModelTierRegistry.getByData`: the highest tier whose requirement `data`
/// reaches. The lowest tier requires zero, so every count has a tier.
#[must_use]
pub fn tier_for(data: i32) -> Tier {
    let data = data.max(0);
    Tier::VALUES
        .iter()
        .rev()
        .copied()
        .find(|tier| data >= tier.required_data())
        .unwrap_or(Tier::Faulty)
}

/// `DataModelInstance.getAccuracy` with continuous accuracy on: the tier's
/// accuracy, plus the next tier's lead in proportion to the progress towards
/// it. The top tier has no next one.
#[must_use]
pub fn accuracy_at(data: i32) -> f32 {
    let tier = tier_for(data);
    let next = tier.next();
    if next == tier {
        return tier.accuracy();
    }
    let span = next.required_data() - tier.required_data();
    let done = (data - tier.required_data()).clamp(0, span);
    let lead = next.accuracy() - tier.accuracy();
    tier.accuracy() + lead * (span - (span - done)) as f32 / span as f32
}

/// `DataModelInstance.rollPredictions` with `sample` as the random float:
/// each whole 100 % is a guaranteed prediction, the fraction is a chance of
/// one more.
#[must_use]
pub fn roll_predictions(accuracy: f32, sample: f32) -> i32 {
    if accuracy <= 0.0 {
        return 0;
    }
    let guaranteed = accuracy.floor();
    let fraction = accuracy - guaranteed;
    let extra = if sample < fraction { 1 } else { 0 };
    guaranteed as i32 + extra
}

/// The chamber's operating mode (`SimChamberTileEntity.SimMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Produces loot; runs 25 % faster than training.
    Inference,
    /// Upgrades the model's data; costs 20 % more per tick.
    Training,
}

impl Mode {
    /// `SimMode.BASE_RUNTIME` is 300 ticks at 100 % speed.
    pub const BASE_RUNTIME: i32 = 300;

    /// The length of one run in this mode: `BASE_RUNTIME / speed`. Inference
    /// runs at 125 %, so 300 * 4 / 5 = 240 ticks.
    #[must_use]
    pub const fn runtime(self) -> i32 {
        match self {
            Mode::Inference => Self::BASE_RUNTIME * 4 / 5,
            Mode::Training => Self::BASE_RUNTIME,
        }
    }

    /// `SimMode.adjustCost`: the mode's multiplier on a model's `sim_cost`.
    #[must_use]
    pub fn cost(self, base: i32) -> i32 {
        match self {
            Mode::Inference => base,
            // Mth.ceil(base * 1.2)
            Mode::Training => (f64::from(base) * 1.2).ceil() as i32,
        }
    }

    /// The mode switched to by a click, as `SimMode.next`.
    #[must_use]
    pub const fn next(self) -> Mode {
        match self {
            Mode::Inference => Mode::Training,
            Mode::Training => Mode::Inference,
        }
    }

    /// The `hostilenetworks.gui.mode.*` key's path.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Mode::Inference => "inference",
            Mode::Training => "training",
        }
    }
}

/// `SimChamberTileEntity.FailureState`, for the states this slice can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailState {
    None,
    Output,
    Energy,
    Input,
    Model,
    Faulty,
    EnergyMidCycle,
    Redstone,
}

impl FailState {
    /// The `hostilenetworks.fail.*` key's path.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            FailState::None => "none",
            FailState::Output => "output",
            FailState::Energy => "energy",
            FailState::Input => "input",
            FailState::Model => "model",
            FailState::Faulty => "faulty",
            FailState::EnergyMidCycle => "energy_mid_cycle",
            FailState::Redstone => "redstone",
        }
    }
}

/// One data model's data (`data/hostilenetworks/data_models/*.json`), as the
/// port carries it: models are a built-in table here, since Horizon has no
/// data-pack registry for them yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    /// The registry id a model item names (`hostilenetworks:zombie`).
    pub id: &'static str,
    /// The English display name (`EntityType.getDescription`).
    pub name: &'static str,
    /// FE per tick while a simulation runs.
    pub sim_cost: i32,
    /// The item one simulation consumes.
    pub input: &'static str,
    /// The model's base drop: one per completed run.
    pub base_drop: Item,
    /// The Loot Fabricator's table, in the order its screen lists it.
    pub fab_drops: &'static [(&'static str, u32)],
}

/// `minecraft:zombie` (slice 1's one model). `reliquary:zombie_heart` is
/// `optional` upstream and is left out: the mod is not ported.
pub const ZOMBIE: Model = Model {
    id: "hostilenetworks:zombie",
    name: "Zombie",
    sim_cost: 128,
    input: "hostilenetworks:prediction_matrix",
    base_drop: Item::new("hostilenetworks:overworld_prediction", 1),
    fab_drops: &[
        ("minecraft:rotten_flesh", 64),
        ("minecraft:iron_ingot", 8),
        ("minecraft:carrot", 16),
        ("minecraft:potato", 16),
    ],
};

/// Every model this slice knows.
pub const MODELS: &[Model] = &[ZOMBIE];

/// The model an id names, when the slice carries it.
#[must_use]
pub fn model(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|model| model.id == id)
}

/// The stack a model's Loot Fabricator selection makes, when the index is in
/// its table.
#[must_use]
pub fn fab_drop(model: &Model, index: i32) -> Option<Item> {
    let index = usize::try_from(index).ok()?;
    let (id, count) = model.fab_drops.get(index)?;
    Some(Item::new(id, *count))
}

/// The chamber as one tick sees it: its saved state and what its slots hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChamberView {
    /// The model the model slot holds, resolved.
    pub model: Option<&'static Model>,
    /// The data the model item carries (0 when it carries none).
    pub data: i32,
    /// The input slot holds what the model's input names.
    pub has_input: bool,
    /// The two output slots can take another run's drops.
    pub output_room: bool,
    /// The chamber's redstone rule is satisfied.
    pub powered: bool,
    pub energy: i32,
    pub runtime: i32,
    /// The predictions this run was rolled for at its start.
    pub prediction_success: i32,
    pub mode: Mode,
    /// A random float in `0..1` for this tick's prediction roll.
    pub sample: f32,
}

/// What the chamber's tick does, for the glue to apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChamberAction {
    /// No model in the slot: clear progress, show `model`.
    Idle(FailState),
    /// A run finished: add the base drop and the predictions, reset progress.
    Finish {
        base_drop: Item,
        predictions: u32,
        /// The model gains one iteration per finished run.
        iterations: u32,
    },
    /// A run began: consume one input, count down from `runtime`.
    Start { runtime: i32, predictions: i32 },
    /// One tick of a run: `runtime` counts down by one, the energy drains.
    Step { energy_cost: i32 },
}

/// `SimChamberTileEntity.serverTick`, as a decision. `runtime == 0` is "no run
/// in progress": the chamber starts one when it can, and shows why it cannot.
#[must_use]
pub fn chamber_action(view: &ChamberView) -> ChamberAction {
    let Some(model) = view.model else {
        return ChamberAction::Idle(FailState::Model);
    };
    let tier = tier_for(view.data);
    if !tier.can_sim() {
        return ChamberAction::Idle(FailState::Faulty);
    }
    let cost = view.mode.cost(model.sim_cost);
    if view.runtime <= 0 {
        let blocked = if !view.has_input {
            FailState::Input
        } else if !view.powered {
            FailState::Redstone
        } else if !view.output_room {
            FailState::Output
        } else if view.energy < cost {
            FailState::Energy
        } else {
            return ChamberAction::Start {
                runtime: view.mode.runtime(),
                predictions: roll_predictions(accuracy_at(view.data), view.sample),
            };
        };
        return ChamberAction::Idle(blocked);
    }
    if view.energy < cost {
        return ChamberAction::Idle(FailState::EnergyMidCycle);
    }
    if !view.powered {
        return ChamberAction::Idle(FailState::Redstone);
    }
    if view.runtime == 1 {
        // The last tick of a run does not drain energy, as upstream's
        // `--runtime == 0` branch returns before `setEnergy`.
        return ChamberAction::Finish {
            base_drop: model.base_drop,
            predictions: view.prediction_success.max(0) as u32,
            iterations: 1,
        };
    }
    ChamberAction::Step { energy_cost: cost }
}

/// The Loot Fabricator's ticks per item (`runtime >= 60`).
pub const FAB_RUNTIME: i32 = 60;
/// `fabPowerCost`: FE per tick while an item fabricates.
pub const FAB_POWER_COST: i32 = 256;
/// `simPowerCap`: the Simulation Chamber's FE capacity.
pub const SIM_POWER_CAP: i32 = 2_000_000;
/// `fabPowerCap`: the Loot Fabricator's FE capacity.
pub const FAB_POWER_CAP: i32 = 1_000_000;

/// The fabricator as one tick sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FabView {
    /// The prediction slot's model, resolved.
    pub model: Option<&'static Model>,
    /// The selected drop index; `-1` is no selection.
    pub selection: i32,
    /// One of the 16 output slots can take the drop.
    pub output_room: bool,
    pub powered: bool,
    pub energy: i32,
    pub runtime: i32,
}

/// What the fabricator's tick does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FabAction {
    /// Nothing to fabricate: reset progress.
    Idle(FailState),
    /// One tick of fabrication: progress and drain.
    Step { energy_cost: i32 },
    /// An item is done: add it to the outputs, consume one prediction.
    Produce { drop: Item },
}

/// `LootFabTileEntity.serverTick`, as a decision.
#[must_use]
pub fn fab_action(view: &FabView) -> FabAction {
    let Some(model) = view.model else {
        return FabAction::Idle(FailState::Model);
    };
    let Some(drop) = fab_drop(model, view.selection) else {
        return FabAction::Idle(FailState::Input);
    };
    if !view.powered {
        // Upstream pauses without resetting progress, so a queue can be edited.
        return FabAction::Idle(FailState::Redstone);
    }
    if view.runtime >= FAB_RUNTIME {
        if !view.output_room {
            return FabAction::Idle(FailState::Output);
        }
        return FabAction::Produce { drop };
    }
    if view.energy < FAB_POWER_COST {
        return FabAction::Idle(FailState::Energy);
    }
    FabAction::Step {
        energy_cost: FAB_POWER_COST,
    }
}

/// The component ids the model and prediction items carry. Registered by
/// `component_types` before content loads.
pub const DATA_MODEL_COMPONENT: &str = "hostilenetworks:data_model";
pub const DATA_COMPONENT: &str = "hostilenetworks:data";
pub const ITERATIONS_COMPONENT: &str = "hostilenetworks:iterations";

/// The item ids this slice places and reads.
pub const DATA_MODEL_ITEM: &str = "hostilenetworks:data_model";
pub const PREDICTION_ITEM: &str = "hostilenetworks:prediction";
pub const SIM_CHAMBER: &str = "hostilenetworks:sim_chamber";
pub const LOOT_FABRICATOR: &str = "hostilenetworks:loot_fabricator";

/// Whether a block id is the Simulation Chamber.
#[must_use]
pub fn is_chamber(block: &str) -> bool {
    block == SIM_CHAMBER
}

/// Whether a block id is the Loot Fabricator.
#[must_use]
pub fn is_fabricator(block: &str) -> bool {
    block == LOOT_FABRICATOR
}

/// The FE capacity of a machine block's buffer: the chamber's or the
/// fabricator's, and 0 for any other block.
#[must_use]
pub const fn data_cap_for(block: &str) -> i32 {
    if const_str_eq(block, SIM_CHAMBER) {
        SIM_POWER_CAP
    } else if const_str_eq(block, LOOT_FABRICATOR) {
        FAB_POWER_CAP
    } else {
        0
    }
}

/// `&str` equality in a `const fn` (no `PartialEq` in const context).
const fn const_str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_thresholds_follow_the_data_files() {
        assert_eq!(tier_for(0), Tier::Faulty);
        assert_eq!(tier_for(5), Tier::Faulty);
        assert_eq!(tier_for(6), Tier::Basic);
        assert_eq!(tier_for(53), Tier::Basic);
        assert_eq!(tier_for(54), Tier::Advanced);
        assert_eq!(tier_for(354), Tier::Superior);
        assert_eq!(tier_for(1253), Tier::Superior);
        assert_eq!(tier_for(1254), Tier::SelfAware);
        assert_eq!(tier_for(99_999), Tier::SelfAware);
    }

    #[test]
    fn only_the_faulty_tier_cannot_simulate() {
        assert!(!Tier::Faulty.can_sim());
        for tier in [Tier::Basic, Tier::Advanced, Tier::Superior, Tier::SelfAware] {
            assert!(tier.can_sim(), "{tier:?} must be simulatable");
        }
    }

    #[test]
    fn accuracy_is_continuous_within_a_tier() {
        assert_eq!(accuracy_at(0), 0.0);
        assert_eq!(accuracy_at(6), 0.05);
        // 14 of the 48 data to advanced: 0.05 + 0.17 * 14/48
        assert!((accuracy_at(20) - 0.099_583_33).abs() < 1e-6);
        assert_eq!(accuracy_at(54), 0.22);
        assert_eq!(accuracy_at(1254), 1.0);
    }

    #[test]
    fn a_fraction_of_a_prediction_is_a_chance_of_one() {
        assert_eq!(roll_predictions(0.65, 0.5), 1);
        assert_eq!(roll_predictions(0.65, 0.7), 0);
        assert_eq!(roll_predictions(0.05, 0.01), 1);
        assert_eq!(roll_predictions(0.05, 0.5), 0);
        assert_eq!(roll_predictions(1.0, 0.99), 1);
        assert_eq!(roll_predictions(0.0, 0.0), 0);
    }

    #[test]
    fn inference_runs_faster_and_training_costs_more() {
        assert_eq!(Mode::Inference.runtime(), 240);
        assert_eq!(Mode::Training.runtime(), 300);
        assert_eq!(Mode::Inference.cost(128), 128);
        assert_eq!(Mode::Training.cost(128), 154);
        assert_eq!(Mode::Inference.next(), Mode::Training);
        assert_eq!(Mode::Training.next(), Mode::Inference);
    }

    fn chamber(model: Option<&'static Model>, data: i32) -> ChamberView {
        ChamberView {
            model,
            data,
            has_input: true,
            output_room: true,
            powered: true,
            energy: SIM_POWER_CAP,
            runtime: 0,
            prediction_success: 0,
            mode: Mode::Inference,
            sample: 0.5,
        }
    }

    #[test]
    fn a_chamber_without_a_model_reports_it_and_stops() {
        let view = chamber(None, 0);
        assert_eq!(chamber_action(&view), ChamberAction::Idle(FailState::Model));
    }

    #[test]
    fn a_faulty_model_cannot_start() {
        let view = chamber(Some(&ZOMBIE), 0);
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Idle(FailState::Faulty)
        );
    }

    #[test]
    fn a_start_needs_input_room_and_power() {
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.has_input = false;
        assert_eq!(chamber_action(&view), ChamberAction::Idle(FailState::Input));
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.output_room = false;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Idle(FailState::Output)
        );
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.energy = 127;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Idle(FailState::Energy)
        );
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.powered = false;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Idle(FailState::Redstone)
        );
    }

    #[test]
    fn a_start_runs_for_the_mode_and_rolls_the_fraction() {
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.sample = 0.01;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Start {
                runtime: 240,
                predictions: 1
            }
        );
        let mut view = chamber(Some(&ZOMBIE), 1254);
        view.sample = 0.99;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Start {
                runtime: 240,
                predictions: 1
            }
        );
        // Training draws the energy cost 20 % up and runs 300 ticks.
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.mode = Mode::Training;
        view.runtime = 10;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Step { energy_cost: 154 }
        );
    }

    #[test]
    fn a_mid_cycle_tick_drains_and_the_last_tick_finishes() {
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.runtime = 10;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Step { energy_cost: 128 }
        );
        view.runtime = 1;
        view.prediction_success = 2;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Finish {
                base_drop: Item::new("hostilenetworks:overworld_prediction", 1),
                predictions: 2,
                iterations: 1
            }
        );
    }

    #[test]
    fn a_run_pauses_without_power_or_redstone() {
        let mut view = chamber(Some(&ZOMBIE), 20);
        view.runtime = 10;
        view.energy = 127;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Idle(FailState::EnergyMidCycle)
        );
        view.energy = SIM_POWER_CAP;
        view.powered = false;
        assert_eq!(
            chamber_action(&view),
            ChamberAction::Idle(FailState::Redstone)
        );
    }

    fn fabricator() -> FabView {
        FabView {
            model: Some(&ZOMBIE),
            selection: 0,
            output_room: true,
            powered: true,
            energy: FAB_POWER_CAP,
            runtime: 0,
        }
    }

    #[test]
    fn the_fabricator_needs_a_prediction_and_a_selection() {
        let mut view = fabricator();
        view.model = None;
        assert_eq!(fab_action(&view), FabAction::Idle(FailState::Model));
        let mut view = fabricator();
        view.selection = -1;
        assert_eq!(fab_action(&view), FabAction::Idle(FailState::Input));
        let mut view = fabricator();
        view.selection = 4;
        assert_eq!(fab_action(&view), FabAction::Idle(FailState::Input));
    }

    #[test]
    fn the_fabricator_takes_256_fe_a_tick_and_produces_on_the_sixtieth() {
        let mut view = fabricator();
        assert_eq!(
            fab_action(&view),
            FabAction::Step {
                energy_cost: FAB_POWER_COST
            }
        );
        view.runtime = 59;
        assert_eq!(
            fab_action(&view),
            FabAction::Step {
                energy_cost: FAB_POWER_COST
            }
        );
        view.runtime = 60;
        assert_eq!(
            fab_action(&view),
            FabAction::Produce {
                drop: Item::new("minecraft:rotten_flesh", 64)
            }
        );
    }

    #[test]
    fn the_fabricator_waits_without_power_room_or_redstone() {
        let mut view = fabricator();
        view.energy = 255;
        assert_eq!(fab_action(&view), FabAction::Idle(FailState::Energy));
        let mut view = fabricator();
        view.runtime = 60;
        view.output_room = false;
        assert_eq!(fab_action(&view), FabAction::Idle(FailState::Output));
        let mut view = fabricator();
        view.powered = false;
        assert_eq!(fab_action(&view), FabAction::Idle(FailState::Redstone));
    }

    #[test]
    fn the_second_drop_is_the_selected_one() {
        let mut view = fabricator();
        view.selection = 1;
        view.runtime = 60;
        assert_eq!(
            fab_action(&view),
            FabAction::Produce {
                drop: Item::new("minecraft:iron_ingot", 8)
            }
        );
    }

    #[test]
    fn the_zombie_model_carries_upstreams_table() {
        let zombie = model("hostilenetworks:zombie").expect("zombie is carried");
        assert_eq!(zombie.sim_cost, 128);
        assert_eq!(zombie.input, "hostilenetworks:prediction_matrix");
        assert_eq!(zombie.base_drop.id, "hostilenetworks:overworld_prediction");
        assert_eq!(zombie.fab_drops.len(), 4);
        assert_eq!(
            fab_drop(zombie, 0),
            Some(Item::new("minecraft:rotten_flesh", 64))
        );
        assert_eq!(fab_drop(zombie, 3), Some(Item::new("minecraft:potato", 16)));
        assert_eq!(fab_drop(zombie, 4), None);
        assert!(model("hostilenetworks:nope").is_none());
    }
}
