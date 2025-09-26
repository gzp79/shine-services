use bevy::{prelude::*, render::view::RenderLayers};
use shine_game::{app::init_application, tokio::TokioPlugin};

mod avatar;
mod camera;
mod hud;
mod map;
mod world;

pub const HUD_LAYER: RenderLayers = RenderLayers::layer(31);

/// Add all the game plugins to the app.
fn setup_game(app: &mut App) {
    use bevy_inspector_egui::bevy_egui::EguiPlugin;
    use bevy_inspector_egui::quick::WorldInspectorPlugin;

    app.add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(TokioPlugin);

    app.add_plugins(hud::HUDPlugin)
        .add_plugins(world::WorldPlugin)
        .add_plugins(map::MapPlugin)
        .add_plugins(avatar::AvatarPlugin)
        .add_plugins(camera::CameraPlugin);
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
