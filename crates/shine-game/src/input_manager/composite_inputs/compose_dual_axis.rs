use crate::input_manager::{DualAxisLike, InputSources, UserInput};
use bevy::{math::Vec2, time::Time};
use std::borrow::Cow;

/// An dual axis combination that returns the value with the maximum length
/// from two axes.
pub struct DualAxisMax<I1, I2>
where
    I1: DualAxisLike,
    I2: DualAxisLike,
{
    name: Option<String>,
    inputs: (I1, I2),
}

impl<I1, I2> DualAxisMax<I1, I2>
where
    I1: DualAxisLike,
    I2: DualAxisLike,
{
    pub fn new(b1: I1, b2: I2) -> Self {
        Self { name: None, inputs: (b1, b2) }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I1, I2> UserInput for DualAxisMax<I1, I2>
where
    I1: DualAxisLike,
    I2: DualAxisLike,
{
    fn type_name(&self) -> &'static str {
        "DualAxisMax"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
            && self.inputs.0.visit_recursive(depth + 1, visitor)
            && self.inputs.1.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.inputs.0.integrate(input);
        self.inputs.1.integrate(input);
    }
}

impl<I1, I2> DualAxisLike for DualAxisMax<I1, I2>
where
    I1: DualAxisLike,
    I2: DualAxisLike,
{
    fn process(&mut self, time: &Time) -> Option<Vec2> {
        let v0 = self.inputs.0.process(time);
        let v1 = self.inputs.1.process(time);
        match (v0, v1) {
            (Some(v0), Some(v1)) => {
                if v0.length_squared() > v1.length_squared() {
                    Some(v0)
                } else {
                    Some(v1)
                }
            }
            (Some(v0), None) => Some(v0),
            (None, Some(v1)) => Some(v1),
            (None, None) => None,
        }
    }
}

pub trait DualAxisCompose: Sized + DualAxisLike {
    fn max<I2>(self, other: I2) -> DualAxisMax<Self, I2>
    where
        I2: DualAxisLike;
}

impl<I1> DualAxisCompose for I1
where
    I1: DualAxisLike + Sized,
{
    fn max<I2>(self, other: I2) -> DualAxisMax<Self, I2>
    where
        I2: DualAxisLike,
    {
        DualAxisMax::new(self, other)
    }
}
