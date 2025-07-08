use bevy::prelude::*;
use bevy::{color::palettes::css, render::view::NoIndirectDrawing};
use shine_game::{
    application,
    camera_rig::{
        drivers::{LookAt, Position},
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

#[derive(Component)]
pub struct Player;

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut window = windows.single_mut().unwrap();
    window.title = "Camera Look At POC".to_string();

    let player = (
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Player,
    );
    commands.spawn(player);

    let floor = (
        Mesh3d(meshes.add(Mesh::from(Plane3d::default().mesh().subdivisions(10).size(15.0, 15.0)))),
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
        Transform::from_xyz(0.0, 5.0, 0.0),
    );
    commands.spawn(light);

    let rig = CameraRig::builder()
        .with(Position::new(Vec3::new(-2.0, 2.5, 5.0)))
        .with(LookAt::new(Vec3::new(0.0, 0.5, 0.0)))
        .build();
    let camera = (
        Camera3d::default(),
        NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
        *rig.transform(),
        rig,
    );
    commands.spawn(camera);
}

fn handle_input(
    mut player_q: Query<&mut Transform, With<Player>>,
    mut camera_q: Query<&mut CameraRig, Without<Player>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let mut player = player_q.single_mut().unwrap();
    let mut rig = camera_q.single_mut().unwrap();

    let mut move_vec = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::KeyA) {
        move_vec.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        move_vec.x += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyW) {
        move_vec.z -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        move_vec.z += 1.0;
    }
    if keyboard_input.pressed(KeyCode::ShiftLeft) {
        log::debug!("Shift pressed, moving faster");
        move_vec *= 10.0f32
    }
    player.translation += move_vec * 5.0 * time.delta_secs();

    rig.driver_mut::<LookAt>().target = player.translation;
}

fn update_camera(mut query: Query<(&mut Transform, &mut CameraRig), With<Camera3d>>, time: Res<Time>) {
    for (mut transform, mut rig) in query.iter_mut() {
        *transform = rig.update(time.delta_secs());
    }
}
