use bevy::{ecs::component::Component, transform::components::Transform};

#[derive(Component, Default)]
pub struct CameraPose {
    pub transform: Transform,
}

/// Helper to visualize the rig update by storing each transformation update step.
#[derive(Component, Default)]
pub struct CameraPoseDebug {
    pub update_steps: Vec<Transform>,
}
