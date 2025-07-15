use crate::camera_rig::{RigDriver, RigUpdateParams};
use bevy::{math::Quat, transform::components::Transform};

/// Directly sets the rotation of the camera
pub struct Rotation {
    pub rotation: Quat,
}

impl Default for Rotation {
    fn default() -> Self {
        Self::new(Quat::default())
    }
}

impl Rotation {
    pub fn new<Q>(rotation: Q) -> Self
    where
        Q: Into<Quat>,
    {
        let rotation = rotation.into();

        Self { rotation }
    }
}

impl RigDriver for Rotation {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        Transform::from_translation(params.parent.translation).with_rotation(self.rotation)
    }
}
