use crate::{
    camera_rig::{CameraPose, RigDriver},
    math::value::{AnimatedVariable, Variable},
};
use bevy::math::Vec3;

/// Align the position of the camera in world space to the target position.
pub struct AlignPosition<P>
where
    P: AnimatedVariable<Value = Vec3>,
{
    position: P,
}

impl Default for AlignPosition<Vec3> {
    fn default() -> Self {
        Self::new(Vec3::ZERO)
    }
}

impl<P> AlignPosition<P>
where
    P: AnimatedVariable<Value = Vec3>,
{
    pub fn new(position: P) -> Self {
        Self { position }
    }
}

impl<P> RigDriver for AlignPosition<P>
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
        pose.transform.translation = pos;
    }
}
