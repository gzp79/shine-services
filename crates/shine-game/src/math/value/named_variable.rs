use crate::math::value::{AnimatedVariable, ValueError, ValueType, Variable};
use bevy::math::{Quat, Vec2, Vec3, Vec4};
use std::borrow::Cow;

pub struct NamedVariable<T>
where
    T: Variable,
{
    name: Option<Cow<'static, str>>,
    value: T,
}

impl<T> NamedVariable<T>
where
    T: Variable,
{
    pub fn new(name: impl Into<Cow<'static, str>>, value: T) -> Self {
        Self { name: Some(name.into()), value }
    }

    pub fn unnamed(value: T) -> Self {
        Self { name: None, value }
    }
}

impl<T> Variable for NamedVariable<T>
where
    T: Variable,
{
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn get(&self) -> ValueType {
        self.value.get()
    }

    fn update(&mut self, value: ValueType) -> Result<(), ValueError> {
        self.value.update(value)
    }

    fn update_with(&mut self, update: &dyn Fn(ValueType) -> Result<ValueType, ValueError>) -> Result<(), ValueError> {
        self.value.update_with(update)
    }
}

impl<T> AnimatedVariable for NamedVariable<T>
where
    T: AnimatedVariable,
{
    type Value = T::Value;

    fn animate(&mut self, delta_time_s: f32) -> Self::Value {
        self.value.animate(delta_time_s)
    }
}

pub trait WithNameVariable: Variable {
    fn with_name(self, name: impl Into<Cow<'static, str>>) -> NamedVariable<Self>
    where
        Self: Sized,
    {
        NamedVariable::new(name, self)
    }
}

impl WithNameVariable for f32 {}
impl WithNameVariable for Vec2 {}
impl WithNameVariable for Vec3 {}
impl WithNameVariable for Vec4 {}
impl WithNameVariable for Quat {}
