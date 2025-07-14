use crate::{
    sentinel::{camera::MainCamera, DebugAction},
    GameState,
};
use bevy::{
    asset::Assets,
    color::{palettes::css, Color},
    core_pipeline::core_3d::Camera3d,
    ecs::{
        component::Component,
        entity::Entity,
        error::BevyError,
        name::Name,
        query::{QuerySingleError, With, Without},
        system::{Commands, Query, Res, ResMut},
    },
    math::Vec3,
    pbr::{MeshMaterial3d, StandardMaterial},
    render::{
        camera::Camera,
        mesh::{Mesh, Mesh3d},
        primitives,
    },
    state::state_scoped::StateScoped,
    time::Time,
    transform::components::Transform,
    window::{CursorGrabMode, Window},
};
use shine_game::{
    application::WindowExt,
    camera_rig::{rigs, CameraRig},
    input_manager::{ActionState, InputMap, MouseMotionInput, VirtualDPad},
};

#[derive(Component)]
pub struct DebugCamera;

pub fn enable(
    mut main_camera_q: Query<(&mut Camera, &Transform), With<MainCamera>>,
    mut windows: Query<&mut Window>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.start_grab(CursorGrabMode::Locked);

    // disable the main camera
    let (mut main_camera, main_camera_transform) = main_camera_q.single_mut()?;
    main_camera.is_active = false;

    // spawn the debug camera
    let input_map = InputMap::<DebugAction>::new()
        .with_dual_axis(DebugAction::MoveCamera, VirtualDPad::wasd())
        .with_dual_axis(DebugAction::RotateCamera, MouseMotionInput::new());

    let rig: CameraRig = CameraRig::builder()
        .with(rigs::Position::new(Vec3::new(-2.0, 2.5, 5.0)))
        .with(rigs::YawPitch::new().yaw_degrees(90.0).pitch_degrees(-30.0))
        .with(rigs::Smooth::new_position_rotation(1.0, 1.0))
        .build();
    /*let rig: CameraRig = CameraRig::builder()
    .with(rigs::Position::new(main_camera_transform.translation))
    .with(rigs::YawPitch::new())
    .with(rigs::Smooth::new_position_rotation(1.0, 1.0))
    .build();*/

    let debug_camera = (
        Name::new("DebugCamera"),
        DebugCamera,
        StateScoped(GameState::InWorld),
        Camera3d::default(),
        //NoIndirectDrawing,
        *rig.transform(),
        rig,
        input_map,
    );
    commands.spawn(debug_camera);

    Ok(())
}

/*
pub fn spawn_camera_frustums(
    frustum_q: Query<&Transform, Without<DebugCamera>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let debug_frustums = commands
        .spawn((Name::new("DebugFrustums"), Transform::IDENTITY))
        .with_children(|parent| {
            for transform in frustum_q.iter() {
                let frustum = (
                    Mesh3d(meshes.add(primitives::Frustum::from_clip_from_world(&transform.compute_matrix()))),
                    MeshMaterial3d(materials.add(Color::Srgba(css::DARK_BLUE))),
                );
                parent.spawn(frustum);
            }
        });
}*/

pub fn disable(
    mut main_camera_q: Query<&mut Camera, With<MainCamera>>,
    mut windows: Query<&mut Window>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let mut window = windows.single_mut().unwrap();
    window.start_grab(CursorGrabMode::Confined);
    window.title = String::new();

    // re-enable the main camera
    let mut main_camera = main_camera_q.single_mut()?;
    main_camera.is_active = true;

    Ok(())
}

pub fn handle_input(
    mut debug_camera_q: Query<(&ActionState<DebugAction>, &mut Transform, &mut CameraRig), With<DebugCamera>>,
    time: Res<Time>,
) -> Result<(), BevyError> {
    let (action_state, mut transform, mut rig) = match debug_camera_q.single_mut() {
        Ok(data) => data,
        Err(QuerySingleError::NoEntities(..)) => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    let movement = action_state.dual_axis(&DebugAction::MoveCamera).value;
    let rotation = action_state.dual_axis(&DebugAction::RotateCamera).value;

    rig.driver_mut::<rigs::YawPitch>()
        .rotate_yaw_pitch(-0.1 * rotation.x, -0.1 * rotation.y);
    rig.driver_mut::<rigs::Position>()
        .translate(Vec3::new(movement.x, 0.0, movement.y) * time.delta_secs() * 10.0);

    *transform = rig.update(time.delta_secs());

    Ok(())
}
