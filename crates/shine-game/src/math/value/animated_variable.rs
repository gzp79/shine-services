use crate::math::value::{ValueLike, Variable};
use bevy::math::{Quat, Vec2, Vec3, Vec4};

/// A time-varying value.
pub trait AnimatedVariable: Variable {
    type Value: ValueLike;

    fn animate(&mut self, delta_time_s: f32) -> Self::Value;
}

macro_rules! impl_animated_variable {
    ($target_type:ty) => {
        impl AnimatedVariable for $target_type {
            type Value = $target_type;

            #[inline(always)]
            fn animate(&mut self, _delta_time_s: f32) -> Self::Value {
                *self
            }
        }
    };
}

impl_animated_variable!(f32);
impl_animated_variable!(Vec2);
impl_animated_variable!(Vec3);
impl_animated_variable!(Vec4);
impl_animated_variable!(Quat);
