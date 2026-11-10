// Runs at the head of `Game::native_menu_button` (API 20): the server applies
// the palette pick or the mode switch.
fn observe_menu_button(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
    id: i32,
) {
    crate::game::hostilenetworks_state::on_menu_button(game, pos, entity, id);
}
