// Runs at the head of `Game::native_perform_command` (API 8): the command
// bodies.
fn observe_perform(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::hostilenetworks_state::on_perform_command(game, player, command, ran);
}
