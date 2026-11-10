// Runs at the head of `Game::native_block_entity_ticks` (API 15): the machines
// tick on the server.
fn observe_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::hostilenetworks_state::on_block_entity_ticks(block, ticks);
}
