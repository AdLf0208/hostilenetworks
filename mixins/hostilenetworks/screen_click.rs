// Runs at the head of `Game::native_screen_click` (API 20): a press on the
// drop palette names the menu button.
fn observe_screen_click(
    block: &str,
    data: &[i32],
    at: [f32; 2],
    right: bool,
    button: &mut Option<i32>,
) {
    crate::game::hostilenetworks_state::on_screen_click(block, data, at, right, button);
}
