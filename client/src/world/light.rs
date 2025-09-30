use bevy::{ecs::system::Commands, pbr::PointLight, transform::components::Transform, utils::default};

pub fn spawn_light(mut commands: Commands) {
    let light = (
        PointLight {
            intensity: 2000.0 * 1000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 0.0),
    );

    commands.spawn(light);
}
