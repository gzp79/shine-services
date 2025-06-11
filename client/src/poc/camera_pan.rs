use crate::camera_rig::{
    drivers::{Arm, Smooth, YawPitch},
    CameraRig,
};
use bevy::prelude::*;
use bevy::{color::palettes::css, render::view::NoIndirectDrawing};

pub struct CameraOrbitPOC<S>
where
    S: States,
{
    pub state: S,
}

impl<S> Plugin for CameraOrbitPOC<S>
where
    S: States,
{
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(self.state.clone()),
            (spawn_world, spawn_camera).chain(),
        );
        app.add_systems(
            Update,
            (handle_input, update_camera)
                .chain()
                .run_if(in_state(self.state.clone())),
        );
    }
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let player = (
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    );
    commands.spawn(player);

    let floor = (
        Mesh3d(meshes.add(Mesh::from(Plane3d::default().mesh().size(15.0, 15.0)))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_GREEN))),
    );
    commands.spawn(floor);

    let light = (
        PointLight {
            shadows_enabled: true,
            range: 100.0,
            intensity: 2000.0 * 1000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0),
    );
    commands.spawn(light);
}

fn spawn_camera(mut commands: Commands) {
    let rig: CameraRig = CameraRig::builder()
        .with(YawPitch::new().yaw_degrees(45.0).pitch_degrees(-30.0))
        .with(Smooth::new_rotation(1.5))
        .with(Arm::new(Vec3::Z * 8.0))
        .build();

    commands.spawn((
        Camera3d::default(),
        NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
        rig.transform().clone(),
        rig,
    ));
}

fn handle_input(
    mut query: Query<&mut CameraRig, With<Camera3d>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    for mut rig in query.iter_mut() {
        if keyboard_input.just_pressed(KeyCode::KeyZ) {
            rig.driver_mut::<YawPitch>().rotate_yaw_pitch(-90.0, 0.0);
        }
        if keyboard_input.just_pressed(KeyCode::KeyX) {
            rig.driver_mut::<YawPitch>().rotate_yaw_pitch(90.0, 0.0);
        }
    }
}

fn update_camera(
    mut query: Query<(&mut Transform, &mut CameraRig), With<Camera3d>>,
    time: Res<Time>,
) {
    for (mut transform, mut rig) in query.iter_mut() {
        *transform = rig.update(time.delta_secs());
    }
}
