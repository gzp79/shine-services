use crate::{
    app::{AppGameSchedule, CameraSimulate, GameSystem},
    camera_rig::{
        auto_toggle_debug_camera, despawn_debug_camera, handle_debug_inputs, render_camera_gizmos, spawn_debug_camera,
        update_camera_pose, update_camera_transform, update_debug_camera_transform, CameraDebugState,
    },
};
use bevy::{
    app::{App, Plugin, PreUpdate},
    ecs::schedule::IntoScheduleConfigs,
    state::{
        app::AppExtStates,
        condition::in_state,
        state::{OnEnter, OnExit},
    },
};

pub struct CameraRigPlugin;

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<CameraDebugState>();

        app.add_systems(PreUpdate, auto_toggle_debug_camera);
        app.add_systems(OnEnter(CameraDebugState::Enabled), spawn_debug_camera);
        app.add_systems(OnExit(CameraDebugState::Enabled), despawn_debug_camera);

        app.add_update_systems(
            CameraSimulate::PreparePose,
            handle_debug_inputs.run_if(in_state(CameraDebugState::Enabled)),
        );
        app.add_update_systems(CameraSimulate::SimulatePose, update_camera_pose);
        app.add_update_systems(
            CameraSimulate::WithPose,
            (
                update_camera_transform,
                update_debug_camera_transform.run_if(in_state(CameraDebugState::Enabled)),
            ),
        );

        app.add_update_systems(
            GameSystem::PrepareRender,
            render_camera_gizmos.after(update_camera_transform),
        );
    }
}
