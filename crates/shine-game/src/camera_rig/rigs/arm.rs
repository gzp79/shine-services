use crate::camera_rig::{RigDriver, RigError, RigParameter, RigUpdateParams, ValueType};
use bevy::{math::Vec3, transform::components::Transform};

/// Offsets the camera along a vector in the coordinate space of the parent.
pub struct Arm<A>
where
    A: RigParameter<Value = Vec3>,
{
    pub offset: A,
}

impl<A> Arm<A>
where
    A: RigParameter<Value = Vec3>,
{
    pub fn new(offset: A) -> Self {
        Self { offset }
    }
}

impl<A> RigDriver for Arm<A>
where
    A: RigParameter<Value = Vec3>,
{
    fn parameter_names(&self) -> Vec<&str> {
        self.offset.name().into_iter().collect()
    }

    fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError> {
        if self.offset.name() == Some(name) {
            self.offset.set(Vec3::try_from(value)?);
            Ok(())
        } else {
            Err(RigError::UnknownParameter(name.into()))
        }
    }

    fn get_parameter_value(&self, name: &str) -> Result<ValueType, RigError> {
        if self.offset.name() == Some(name) {
            Ok((*self.offset.get()).into())
        } else {
            Err(RigError::UnknownParameter(name.into()))
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let parent_position = params.parent.translation;
        let parent_rotation = params.parent.rotation;
        let offset: Vec3 = self.offset.update(params.delta_time_s);

        let position = parent_position + parent_rotation * offset;

        Transform::from_translation(position).with_rotation(parent_rotation)
    }
}
