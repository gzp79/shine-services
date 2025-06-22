use bevy::prelude::*;
use shine_client::bevy_utils::application::{self, create_application, platform};

mod bevy_utils;
mod camera_rig;
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
    application::init(setup_game);
    let mut app = create_application(platform::Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    application::init(setup_game);
}
