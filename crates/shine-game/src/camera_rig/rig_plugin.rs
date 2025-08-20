use crate::{app::GameSystem, camera_rig::update_camera_pose};
use bevy::{
    app::{App, Plugin, Update},
    ecs::schedule::IntoScheduleConfigs,
};

pub struct CameraRigPlugin {}

impl Default for CameraRigPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraRigPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_camera_pose.after(GameSystem::Input).before(GameSystem::Logic),
        );
    }
}
