use crate::{
    camera_rig::{RigDriver, RigUpdateParams},
    math::value::{AnimatedVariable, Variable},
};
use bevy::{math::Quat, transform::components::Transform};

/// Directly sets the rotation of the camera
pub struct Rotation<Q>
where
    Q: Variable + AnimatedVariable<Value = Quat>,
{
    pub rotation: Q,
}

impl Default for Rotation<Quat> {
    fn default() -> Self {
        Self::new(Quat::default())
    }
}

impl<Q> Rotation<Q>
where
    Q: Variable + AnimatedVariable<Value = Quat>,
{
    pub fn new(rotation: Q) -> Self {
        Self { rotation }
    }
}

impl<Q> RigDriver for Rotation<Q>
where
    Q: Variable + AnimatedVariable<Value = Quat>,
{
    fn visit_parameters(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool) {
        visitor(&self.rotation);
    }

    fn parameter_mut(&mut self, name: &str) -> Option<&mut dyn Variable> {
        if self.rotation.name() == Some(name) {
            Some(&mut self.rotation)
        } else {
            None
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let rot = self.rotation.animate(params.delta_time_s);
        Transform::from_translation(params.parent.translation).with_rotation(rot)
    }
}
