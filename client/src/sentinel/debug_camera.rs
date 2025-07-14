use crate::sentinel::{camera::MainCamera, DebugAction, Sentinel, SentinelAction};
use bevy::{
    core_pipeline::core_3d::Camera3d,
    ecs::{
        component::Component,
        entity::Entity,
        error::BevyError,
        query::{QuerySingleError, With},
        system::{Commands, Query, Res},
    },
    math::Vec3,
    render::camera::Camera,
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
    sentinel_q: Query<&ActionState<SentinelAction>, With<Sentinel>>,
    debug_camera_q: Query<Entity, With<DebugCamera>>,
    mut main_camera_q: Query<(&mut Camera, &Transform), With<MainCamera>>,
    mut windows: Query<&mut Window>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let sentinel_actions = sentinel_q.single()?;
    if !sentinel_actions.button(&SentinelAction::ToggleFreeView).just_pressed() {
        return Ok(());
    }

    match debug_camera_q.single() {
        Ok(_) => {
            log::info!("Debug camera is already enabled");
            return Ok(());
        }
        Err(QuerySingleError::NoEntities(..)) => {
            log::info!("Enabling debug camera");
        }
        Err(err) => return Err(err.into()),
    };

    let mut window = windows.single_mut().unwrap();
    window.start_grab(false);

    // disable the main camera
    let (mut main_camera, main_camera_transform) = main_camera_q.single_mut()?;
    main_camera.is_active = false;

    // spawn the debug camera
    let input_map = InputMap::<DebugAction>::new()
        .with_dual_axis(DebugAction::MoveCamera, VirtualDPad::wasd())
        .with_dual_axis(DebugAction::RotateCamera, MouseMotionInput::new());

    let rig: CameraRig = CameraRig::builder()
        .with(rigs::Position::new(main_camera_transform.translation))
        .with(rigs::YawPitch::new())
        .with(rigs::Smooth::new_position_rotation(1.0, 1.0))
        .build();

    let debug_camera = (DebugCamera, Camera3d::default(), *rig.transform(), rig, input_map);
    commands.spawn(debug_camera);

    Ok(())
}

pub fn disable(
    sentinel_q: Query<&ActionState<SentinelAction>, With<Sentinel>>,
    debug_camera_q: Query<Entity, With<DebugCamera>>,
    mut main_camera_q: Query<&mut Camera, With<MainCamera>>,
    mut windows: Query<&mut Window>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    let sentinel_actions = sentinel_q.single()?;
    if !sentinel_actions.button(&SentinelAction::ToggleFreeView).just_pressed() {
        return Ok(());
    }

    let debug_camera_entity = match debug_camera_q.single() {
        Ok(debug_camera) => {
            log::info!("Disabling debug camera");
            debug_camera
        }
        Err(QuerySingleError::NoEntities(..)) => {
            log::info!("Debug camera is already disabled");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let mut window = windows.single_mut().unwrap();
    window.cursor_options.grab_mode = CursorGrabMode::Confined;
    window.title = String::new();

    // re-enable the main camera
    let mut main_camera = main_camera_q.single_mut()?;
    main_camera.is_active = true;

    // Despawn the debug camera entity
    commands.entity(debug_camera_entity).despawn();

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
