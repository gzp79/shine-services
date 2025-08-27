use bevy::{prelude::*, render::view::RenderLayers};
use shine_game::app::init_application;

mod avatar;
mod camera;
mod hud;
mod world;

pub const HUD_LAYER: RenderLayers = RenderLayers::layer(31);

/// Add all the game plugins to the app.
fn setup_game(app: &mut App) {
    app.add_plugins(hud::HUDPlugin);
    app.add_plugins(world::WorldPlugin);
    app.add_plugins(avatar::AvatarPlugin);
    app.add_plugins(camera::CameraPlugin);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::app::{create_application, platform};

    init_application(setup_game);
    let mut app = create_application(platform::Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    init_application(setup_game);
}
