// Runs at the head of `Game::native_block_entity_tick` (API 17): one tick of a
// simulation run or of a fabrication.
fn observe_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::hostilenetworks_state::on_tick(game, pos, entity);
}
