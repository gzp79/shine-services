use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::CursorGrabMode,
    {color::palettes::css, render::view::NoIndirectDrawing},
};
use shine_game::{
    app::{init_application, AppGameSchedule, GameSystem},
    camera_rig::{rigs, CameraPoseDebug, CameraRig, CameraRigPlugin, DebugCameraTarget},
    math::animated::TemporalValueExt,
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
    app.add_plugins(CameraRigPlugin { enable_debug: true });

    app.add_systems(Startup, spawn_world);
    app.add_update_systems(GameSystem::Action, (handle_input, toggle_camera_debug));
}

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> Result<(), BevyError> {
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
            .with(rigs::Position::new(Vec3::new(-2.0, 2.5, 5.0).with_name("position")))?
            .with(rigs::YawPitch::new((90.0).with_name("yaw"), (-30.0).with_name("pitch")))?
            .with(rigs::Smooth::position_rotation(1.0, 1.0))?;

        let mut rig_debug = CameraPoseDebug::default();
        let transform = rig.calculate_transform(0.0, Some(&mut rig_debug.update_steps));

        (
            Camera3d::default(),
            NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
            rig,
            rig_debug,
            transform,
        )
    };
    commands.spawn(camera);

    Ok(())
}

fn toggle_camera_debug(
    camera_q: Query<(Entity, Option<&DebugCameraTarget>), With<Camera>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    for (entity, debug_target) in camera_q.iter() {
        if keyboard_input.just_pressed(KeyCode::F12) {
            if debug_target.is_some() {
                commands.entity(entity).remove::<DebugCameraTarget>();
            } else {
                commands.entity(entity).insert(DebugCameraTarget::default());
            }
        }
    }
}

fn handle_input(
    mut query: Query<(&Transform, &mut CameraRig), With<Camera3d>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    time: Res<Time>,
) -> Result<(), BevyError> {
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

        rig.set_parameter_with("yaw", |yaw: f32| yaw - 0.1 * delta.x)?;
        rig.set_parameter_with("pitch", |pitch: f32| pitch - 0.1 * delta.y)?;
        rig.set_parameter_with("position", |pos: Vec3| pos + move_vec * time.delta_secs() * 10.0)?;
    }

    Ok(())
}
