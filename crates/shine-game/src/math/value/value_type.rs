use crate::math::value::ValueError;
use bevy::math::{Quat, Vec2, Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Quat,
}

pub trait ValueLike: TryFrom<ValueType, Error = ValueError> + Into<ValueType> + Clone + Send + Sync + 'static {
    const KIND: ValueKind;
}

macro_rules! impl_try_from_value_kind {
    ($target_type:ty, $variant:ident) => {
        impl ValueLike for $target_type {
            const KIND: ValueKind = ValueKind::$variant;
        }
    };
}

impl_try_from_value_kind!(f32, Float);
impl_try_from_value_kind!(Vec2, Vec2);
impl_try_from_value_kind!(Vec3, Vec3);
impl_try_from_value_kind!(Vec4, Vec4);
impl_try_from_value_kind!(Quat, Quat);

#[derive(Clone, Debug)]
pub enum ValueType {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
    Quat(Quat),
}

impl ValueType {
    pub fn kind(&self) -> ValueKind {
        match self {
            ValueType::Float(_) => ValueKind::Float,
            ValueType::Vec2(_) => ValueKind::Vec2,
            ValueType::Vec3(_) => ValueKind::Vec3,
            ValueType::Vec4(_) => ValueKind::Vec4,
            ValueType::Quat(_) => ValueKind::Quat,
        }
    }
}

macro_rules! impl_try_from_value_type {
    ($target_type:ty, $variant:ident) => {
        impl From<$target_type> for ValueType {
            #[inline(always)]
            fn from(value: $target_type) -> Self {
                ValueType::$variant(value)
            }
        }

        impl TryFrom<ValueType> for $target_type {
            type Error = ValueError;

            #[inline(always)]
            fn try_from(value: ValueType) -> Result<Self, Self::Error> {
                match value {
                    ValueType::$variant(v) => Ok(v),
                    value => Err(ValueError::TypeMismatch {
                        expected: ValueKind::$variant,
                        found: value.kind(),
                    }),
                }
            }
        }
    };
}

impl_try_from_value_type!(f32, Float);
impl_try_from_value_type!(Vec2, Vec2);
impl_try_from_value_type!(Vec3, Vec3);
impl_try_from_value_type!(Vec4, Vec4);
impl_try_from_value_type!(Quat, Quat);
