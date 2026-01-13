use crate::{avatar, camera, debug_functions, hud, world};
use bevy::{
    app::{App, Update},
    ecs::schedule::IntoScheduleConfigs,
    tasks::BoxedFuture,
};
use shine_game::{
    app::{platform, GameSetup, GameSystems, PlatformInit},
    tokio::TokioPlugin,
};

pub struct TheGame;

impl GameSetup for TheGame {
    type GameConfig = ();

    fn create_setup(&self, _config: &platform::Config) -> BoxedFuture<'static, Self::GameConfig> {
        Box::pin(async move {})
    }

    fn setup_application(&self, app: &mut App, config: &platform::Config, _game_config: ()) {
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
}
