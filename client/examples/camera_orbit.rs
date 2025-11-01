use bevy::{
    app::{App, Startup},
    asset::Assets,
    camera::{Camera, Camera3d},
    color::{palettes::css, Color},
    ecs::{
        entity::Entity,
        error::BevyError,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    input::{keyboard::KeyCode, ButtonInput},
    light::PointLight,
    math::{
        primitives::{Cuboid, Plane3d},
        Dir3, Vec2, Vec3,
    },
    mesh::{Mesh, Mesh3d},
    pbr::{MeshMaterial3d, StandardMaterial},
    render::view::NoIndirectDrawing,
    tasks::BoxedFuture,
    transform::components::Transform,
    utils::default,
    window::Window,
};
use shine_game::{
    app::{init_application, platform, AppGameSchedule, GameSetup, GameSystems, PlatformInit},
    camera_rig::{rigs, CameraRig, CameraRigPlugin, DebugCameraTarget},
    math::value::IntoNamedVariable,
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn main() {
    use shine_game::app::platform::{start_game, Config};

    init_application(GameExample);
    start_game(Config::default());
}
#[cfg(target_family = "wasm")]
pub fn main() {
    init_application(GameExample);
}

struct GameExample;

impl GameSetup for GameExample {
    type GameConfig = ();

    fn create_setup(&self, _config: &platform::Config) -> BoxedFuture<'static, Self::GameConfig> {
        Box::pin(async move {})
    }

    fn setup_application(&self, app: &mut App, config: &platform::Config, _game_config: ()) {
        app.platform_init(config);

        app.add_plugins(CameraRigPlugin { enable_debug: true });

        app.add_systems(Startup, spawn_world);
        app.add_update_systems(GameSystems::Action, (handle_input, toggle_camera_debug));
    }
}

fn spawn_world(
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.title = "Camera Orbit (QE)".to_string();

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::Srgba(css::YELLOW))),
        Transform::from_xyz(0.0, 0.0, 0.5),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 0.2, 0.2))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_RED))),
        Transform::from_xyz(1.0, 0.0, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 0.2, 0.2))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_GREEN))),
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 0.2, 0.2))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));

    let floor_plane = Plane3d {
        normal: Dir3::Z,
        half_size: Vec2::new(15.0, 15.0),
    };
    let floor = (
        Mesh3d(meshes.add(Mesh::from(floor_plane))),
        MeshMaterial3d(materials.add(Color::Srgba(css::DARK_GRAY))),
    );
    commands.spawn(floor);

    let light = (
        PointLight {
            shadows_enabled: true,
            range: 100.0,
            intensity: 2000.0 * 1000.0,
            ..default()
        },
        Transform::from_xyz(3.0, 0.0, 5.0),
    );
    commands.spawn(light);

    let camera = {
        let rig: CameraRig = CameraRig::new()
            .with(rigs::YawPitch::new((45.0).with_name("yaw"), (-30.0).with_name("pitch")))?
            .with(rigs::Smooth::rotation(1.5))?
            .with(rigs::Arm::new(Vec3::Z * 8.0))?;

        (
            Camera3d::default(),
            NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
            rig.into_bundle_with_trace(),
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
    mut query: Query<&mut CameraRig, With<Camera3d>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) -> Result<(), BevyError> {
    for mut rig in query.iter_mut() {
        if keyboard_input.just_pressed(KeyCode::KeyA) {
            rig.set_parameter_with("yaw", |x: f32| (x - 45.0) % 360.0)?;
        }
        if keyboard_input.just_pressed(KeyCode::KeyD) {
            rig.set_parameter_with("yaw", |x: f32| (x + 45.0) % 360.0)?;
        }

        if keyboard_input.just_pressed(KeyCode::KeyW) {
            rig.set_parameter_with("pitch", |x: f32| (x - 15.0).clamp(-90.0, 90.0))?;
        }
        if keyboard_input.just_pressed(KeyCode::KeyS) {
            rig.set_parameter_with("pitch", |x: f32| (x + 15.0).clamp(-90.0, 90.0))?;
        }
    }

    Ok(())
}
