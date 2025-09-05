use crate::{
    camera_rig::{rigs, CameraPose, CameraPoseDebug, CameraRig},
    math::value::WithNameVariable,
};
use bevy::{
    color::{palettes::css, Color},
    ecs::{
        component::Component,
        error::BevyError,
        event::EventReader,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    gizmos::gizmos::Gizmos,
    input::{keyboard::KeyCode, mouse::MouseMotion, ButtonInput},
    log,
    math::{EulerRot, Vec2, Vec3, Vec3A},
    render::{
        camera::{Camera, CameraProjection, Projection},
        view::RenderLayers,
    },
    state::state::{NextState, State, States},
    text::{TextColor, TextFont},
    time::Time,
    transform::components::Transform,
    ui::{widget::Text, AlignItems, BackgroundColor, BorderColor, JustifyContent, Node, PositionType, UiRect, Val},
    window::{CursorGrabMode, PrimaryWindow, Window},
};

/// Camera debug state to enable or disable debug camera functionality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, States)]
pub enum CameraDebugState {
    #[default]
    Disabled,
    Enabled,
}

/// Marker to despawn debug camera components.
#[derive(Component)]
pub struct DebugCameraComponents;

/// Component that marks an entity as a debug target for the camera system.
/// When this component is added to any entity, the debug camera will automatically be enabled.
/// When no entities have this component, the debug camera will be disabled.
#[derive(Component)]
pub struct DebugCameraTarget {
    pub watermark_layer: Option<RenderLayers>,
}

impl Default for DebugCameraTarget {
    fn default() -> Self {
        Self {
            watermark_layer: Some(RenderLayers::layer(0)),
        }
    }
}

/// Component marking the rig for the debug camera control.
#[derive(Component)]
pub struct DebugCameraRig;

/// Marker indicating the CameraPose of the debug camera rig and storing restore points
/// to revert the effect of the debug mode.
#[derive(Component)]
pub struct DebugCameraRestoreData {
    saved_grab_state: CursorGrabMode,
    saved_cursor_visible: bool,
}

pub fn spawn_debug_camera(
    camera_q: Query<(&Transform, &DebugCameraTarget)>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    mut commands: Commands,
) -> Result<(), BevyError> {
    log::debug!("Spawning debug camera");

    let mut window = window_q.single_mut()?;
    let (target_camera, debug_config) = camera_q.single()?;

    let (yaw, pitch, _) = target_camera.rotation.to_euler(EulerRot::YXZ);
    let yaw = yaw.to_degrees();
    let pitch = pitch.to_degrees();

    let camera = {
        let mut rig = CameraRig::new()
            .with(rigs::Position::new(target_camera.translation.with_name("position")))?
            .with(rigs::YawPitch::new(yaw.with_name("yaw"), pitch.with_name("pitch")))?
            .with(rigs::Smooth::position_rotation(1.0, 1.0))?;
        (
            DebugCameraComponents,
            DebugCameraRig,
            DebugCameraRestoreData {
                saved_grab_state: window.cursor_options.grab_mode,
                saved_cursor_visible: window.cursor_options.visible,
            },
            rig.calculate_transform(0.0, None),
            rig,
        )
    };
    commands.spawn(camera);

    if let Some(layer) = &debug_config.watermark_layer {
        let watermark = (
            DebugCameraComponents,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                right: Val::Px(10.0),
                padding: UiRect::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            Text::new("Debug"),
            TextFont {
                font_size: 12.0,
                ..Default::default()
            },
            TextColor(Color::srgba(0.8, 0.8, 0.8, 0.7)),
            layer.clone(),
        );
        commands.spawn(watermark);

        let border_quad = (
            DebugCameraComponents,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(5.0),
                right: Val::Px(5.0),
                top: Val::Px(5.0),
                bottom: Val::Px(5.0),
                border: UiRect::all(Val::Px(2.0)),
                ..Default::default()
            },
            BorderColor(Color::srgba(1.0, 0.0, 0.0, 1.0)),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            layer.clone(),
        );
        commands.spawn(border_quad);
    }

    window.cursor_options.grab_mode = CursorGrabMode::Locked;
    window.cursor_options.visible = false;

    Ok(())
}

pub fn restore_debug_states(
    debug_q: Query<&DebugCameraRestoreData>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) -> Result<(), BevyError> {
    let mut window = window_q.single_mut()?;
    let restore_point = debug_q.single()?;

    window.cursor_options.grab_mode = restore_point.saved_grab_state;
    window.cursor_options.visible = restore_point.saved_cursor_visible;

    Ok(())
}

/// System that automatically toggles the debug camera based on the presence of DebugTarget components
pub fn auto_toggle_debug_camera(
    debug_targets: Query<&DebugCameraTarget>,
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
    camera_q: Query<&Transform, With<DebugCameraTarget>>,
    mut rig_q: Query<&mut CameraRig, With<DebugCameraRig>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    time: Res<Time>,
) -> Result<(), BevyError> {
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
            move_vec.y += 1.0;
        }
        if keyboard_input.pressed(KeyCode::Numpad7) {
            move_vec.y -= 1.0;
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

fn render_frustum(gizmos: &mut Gizmos, transform: &Transform, corners: &[Vec3A; 8], color: impl Into<Color>) {
    let [tln, trn, brn, bln, tlf, trf, brf, blf] = corners.map(|x| transform.transform_point(Vec3::from(x)));

    #[rustfmt::skip]
    let positions = [
        // Near
        tln, trn, brn, bln, tln,
        // Far
        tlf, trf, brf, blf, tlf,
        // Near to far
        Vec3::NAN, trn, trf,
        Vec3::NAN, tln, tlf,
        Vec3::NAN, brn, brf,
        Vec3::NAN, bln, blf,
    ];

    gizmos.linestrip(positions, color);
}

pub fn render_camera_gizmos(
    camera_q: Query<(&CameraPose, &Projection, Option<&CameraPoseDebug>), With<Camera>>,
    mut gizmos: Gizmos,
) {
    for (pose, projection, pose_debug) in camera_q.iter() {
        let near = match projection {
            Projection::Perspective(projection) => projection.near,
            Projection::Orthographic(projection) => projection.near,
            Projection::Custom(_) => 0.0,
        };
        let far = projection.far();

        let corners = projection.get_frustum_corners(-near, -far);
        render_frustum(&mut gizmos, &pose.transform, &corners, css::GREEN);

        if let Some(pose_debug) = pose_debug {
            let steps = &pose_debug.update_steps;
            for (step_id, step) in steps.iter().enumerate() {
                let color = {
                    let t = if steps.len() <= 1 {
                        1.0
                    } else {
                        step_id as f32 / (steps.len() - 1) as f32
                    };
                    Color::srgb(t, t, 0.0)
                };

                render_frustum(&mut gizmos, step, &corners, color);
            }
        }
    }
}
