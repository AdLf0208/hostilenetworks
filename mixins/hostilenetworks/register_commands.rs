// Runs at the head of `Game::native_register_commands` (API 8): the harness
// commands.
fn observe_register(commands: &mut Vec<NativeCommand>) {
    crate::game::hostilenetworks_state::on_register_commands(commands);
}
