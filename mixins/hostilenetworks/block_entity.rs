// Runs at the head of `Game::native_block_entity` (API 15): the machines get a
// block entity.
fn observe_block_entity(
    block: &str,
    entity: &mut Option<(String, horizon_world::nbt::Compound)>,
) {
    crate::game::hostilenetworks_state::on_block_entity(block, entity);
}
