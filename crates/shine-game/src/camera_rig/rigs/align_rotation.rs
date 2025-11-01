use crate::{
    camera_rig::{CameraPose, RigDriver},
    math::value::{AnimatedVariable, Variable},
};
use bevy::math::Quat;

/// Align the rotation of the camera in world space to the target rotation.
pub struct AlignRotation<Q>
where
    Q: AnimatedVariable<Value = Quat>,
{
    pub rotation: Q,
}

impl Default for AlignRotation<Quat> {
    fn default() -> Self {
        Self::new(Quat::default())
    }
}

impl<Q> AlignRotation<Q>
where
    Q: AnimatedVariable<Value = Quat>,
{
    pub fn new(rotation: Q) -> Self {
        Self { rotation }
    }
}

impl<Q> RigDriver for AlignRotation<Q>
where
    Q: AnimatedVariable<Value = Quat>,
{
    fn visit_variables(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool) {
        visitor(&self.rotation);
    }

    fn variable_mut(&mut self, name: &str) -> Option<&mut dyn Variable> {
        if self.rotation.name() == Some(name) {
            Some(&mut self.rotation)
        } else {
            None
        }
    }

    fn update(&mut self, pose: &mut CameraPose, delta_time_s: f32) {
        let rot = self.rotation.animate(delta_time_s);
        pose.transform.rotation = rot;
    }
}
