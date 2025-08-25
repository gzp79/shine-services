use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        error::BevyError,
        event::EventReader,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    gizmos::gizmos::Gizmos,
    input::{keyboard::KeyCode, mouse::MouseMotion, ButtonInput},
    log,
    math::{EulerRot, Vec2, Vec3},
    render::camera::Projection,
    state::state::{NextState, State, States},
    time::Time,
    transform::components::Transform,
};

use crate::camera_rig::{rigs, CameraPose, CameraRig};

/// Camera debug state to enable or disable debug camera functionality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, States)]
pub enum CameraDebugState {
    #[default]
    Disabled,
    Enabled,
}

/// Component that marks an entity as a debug target for the camera system.
/// When this component is added to any entity, the debug camera will automatically be enabled.
/// When no entities have this component, the debug camera will be disabled.
#[derive(Component)]
pub struct DebugTargetCamera;

/// Marker indicating the CameraPose of the debug camera rig
#[derive(Component)]
pub struct DebugCameraPose;

pub fn spawn_debug_camera(
    camera_q: Query<&Transform, With<DebugTargetCamera>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    log::debug!("Spawning debug camera");

    let target_camera = camera_q.single()?;
    let (yaw, pitch, _) = target_camera.rotation.to_euler(EulerRot::YXZ);
    let yaw = yaw.to_degrees();
    let pitch = pitch.to_degrees();

    let camera = {
        let mut rig = CameraRig::new()
            .with(rigs::Position::new(target_camera.translation))
            .with(rigs::YawPitch::new().yaw_degrees(yaw).pitch_degrees(pitch))
            .with(rigs::Smooth::new_position_rotation(1.0, 1.0));
        (DebugCameraPose, rig.calculate_transform(0.0), rig)
    };

    commands.spawn(camera);

    Ok(())
}

pub fn despawn_debug_camera(debug_q: Query<Entity, With<DebugCameraPose>>, mut commands: Commands) {
    for entity in debug_q.iter() {
        commands.entity(entity).despawn();
    }
}

/// System that automatically toggles the debug camera based on the presence of DebugTarget components
pub fn auto_toggle_debug_camera(
    debug_targets: Query<&DebugTargetCamera>,
    current_state: Res<State<CameraDebugState>>,
    mut next_state: ResMut<NextState<CameraDebugState>>,
) {
    let has_debug_targets = !debug_targets.is_empty();
    let is_enabled = *current_state.get() == CameraDebugState::Enabled;

    match (has_debug_targets, is_enabled) {
        // Enable debug camera if we have targets but it's not enabled
        (true, false) => {
            next_state.set(CameraDebugState::Enabled);
        }
        // Disable debug camera if we have no targets but it's enabled
        (false, true) => {
            next_state.set(CameraDebugState::Disabled);
        }
        // No change needed in other cases
        _ => {}
    }
}

pub fn handle_debug_inputs(
    camera_q: Query<&Transform, With<DebugTargetCamera>>,
    mut rig_q: Query<&mut CameraRig, With<DebugCameraPose>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    time: Res<Time>,
) {
    if let (Ok(transform), Ok(mut rig)) = (camera_q.single(), rig_q.single_mut()) {
        let mut move_vec = Vec3::ZERO;
        if keyboard_input.pressed(KeyCode::Numpad4) {
            move_vec.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::Numpad6) {
            move_vec.x += 1.0;
        }
        if keyboard_input.pressed(KeyCode::Numpad8) {
            move_vec.z -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::Numpad5) {
            move_vec.z += 1.0;
        }
        if keyboard_input.pressed(KeyCode::Numpad9) {
            move_vec.y -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::Numpad7) {
            move_vec.y += 1.0;
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

pub fn render_camera_gizmos(camera_q: Query<(&CameraPose, &Projection)>, gizmo: Gizmos) {
    for (pose, projection) in camera_q.iter() {
        // add frustum of the camera
    }
}
