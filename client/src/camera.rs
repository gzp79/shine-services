use bevy::{
    app::{App, Plugin, Update},
    core_pipeline::core_3d::Camera3d,
    ecs::{
        query::With,
        schedule::IntoScheduleConfigs,
        system::{Commands, Query, Res},
    },
    input::keyboard::KeyCode,
    math::Vec3,
    reflect::Reflect,
    render::view::NoIndirectDrawing,
    state::{
        condition::in_state,
        state::{OnEnter, States},
    },
    transform::components::Transform,
};
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
enum CameraAction {
    #[actionlike(DualAxis)]
    Move,
    #[actionlike(DualAxis)]
    Look,
}

pub struct CameraPlugin<S: States> {
    pub state: S,
}

impl<S: States> Plugin for CameraPlugin<S> {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<CameraAction>::default());

        app.add_systems(OnEnter(self.state.clone()), spawn_camera);
        app.add_systems(Update, (camera_control_system).run_if(in_state(self.state.clone())));
    }
}

fn spawn_camera(mut commands: Commands) {
    let input_map = InputMap::default()
        .with_dual_axis(
            CameraAction::Move,
            VirtualDPad::new(KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD),
        )
        .with_dual_axis(CameraAction::Look, MouseMove::default());

    commands.spawn((
        Camera3d::default(),
        NoIndirectDrawing, //todo: https://github.com/bevyengine/bevy/issues/19209
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        input_map,
    ));
}

fn camera_control_system(
    mut query: Query<(&mut Transform, &ActionState<CameraAction>), With<Camera3d>>,
    time: Res<bevy::prelude::Time>,
) {
    let speed = 5.0;
    for (mut transform, action_state) in &mut query {
        // Get the axis pair for movement (X: left/right, Y: forward/backward)
        let axis = action_state.axis_pair(&CameraAction::Move);
        let x = axis.x;
        let y = axis.y;

        // move the camera parallel to the ground plane
        if x != 0.0 || y != 0.0 {
            // Get the forward and right vectors
            let mut forward = Vec3::from(transform.forward());
            forward.y = 0.0; // Zero out Y to keep movement parallel to ground
            if forward.length_squared() > 0.0 {
                forward = forward.normalize();
            }
            let right = transform.right();

            let move_vec = (forward * y + right * x).normalize_or_zero();
            transform.translation += move_vec * speed * time.delta_secs();
        }

        let look = action_state.axis_pair(&CameraAction::Look);

        // handle look around
        let sensitivity = 0.2;
        if look.x != 0.0 || look.y != 0.0 {
            // Yaw (around global Y axis)
            let yaw = -look.x * sensitivity * time.delta_secs();
            transform.rotate_y(yaw);

            // Pitch (around camera's local X axis)
            /*let pitch = -look.y * sensitivity * time.delta_secs();
            let right = transform.rotation * Vec3::X;
            transform.rotate_axis(right.into_dir(), pitch);*/
        }
    }
}
