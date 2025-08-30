use crate::camera_rig::{RigError, ValueLike, ValueType};
use bevy::transform::components::Transform;
use shine_core::utils::TypeErase;

pub struct RigUpdateParams<'a> {
    pub parent: &'a Transform,
    pub delta_time_s: f32,
}

/// A building block of a camera rig, to calculate the transform of the camera.
pub trait RigDriver: std::any::Any {
    /// Returns the name and type of the parameters this driver.
    fn parameter_names(&self) -> Vec<&str>;

    /// Sets a parameter by name. If the parameter is not found or the
    /// value is of the wrong type, an error is returned.
    fn set_parameter_value(&mut self, name: &str, value: ValueType) -> Result<(), RigError>;

    /// Gets a parameter by name. If the parameter is not found, an error is returned.
    fn get_parameter_value(&self, name: &str) -> Result<ValueType, RigError>;

    /// Calculates the transform of this driver component based on the parent
    /// provided in `params`.
    fn update(&mut self, params: RigUpdateParams) -> Transform;
}

/// Type erase RigDriver
pub trait AnyRigDriver: TypeErase + RigDriver {}
impl<T> AnyRigDriver for T where T: RigDriver + TypeErase {}

pub trait RigDriverExt: AnyRigDriver {
    fn set_parameter<T>(&mut self, name: &str, value: T) -> Result<(), RigError>
    where
        T: ValueLike,
    {
        self.set_parameter_value(name, value.into())
    }

    fn set_parameter_value_with(&mut self, name: &str, f: impl FnOnce(ValueType) -> ValueType) -> Result<(), RigError> {
        self.set_parameter_value(name, f(self.get_parameter_value(name)?))
    }

    fn set_parameter_with<T>(&mut self, name: &str, f: impl FnOnce(T) -> T) -> Result<(), RigError>
    where
        T: ValueLike,
    {
        let old_value = self.get_parameter_value(name)?;
        let old_value: T = T::try_from(old_value)?;
        self.set_parameter_value(name, f(old_value).into())
    }
}
impl<T: ?Sized> RigDriverExt for T where T: AnyRigDriver {}
