use bevy::prelude::*;
use bevy::{color::palettes::css, render::view::NoIndirectDrawing};
use shine_game::{
    application,
    camera_rig::{
        rigs::{Arm, Smooth, YawPitch},
        CameraRig,
    },
};

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::application::{create_application, platform::Config};

    application::init(setup_game);
    let mut app = create_application(Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    application::init(setup_game);
}

fn setup_game(app: &mut App) {
    app.add_systems(Startup, spawn_world);
    app.add_systems(Update, (handle_input, update_camera).chain());
}

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut window = windows.single_mut().unwrap();
    window.title = "Camera Orbit POC".to_string();

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

    let rig: CameraRig = CameraRig::builder()
        .with(YawPitch::new().yaw_degrees(45.0).pitch_degrees(-30.0))
        .with(Smooth::new_rotation(1.5))
        .with(Arm::new(Vec3::Z * 8.0))
        .build();
    let camera = (
        Camera3d::default(),
        NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
        *rig.transform(),
        rig,
    );
    commands.spawn(camera);
}

fn handle_input(mut query: Query<&mut CameraRig, With<Camera3d>>, keyboard_input: Res<ButtonInput<KeyCode>>) {
    for mut rig in query.iter_mut() {
        if keyboard_input.just_pressed(KeyCode::KeyZ) {
            rig.driver_mut::<YawPitch>().rotate_yaw_pitch(-90.0, 0.0);
        }
        if keyboard_input.just_pressed(KeyCode::KeyX) {
            rig.driver_mut::<YawPitch>().rotate_yaw_pitch(90.0, 0.0);
        }
    }
}

fn update_camera(mut query: Query<(&mut Transform, &mut CameraRig), With<Camera3d>>, time: Res<Time>) {
    for (mut transform, mut rig) in query.iter_mut() {
        *transform = rig.update(time.delta_secs());
    }
}
