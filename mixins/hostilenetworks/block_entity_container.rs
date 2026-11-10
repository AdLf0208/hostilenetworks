// Runs at the head of `Game::native_block_entity_container` (API 16/17): the
// machine slots and their menu types.
fn observe_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::hostilenetworks_state::on_container(block, container);
}
