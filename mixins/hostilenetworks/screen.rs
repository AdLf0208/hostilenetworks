// Runs at the head of `Game::native_screen` (API 19): the machines' slot
// layouts.
fn observe_screen(block: &str, screen: &mut Option<NativeScreen>) {
    crate::game::hostilenetworks_state::on_screen(block, screen);
}
