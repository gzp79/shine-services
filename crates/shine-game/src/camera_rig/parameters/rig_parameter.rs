use crate::{
    camera_rig::{NamedParameter, PredictedParameter, RigError, SmoothedParameter, ValueKind, ValueLike, ValueType},
    math::interpolate::Interpolate,
};
use bevy::math::{Quat, Vec2, Vec3, Vec4};
use std::borrow::Cow;

pub trait RigParameter: Send + Sync + 'static {
    type Value: ValueLike;

    fn name(&self) -> Option<&str>;

    fn set(&mut self, value: Self::Value);
    fn get(&self) -> &Self::Value;
    fn update(&mut self, delta_time_s: f32) -> Self::Value;
}

/// Extension methods for RigParameter
pub trait RigParameterExt: RigParameter {
    fn kind(&self) -> ValueKind {
        Self::Value::KIND
    }

    fn set_value(&mut self, value: ValueType) -> Result<(), RigError> {
        let value = Self::Value::try_from(value)?;
        self.set(value);
        Ok(())
    }

    fn set_value_with(&mut self, f: &dyn Fn(ValueType) -> ValueType) -> Result<(), RigError> {
        let value: ValueType = self.get().clone().into();
        self.set_value(f(value))
    }

    fn with_name(self, name: impl Into<Cow<'static, str>>) -> NamedParameter<Self>
    where
        Self: Sized + 'static,
    {
        NamedParameter::new(name, self)
    }

    fn smoothed(self, duration_s: f32) -> SmoothedParameter<Self>
    where
        Self: Sized + 'static,
        Self::Value: Interpolate,
    {
        SmoothedParameter::new(self, duration_s)
    }

    fn predicted(self, duration_s: f32) -> PredictedParameter<Self>
    where
        Self: Sized + 'static,
        Self::Value: Interpolate,
    {
        PredictedParameter::new(self, duration_s)
    }
}

impl<P> RigParameterExt for P where P: RigParameter {}

macro_rules! impl_rig_parameter_for_value_type {
    ($type:ty, $kind:ident) => {
        impl RigParameter for $type {
            type Value = $type;

            fn name(&self) -> Option<&str> {
                None
            }

            fn set(&mut self, value: $type) {
                *self = value;
            }

            fn get(&self) -> &$type {
                self
            }

            fn update(&mut self, _delta_time_s: f32) -> $type {
                self.clone()
            }
        }
    };
}

// Implement RigParameter for basic value types
impl_rig_parameter_for_value_type!(f32, Float);
impl_rig_parameter_for_value_type!(Vec2, Vec2);
impl_rig_parameter_for_value_type!(Vec3, Vec3);
impl_rig_parameter_for_value_type!(Vec4, Vec4);
impl_rig_parameter_for_value_type!(Quat, Quat);
