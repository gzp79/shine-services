use crate::math::value::{
    Interpolate, NamedValue, PredictedValue, SmoothedValue, ValueError, ValueKind, ValueLike, ValueType,
};
use bevy::math::{Quat, Vec2, Vec3, Vec4};
use std::borrow::Cow;

pub trait TemporalValue: Send + Sync + 'static {
    type Value: ValueLike;

    fn name(&self) -> Option<&str>;

    fn set(&mut self, value: Self::Value);
    fn get(&self) -> &Self::Value;
    fn update(&mut self, delta_time_s: f32) -> Self::Value;
}

/// Extension methods for TemporalValue
pub trait TemporalValueExt: TemporalValue {
    fn kind(&self) -> ValueKind {
        Self::Value::KIND
    }

    fn set_value(&mut self, value: ValueType) -> Result<(), ValueError> {
        let value = Self::Value::try_from(value)?;
        self.set(value);
        Ok(())
    }

    fn set_value_with(&mut self, f: &dyn Fn(ValueType) -> ValueType) -> Result<(), ValueError> {
        let value: ValueType = self.get().clone().into();
        self.set_value(f(value))
    }

    fn with_name(self, name: impl Into<Cow<'static, str>>) -> NamedValue<Self>
    where
        Self: Sized + 'static,
    {
        NamedValue::new(name, self)
    }

    fn smoothed(self, duration_s: f32) -> SmoothedValue<Self>
    where
        Self: Sized + 'static,
        Self::Value: Interpolate,
    {
        SmoothedValue::new(self, duration_s)
    }

    fn predicted(self, duration_s: f32) -> PredictedValue<Self>
    where
        Self: Sized + 'static,
        Self::Value: Interpolate,
    {
        PredictedValue::new(self, duration_s)
    }
}

impl<P> TemporalValueExt for P where P: TemporalValue {}

macro_rules! impl_rig_parameter_for_value_type {
    ($type:ty, $kind:ident) => {
        impl TemporalValue for $type {
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

// Implement TemporalValue for basic value types
impl_rig_parameter_for_value_type!(f32, Float);
impl_rig_parameter_for_value_type!(Vec2, Vec2);
impl_rig_parameter_for_value_type!(Vec3, Vec3);
impl_rig_parameter_for_value_type!(Vec4, Vec4);
impl_rig_parameter_for_value_type!(Quat, Quat);
