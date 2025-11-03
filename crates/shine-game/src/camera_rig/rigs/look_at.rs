use crate::{
    camera_rig::{CameraPose, RigDriver},
    math::value::{AnimatedVariable, Variable},
};
use bevy::math::Vec3;

/// Rotates the camera to point at a world-space position with z as up direction.
pub struct LookAt<T>
where
    T: AnimatedVariable<Value = Vec3>,
{
    target: T,
}

impl<T> LookAt<T>
where
    T: AnimatedVariable<Value = Vec3>,
{
    pub fn new(target: T) -> Self {
        Self { target }
    }
}

impl<T> RigDriver for LookAt<T>
where
    T: AnimatedVariable<Value = Vec3>,
{
    fn visit_variables(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool) {
        visitor(&self.target);
    }

    fn variable_mut(&mut self, name: &str) -> Option<&mut dyn Variable> {
        if self.target.name() == Some(name) {
            Some(&mut self.target)
        } else {
            None
        }
    }

    fn update(&mut self, pose: &mut CameraPose, delta_time_s: f32) {
        let target = self.target.animate(delta_time_s);

        pose.transform = pose.transform.looking_at(target, Vec3::Z);
    }
}
