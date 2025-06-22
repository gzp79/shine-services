use bevy::prelude::*;
use shine_client::bevy_utils::application;

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_client::bevy_utils::application::{create_application, platform::Config};

    application::init(setup_game);
    let mut app = create_application(Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    application::init(setup_game);
}

fn setup_game(_app: &mut App) {}
