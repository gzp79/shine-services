use bevy::{
    app::{App, Update},
    camera::visibility::RenderLayers,
    ecs::schedule::IntoScheduleConfigs,
};
use shine_game::{
    app::{init_application, platform, GameSystems, PlatformInit},
    tokio::TokioPlugin,
};

mod avatar;
mod camera;
mod hud;
mod world;

mod debug_functions;

pub const HUD_LAYER: RenderLayers = RenderLayers::layer(31);

/// Add all the game plugins to the app.
fn setup_game(app: &mut App, config: &platform::Config) {
    /*use bevy_inspector_egui::bevy_egui::EguiPlugin;
    use bevy_inspector_egui::quick::WorldInspectorPlugin;

     panics, need some investigation
    app.add_plugins(EguiPlugin::default())
       .add_plugins(WorldInspectorPlugin::new());*/

    app.platform_init(config);
    app.add_plugins(TokioPlugin);

    app.add_plugins(hud::HUDPlugin)
        .add_plugins(world::WorldPlugin)
        .add_plugins(avatar::AvatarPlugin)
        .add_plugins(camera::CameraPlugin);

    app.add_systems(Update, debug_functions::debug_load_chunk.in_set(GameSystems::Action));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::app::create_application;

    init_application(setup_game);
    let mut app = create_application(platform::Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    init_application(setup_game);
}
