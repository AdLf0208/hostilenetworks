// Runs at the head of `Game::native_component_types` (API 65): register the
// `hostilenetworks:` components a Data Model and a Prediction carry.
fn register_model_components(types: &mut Vec<String>) {
    crate::game::hostilenetworks_state::register_component_types(types);
}
