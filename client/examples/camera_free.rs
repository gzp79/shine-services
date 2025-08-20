use bevy::prelude::*;
use bevy::{
    input::mouse::MouseMotion,
    window::CursorGrabMode,
    {color::palettes::css, render::view::NoIndirectDrawing},
};
use shine_game::{
    app::{init_application, AppGameSchedule},
    camera_rig::{rigs, CameraPose, CameraRig, CameraRigPlugin},
};

#[cfg(not(target_arch = "wasm32"))]
pub fn main() {
    use shine_game::app::{create_application, platform::Config};

    init_application(setup_game);
    let mut app = create_application(Config::default());
    app.run();
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    init_application(setup_game);
}

fn setup_game(app: &mut App) {
    app.add_plugins(CameraRigPlugin::default());

    app.add_systems(Startup, spawn_world);

    app.add_input(handle_input);
    app.add_render(update_camera);
}

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut window = windows.single_mut().unwrap();
    window.cursor_options.grab_mode = CursorGrabMode::Locked;
    window.title = "Camera Free (Mouse, WASD)".to_string();

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

    let camera = {
        let mut rig: CameraRig = CameraRig::new()
            .with(rigs::Position::new(Vec3::new(-2.0, 2.5, 5.0)))
            .with(rigs::YawPitch::new().yaw_degrees(90.0).pitch_degrees(-30.0))
            .with(rigs::Smooth::new_position_rotation(1.0, 1.0));

        (
            Camera3d::default(),
            NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
            rig.calculate_transform(0.0),
            rig,
        )
    };
    commands.spawn(camera);
}

fn handle_input(
    mut query: Query<(&Transform, &mut CameraRig), With<Camera3d>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    time: Res<Time>,
) {
    for (transform, mut rig) in query.iter_mut() {
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
        let move_vec = transform.rotation * move_vec;

        let mut delta = Vec2::ZERO;
        for event in mouse_motion_events.read() {
            delta += event.delta;
        }

        rig.driver_mut::<rigs::YawPitch>()
            .rotate_yaw_pitch(-0.1 * delta.x, -0.1 * delta.y);
        rig.driver_mut::<rigs::Position>()
            .translate(move_vec * time.delta_secs() * 10.0);
    }
}

fn update_camera(query: Query<(&mut Transform, &CameraPose)>) {
    for (mut transform, pose) in query {
        *transform = pose.transform;
    }
}
