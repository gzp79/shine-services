use bevy::{color::palettes::css, prelude::*, render::view::NoIndirectDrawing};
use shine_game::{
    app::{init_application, AppGameSchedule, GameSystems},
    camera_rig::{rigs, CameraPoseDebug, CameraRig, CameraRigPlugin, DebugCameraTarget},
    math::value::IntoNamedVariable,
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
    app.add_update_systems(GameSystems::Action, (handle_input, toggle_camera_debug));
}

#[derive(Component)]
pub struct Player;

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.title = "Camera Look At (WASD)".to_string();

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

    let camera = {
        let mut rig = CameraRig::new()
            .with(rigs::Position::new(Vec3::new(-2.0, 2.5, 5.0)))?
            .with(rigs::LookAt::new(Vec3::new(0.0, 0.5, 0.0).with_name("target")))?;
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
    mut player_q: Query<&mut Transform, With<Player>>,
    mut camera_q: Query<&mut CameraRig, With<Camera3d>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) -> Result<(), BevyError> {
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

    rig.set_parameter("target", player.translation)?;

    Ok(())
}
