use crate::{
    camera_rig::{CameraPose, RigDriver},
    math::value::{AnimatedVariable, Variable},
};
use bevy::math::Vec3;

/// Sets the position of the camera relative to the current pose.
pub struct Position<P>
where
    P: AnimatedVariable<Value = Vec3>,
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
    P: AnimatedVariable<Value = Vec3>,
{
    pub fn new(position: P) -> Self {
        Self { position }
    }
}

impl<P> RigDriver for Position<P>
where
    P: AnimatedVariable<Value = Vec3>,
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

    fn update(&mut self, pose: &mut CameraPose, delta_time_s: f32) {
        let pos = self.position.animate(delta_time_s);
        pose.transform.translation += pos;
    }
}
