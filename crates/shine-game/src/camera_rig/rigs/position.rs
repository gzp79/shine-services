use crate::camera_rig::{RigDriver, RigUpdateParams};
use bevy::{math::Vec3, transform::components::Transform};

/// Directly sets the position of the camera
pub struct Position {
    pub position: Vec3,
}

impl Default for Position {
    fn default() -> Self {
        Self { position: Vec3::ZERO }
    }
}

impl Position {
    pub fn new<P>(position: P) -> Self
    where
        P: Into<Vec3>,
    {
        let position = position.into();

        Self { position }
    }

    /// Add the specified vector to the position of this component
    pub fn translate<V>(&mut self, move_vec: V)
    where
        V: Into<Vec3>,
    {
        let position = self.position;
        let move_vec = move_vec.into();
        self.position = position + move_vec;
    }
}

impl RigDriver for Position {
    fn update(&mut self, params: RigUpdateParams) -> Transform {
        Transform::from_translation(self.position).with_rotation(params.parent.rotation)
    }
}
