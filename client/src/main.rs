use bevy::prelude::*;
use shine_game::application;

mod sentinel;
mod world;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
#[states(scoped_entities)]
pub enum GameState {
    InWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
#[states(scoped_entities)]
pub enum DebugState {
    NoDebug,
    HasFreeCamera,
}

/// Add all the game plugins to the app.
fn setup_game(app: &mut App) {
    app.add_plugins(world::WorldPlugin)
        .add_plugins(sentinel::SentinelPlugin);

    app.insert_state(GameState::InWorld);
    app.insert_state(DebugState::NoDebug);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
    use shine_game::application::{create_application, platform};

    application::init(setup_game);
    let mut app = create_application(platform::Config::default());

    app.add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::new());

    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    application::init(setup_game);
}
