use crate::math::value::{ValueError, ValueType};
use bevy::math::{Quat, Vec2, Vec3, Vec4};

/// A named parameter that can be updated from a 'ValueType'
pub trait Variable: Send + Sync + 'static {
    fn name(&self) -> Option<&str>;

    fn get(&self) -> ValueType;
    fn update(&mut self, value: ValueType) -> Result<(), ValueError>;
    fn update_with(&mut self, update: &dyn Fn(ValueType) -> Result<ValueType, ValueError>) -> Result<(), ValueError>;
}

macro_rules! impl_variable {
    ($target_type:ty) => {
        impl Variable for $target_type {
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

impl_variable!(f32);
impl_variable!(Vec2);
impl_variable!(Vec3);
impl_variable!(Vec4);
impl_variable!(Quat);
