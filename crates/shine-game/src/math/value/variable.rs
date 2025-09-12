use crate::math::value::{TweenLike, ValueError, ValueKind, ValueLike, ValueType};
use bevy::math::{Quat, Vec2, Vec3, Vec4};

/// A named parameter that can be read and written as a ValueType.
pub trait Variable: Send + Sync + 'static {
    fn name(&self) -> Option<&str>;
    fn kind(&self) -> ValueKind;

    fn get(&self) -> ValueType;
    fn update(&mut self, value: ValueType) -> Result<(), ValueError>;
    fn update_with(&mut self, update: &dyn Fn(ValueType) -> Result<ValueType, ValueError>) -> Result<(), ValueError>;
}

macro_rules! impl_variable {
    ($target_type:ty) => {
        impl Variable for $target_type {
            #[inline(always)]
            fn kind(&self) -> ValueKind {
                <Self as ValueLike>::KIND
            }

            #[inline(always)]
            fn name(&self) -> Option<&str> {
                None
            }

            #[inline(always)]
            fn get(&self) -> ValueType {
                ValueType::from(self.clone())
            }

            #[inline(always)]
            fn update(&mut self, value: ValueType) -> Result<(), ValueError> {
                *self = value.try_into()?;
                Ok(())
            }

            #[inline(always)]
            fn update_with(
                &mut self,
                update: &dyn Fn(ValueType) -> Result<ValueType, ValueError>,
            ) -> Result<(), ValueError> {
                let new_value = update(ValueType::from(self.clone()))?;
                *self = new_value.try_into()?;
                Ok(())
            }
        }
    };
}

impl_variable!(());
impl_variable!(f32);
impl_variable!(Vec2);
impl_variable!(Vec3);
impl_variable!(Vec4);
impl_variable!(Quat);

/// A value changing over time.
pub trait AnimatedVariable: Variable {
    type Value: TweenLike;

    fn animate(&mut self, delta_time_s: f32) -> Self::Value;
}

macro_rules! impl_animated {
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

impl_animated!(());
impl_animated!(f32);
impl_animated!(Vec2);
impl_animated!(Vec3);
impl_animated!(Vec4);
impl_animated!(Quat);
