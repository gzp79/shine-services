use bevy::{
    app::{App, Startup},
    core_pipeline::core_3d::Camera3d,
    ecs::system::Commands,
    DefaultPlugins,
};

pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup_cam)
        .run();
}

fn setup_cam(mut commands: Commands) {
    commands.spawn(Camera3d::default());
}
