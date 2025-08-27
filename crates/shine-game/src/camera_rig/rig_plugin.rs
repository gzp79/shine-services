use crate::{
    app::{AppGameSchedule, CameraSimulate, GameSystem},
    bevy_ext::systems,
    camera_rig::{
        debug_camera_plugin::{
            auto_toggle_debug_camera, handle_debug_inputs, render_camera_gizmos, restore_debug_states,
            spawn_debug_camera, CameraDebugState, DebugCameraComponents,
        },
        rig::{update_camera_pose, update_camera_transform, update_debug_camera_transform},
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

pub struct CameraRigPlugin {
    pub enable_debug: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for CameraRigPlugin {
    fn default() -> Self {
        Self { enable_debug: false }
    }
}

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.add_update_systems(CameraSimulate::SimulatePose, update_camera_pose);
        app.add_update_systems(CameraSimulate::WithPose, update_camera_transform);

        if self.enable_debug {
            app.init_state::<CameraDebugState>();

            app.add_systems(PreUpdate, auto_toggle_debug_camera);
            app.add_systems(OnEnter(CameraDebugState::Enabled), spawn_debug_camera);
            app.add_systems(
                OnExit(CameraDebugState::Enabled),
                (restore_debug_states, systems::despawn_tagged::<DebugCameraComponents>).chain(),
            );

            app.add_update_systems(
                CameraSimulate::PreparePose,
                handle_debug_inputs.run_if(in_state(CameraDebugState::Enabled)),
            );
            app.add_update_systems(
                CameraSimulate::WithPose,
                update_debug_camera_transform.run_if(in_state(CameraDebugState::Enabled)),
            );
            app.add_update_systems(
                GameSystem::PrepareRender,
                (render_camera_gizmos.after(update_camera_transform)).run_if(in_state(CameraDebugState::Enabled)),
            );
        }
    }
}
