use bevy::{ecs::component::Component, transform::components::Transform};

#[derive(Component, Default)]
pub struct CameraPose {
    pub transform: Transform,
}
