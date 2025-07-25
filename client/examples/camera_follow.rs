use bevy::prelude::*;
use bevy::{color::palettes::css, render::view::NoIndirectDrawing};
use shine_game::{
    application,
    camera_rig::{rigs, CameraRig},
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
    window.title = "Camera Follow POC".to_string();

    let start_position = Vec3::new(0.0, 0.0, 0.0);

    let player = (
        Mesh3d(meshes.add(Tetrahedron::new(
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.5, -1.0),
        ))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
        Transform::from_translation(start_position),
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
        .with(rigs::Position::new(start_position))
        .with(rigs::Rotation::new(Quat::default()))
        .with(rigs::Smooth::new_position(1.25).predictive(true))
        .with(rigs::Arm::new(Vec3::new(0.0, 3.5, -5.5)))
        .with(rigs::Smooth::new_position(2.5).predictive(true))
        .with(
            rigs::LookAt::new(start_position + Vec3::Y)
                .smoothness(1.25)
                .predictive(true),
        )
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
        move_vec.x += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        move_vec.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyW) {
        move_vec.z += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        move_vec.z -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::ShiftLeft) {
        log::debug!("Shift pressed, moving faster");
        move_vec *= 10.0f32
    }

    let mut rot = 0.0;
    if keyboard_input.pressed(KeyCode::KeyQ) {
        rot += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyE) {
        rot -= 1.0;
    }

    rot *= time.delta_secs() * 2.0;
    player.rotation = Quat::from_rotation_y(rot) * player.rotation;

    move_vec = player.rotation * move_vec;
    move_vec.y = 0.0;
    if move_vec.length_squared() > 0.0 {
        move_vec = move_vec.normalize();
    }
    move_vec *= time.delta_secs() * 5.0;

    player.translation += move_vec;

    rig.driver_mut::<rigs::Position>().position = player.translation;
    rig.driver_mut::<rigs::Rotation>().rotation = player.rotation;
    rig.driver_mut::<rigs::LookAt>().target = player.translation + Vec3::Y;
}

fn update_camera(mut query: Query<(&mut Transform, &mut CameraRig), With<Camera3d>>, time: Res<Time>) {
    for (mut transform, mut rig) in query.iter_mut() {
        *transform = rig.update(time.delta_secs());
    }
}
