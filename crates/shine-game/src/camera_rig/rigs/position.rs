use crate::{
    camera_rig::{RigDriver, RigError, RigUpdateParams},
    math::value::{TemporalValue, ValueError, ValueType},
};
use bevy::{math::Vec3, transform::components::Transform};

/// Directly sets the position of the camera
pub struct Position<P>
where
    P: TemporalValue<Value = Vec3>,
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
    P: TemporalValue<Value = Vec3>,
{
    pub fn new(position: P) -> Self {
        Self { position }
    }
}

impl<P> RigDriver for Position<P>
where
    P: TemporalValue<Value = Vec3>,
{
    fn parameter_names(&self) -> Vec<&str> {
        self.position.name().into_iter().collect()
    }

    fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError> {
        if self.position.name() == Some(name) {
            self.position.set(Vec3::try_from(value)?);
            Ok(())
        } else {
            Err(ValueError::UnknownParameter(name.into()).into())
        }
    }

    fn get_parameter_value(&self, name: &str) -> Result<ValueType, RigError> {
        if self.position.name() == Some(name) {
            Ok((*self.position.get()).into())
        } else {
            Err(ValueError::UnknownParameter(name.into()).into())
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let pos = self.position.update(params.delta_time_s);
        Transform::from_translation(pos).with_rotation(params.parent.rotation)
    }
}
