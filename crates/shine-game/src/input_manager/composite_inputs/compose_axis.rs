use crate::input_manager::{AxisLike, InputSources, UserInput};
use bevy::time::Time;
use std::borrow::Cow;

/// An axis combination that returns the maximum value from two axes.
pub struct AxisMax<I1, I2>
where
    I1: AxisLike,
    I2: AxisLike,
{
    name: Option<String>,
    inputs: (I1, I2),
}

impl<I1, I2> AxisMax<I1, I2>
where
    I1: AxisLike,
    I2: AxisLike,
{
    pub fn new(i1: I1, i2: I2) -> Self {
        Self { name: None, inputs: (i1, i2) }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I1, I2> UserInput for AxisMax<I1, I2>
where
    I1: AxisLike,
    I2: AxisLike,
{
    fn type_name(&self) -> &'static str {
        "AxisMax"
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

impl<I1, I2> AxisLike for AxisMax<I1, I2>
where
    I1: AxisLike,
    I2: AxisLike,
{
    fn process(&mut self, time: &Time) -> Option<f32> {
        let v0 = self.inputs.0.process(time);
        let v1 = self.inputs.1.process(time);
        match (v0, v1) {
            (Some(v0), Some(v1)) => Some(v0.max(v1)),
            (Some(v0), None) => Some(v0),
            (None, Some(v1)) => Some(v1),
            (None, None) => None,
        }
    }
}

pub trait AxisCompose: Sized + AxisLike {
    fn max<I2>(self, other: I2) -> AxisMax<Self, I2>
    where
        I2: AxisLike;
}

impl<I1> AxisCompose for I1
where
    I1: AxisLike + Sized,
{
    fn max<I2>(self, other: I2) -> AxisMax<Self, I2>
    where
        I2: AxisLike,
    {
        AxisMax::new(self, other)
    }
}
