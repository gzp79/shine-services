use crate::camera_rig::{RigDriver, RigUpdateParams};
use bevy::{math::Vec3, transform::components::Transform};

/// Offsets the camera along a vector in the coordinate space of the parent.
pub struct Arm {
    pub offset: Vec3,
}

impl Arm {
    pub fn new<V>(offset: V) -> Self
    where
        V: Into<Vec3>,
    {
        let offset = offset.into();

        Self { offset }
    }
}

impl RigDriver for Arm {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let parent_position = params.parent.translation;
        let parent_rotation = params.parent.rotation;
        let offset: Vec3 = self.offset;

        let position = parent_position + parent_rotation * offset;

        Transform::from_translation(position).with_rotation(parent_rotation)
    }
}
