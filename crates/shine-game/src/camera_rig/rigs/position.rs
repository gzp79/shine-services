use crate::{
    camera_rig::{RigDriver, RigUpdateParams},
    math::value::{Animated, Variable},
};
use bevy::{math::Vec3, transform::components::Transform};

/// Directly sets the position of the camera
pub struct Position<P>
where
    P: Animated<Value = Vec3>,
{
    position: P,
}

impl Default for Position<Vec3> {
    fn default() -> Self {
        Self::new(Vec3::ZERO)
    }
}

impl<P> Position<P>
where
    P: Animated<Value = Vec3>,
{
    pub fn new(position: P) -> Self {
        Self { position }
    }
}

impl<P> RigDriver for Position<P>
where
    P: Animated<Value = Vec3>,
{
    fn visit_variables(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool) {
        visitor(&self.position);
    }

    fn variable_mut(&mut self, name: &str) -> Option<&mut dyn Variable> {
        if self.position.name() == Some(name) {
            Some(&mut self.position)
        } else {
            None
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let pos = self.position.animate(params.delta_time_s);
        Transform::from_translation(pos).with_rotation(params.parent.rotation)
    }
}
