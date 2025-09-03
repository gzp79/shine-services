use crate::{
    camera_rig::{RigDriver, RigError, RigUpdateParams},
    math::temporal::{TemporalValue, ValueError, ValueType},
};
use bevy::{math::Quat, transform::components::Transform};

/// Directly sets the rotation of the camera
pub struct Rotation<Q>
where
    Q: TemporalValue<Value = Quat>,
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
    Q: TemporalValue<Value = Quat>,
{
    pub fn new(rotation: Q) -> Self {
        Self { rotation }
    }
}

impl<Q> RigDriver for Rotation<Q>
where
    Q: TemporalValue<Value = Quat>,
{
    fn parameter_names(&self) -> Vec<&str> {
        self.rotation.name().into_iter().collect()
    }

    fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError> {
        if self.rotation.name() == Some(name) {
            self.rotation.set(Quat::try_from(value)?);
            Ok(())
        } else {
            Err(ValueError::UnknownParameter(name.into()).into())
        }
    }

    fn get_parameter_value(&self, name: &str) -> Result<ValueType, RigError> {
        if self.rotation.name() == Some(name) {
            Ok((*self.rotation.get()).into())
        } else {
            Err(ValueError::UnknownParameter(name.into()).into())
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let rot = self.rotation.update(params.delta_time_s);
        Transform::from_translation(params.parent.translation).with_rotation(rot)
    }
}
