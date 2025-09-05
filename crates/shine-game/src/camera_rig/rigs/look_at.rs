use crate::{
    camera_rig::{RigDriver, RigUpdateParams},
    math::value::{AnimatedVariable, Variable},
};
use bevy::{math::Vec3, transform::components::Transform};

/// Rotates the camera to point at a world-space position.
///
/// The target tracking can be additionally smoothed, and made to look ahead of it.
pub struct LookAt<T>
where
    T: Variable + AnimatedVariable<Value = Vec3>,
{
    target: T,
}

impl<T> LookAt<T>
where
    T: Variable + AnimatedVariable<Value = Vec3>,
{
    pub fn new(target: T) -> Self {
        Self { target }
    }
}

impl<T> RigDriver for LookAt<T>
where
    T: Variable + AnimatedVariable<Value = Vec3>,
{
    fn visit_parameters(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool) {
        visitor(&self.target);
    }

    fn parameter_mut(&mut self, name: &str) -> Option<&mut dyn Variable> {
        if self.target.name() == Some(name) {
            Some(&mut self.target)
        } else {
            None
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let target = self.target.animate(params.delta_time_s);

        let parent_position = params.parent.translation;
        Transform::from_translation(parent_position).looking_at(target, Vec3::Y)
    }
}
