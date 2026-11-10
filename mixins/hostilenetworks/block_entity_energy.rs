// Runs at the head of `Game::native_block_entity_energy` (API 142): the
// machines' FE buffers.
fn observe_energy(
    block: &str,
    energy: &mut Option<horizon_client::containers::ModEnergyBuffer>,
) {
    crate::game::hostilenetworks_state::on_energy(block, energy);
}
