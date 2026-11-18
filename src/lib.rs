//! Hostile Neural Networks for Horizon — slice 1, the Simulation Chamber → Prediction → Loot
//! Fabricator loop.
//!
//! One flat SDK package (mod API 0.2): the carried blocks, items, models, textures, lang, menus,
//! recipes and loot tables are the `assets/` and `data/` here, and the behaviour is the one
//! `impl Hooks` below. The rules themselves live in [`runtime`], byte-identical to the retired
//! `.hnmod` bundle's `runtime.rs`; [`support`] is the glue.
//!
//! The retired bundle's twelve injected helpers are here three shapes: each machine's block
//! entity, tick, screen, draw, click and menu button are a **Part** attached to the type declared
//! for its block, its slots, FE buffer and menu are **declarations**, and the harness command is
//! the typed `register_commands`/`run_mod_command` pair. Its embedded-content helper has no
//! successor: one `.hmod` carries the content and the code, which is what that helper existed to
//! work around.
use mod_api::{
    self, ComponentTypesEvent, Flow, Hooks, RegisterCommandsEvent, Registrar, RegistrationContext,
    RunModCommandEvent, ServerHost,
};

pub mod runtime;
pub mod support;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare the two machines: their block entity types and Parts, their slots and menus, their
    /// insert-only FE buffers.
    ///
    /// The retired bundle wrote into the `block_entity`, `block_entity_ticks`,
    /// `block_entity_container` and `block_entity_energy` hooks' shared outputs and had to leave a
    /// foreign block's entry alone; a declaration is keyed by the block or by a type of the mod's
    /// own, so that hazard cannot recur.
    fn register(&self, registrar: &mut dyn Registrar, context: &RegistrationContext) {
        support::declare(registrar, context);
    }

    /// Before content loads: register the components a Data Model and a Prediction carry
    /// (upstream's `Hostile.Components`).
    fn component_types(&self, _context: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        for id in support::component_ids() {
            event.types.push(id.to_owned());
        }
    }

    /// `/hostilenetworks info|energy|model|fab|take|put`: the harness controls this slice's
    /// in-game check needs (there is no other way to charge a machine or type a model headlessly).
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        support::declare_commands(event);
    }

    /// The command bodies.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        support::run_command(host, event)
    }
}
