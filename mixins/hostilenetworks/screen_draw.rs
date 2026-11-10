// Runs at the head of `Game::native_screen_draw` (API 19): energy, progress,
// tier and the fabricator's drop palette.
fn observe_screen_draw(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::hostilenetworks_state::on_screen_draw(game, block, data, batch, origin);
}
