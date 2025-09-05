use crate::{
    camera_rig::RigError,
    math::value::{ValueError, ValueLike, ValueType, Variable},
};
use bevy::transform::components::Transform;
use shine_core::utils::TypeErase;

pub struct RigUpdateParams<'a> {
    pub parent: &'a Transform,
    pub delta_time_s: f32,
}

/// A building block of a camera rig, to calculate the transform of the camera.
pub trait RigDriver: TypeErase {
    /// Iterate over each parameter of this driver.
    fn visit_parameters(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool);

    /// Find a parameter by name
    fn parameter_mut(&mut self, name: &str) -> Option<&mut dyn Variable>;

    /// Calculates the transform of this driver component based on the parent
    /// provided in `params`.
    fn update(&mut self, params: RigUpdateParams) -> Transform;
}

pub trait RigDriverExt: RigDriver {
    fn for_each_parameter(&self, mut visitor: impl FnMut(&dyn Variable) -> bool) {
        self.visit_parameters(&mut visitor);
    }

    fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError> {
        let param = self
            .parameter_mut(name)
            .ok_or(ValueError::UnknownParameter(name.to_string()))?;
        param.update(value)?;
        Ok(())
    }

    fn set_parameter<T>(&mut self, name: &str, value: T) -> Result<(), RigError>
    where
        T: ValueLike,
    {
        self.set_parameter_value(name, value.into())
    }

    fn set_parameter_value_with(
        &mut self,
        name: &str,
        update: impl Fn(ValueType) -> Result<ValueType, ValueError>,
    ) -> Result<(), RigError> {
        let param = self
            .parameter_mut(name)
            .ok_or(ValueError::UnknownParameter(name.to_string()))?;
        param.update_with(&update)?;
        Ok(())
    }

    fn set_parameter_with<T>(&mut self, name: &str, update: impl Fn(T) -> T) -> Result<(), RigError>
    where
        T: ValueLike,
    {
        self.set_parameter_value_with(name, &move |old_value| {
            let old_value: T = T::try_from(old_value)?;
            Ok(update(old_value).into())
        })
    }
}

impl<T> RigDriverExt for T where T: ?Sized + RigDriver {}
