use bevy::prelude::*;
use shine_game::application;

mod world;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum GameState {
    /// Main gameplay state.
    Playing,
}

/// Add all the game plugins to the app.
fn setup_game(app: &mut App) {
    app.add_plugins(world::WorldPlugin { state: GameState::Playing });

    app.insert_state(GameState::Playing);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::application::{create_application, platform};

    application::init(setup_game);
    let mut app = create_application(platform::Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    application::init(setup_game);
}
