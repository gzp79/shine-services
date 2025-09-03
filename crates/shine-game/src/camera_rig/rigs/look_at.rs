use crate::{
    camera_rig::{RigDriver, RigError, RigUpdateParams},
    math::temporal::{TemporalValue, ValueError, ValueType},
};
use bevy::{math::Vec3, transform::components::Transform};

/// Rotates the camera to point at a world-space position.
///
/// The target tracking can be additionally smoothed, and made to look ahead of it.
pub struct LookAt<T>
where
    T: TemporalValue<Value = Vec3>,
{
    target: T,
}

impl<T> LookAt<T>
where
    T: TemporalValue<Value = Vec3>,
{
    pub fn new(target: T) -> Self {
        Self { target }
    }
}

impl<T> RigDriver for LookAt<T>
where
    T: TemporalValue<Value = Vec3>,
{
    fn parameter_names(&self) -> Vec<&str> {
        self.target.name().into_iter().collect()
    }

    fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError> {
        if self.target.name() == Some(name) {
            self.target.set(Vec3::try_from(value)?);
            Ok(())
        } else {
            Err(ValueError::UnknownParameter(name.into()).into())
        }
    }

    fn get_parameter_value(&self, name: &str) -> Result<ValueType, RigError> {
        if self.target.name() == Some(name) {
            Ok((*self.target.get()).into())
        } else {
            Err(ValueError::UnknownParameter(name.into()).into())
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let target = self.target.update(params.delta_time_s);

        let parent_position = params.parent.translation;
        Transform::from_translation(parent_position).looking_at(target, Vec3::Y)
    }
}
