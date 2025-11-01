use bevy::{
    ecs::component::Component,
    math::{Quat, Vec3},
    transform::components::Transform,
};

#[derive(Component)]
pub struct CameraPose {
    pub transform: Transform,
}

impl Default for CameraPose {
    fn default() -> Self {
        Self::default_z_up()
    }
}

impl CameraPose {
    pub fn identity() -> Self {
        Self { transform: Transform::IDENTITY }
    }

    pub fn default_z_up() -> Self {
        Self {
            transform: Transform {
                translation: Vec3::ZERO,
                rotation: Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::Y),
                ..Default::default()
            },
        }
    }
}

/// Helper to visualize the rig update by storing each update step.
#[derive(Component, Default)]
pub struct CameraPoseTrace {
    pub update_steps: Vec<Transform>,
}
