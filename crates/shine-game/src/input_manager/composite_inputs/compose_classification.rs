use crate::input_manager::{ClassificationLike, InputSources, UserInput};
use bevy::time::Time;
use std::borrow::Cow;

/// A classification combination that returns the maximum value from two classifications.
pub struct ClassificationMax<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    name: Option<String>,
    inputs: (I1, I2),
}

impl<I1, I2> ClassificationMax<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    pub fn new(i1: I1, i2: I2) -> Self {
        Self { name: None, inputs: (i1, i2) }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I1, I2> UserInput for ClassificationMax<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    fn type_name(&self) -> &'static str {
        "ClassificationMax"
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

impl<I1, I2> ClassificationLike for ClassificationMax<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    fn process(&mut self, time: &Time) -> Option<(usize, f32)> {
        let v0 = self.inputs.0.process(time);
        let v1 = self.inputs.1.process(time);
        match (v0, v1) {
            (Some(v0), Some(v1)) => {
                if v0.1 > v1.1 {
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

/// A classification combination that returns the minimum value from two classifications.
pub struct ClassificationMin<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    name: Option<String>,
    inputs: (I1, I2),
}

impl<I1, I2> ClassificationMin<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    pub fn new(i1: I1, i2: I2) -> Self {
        Self { name: None, inputs: (i1, i2) }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I1, I2> UserInput for ClassificationMin<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    fn type_name(&self) -> &'static str {
        "ClassificationMin"
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

impl<I1, I2> ClassificationLike for ClassificationMin<I1, I2>
where
    I1: ClassificationLike,
    I2: ClassificationLike,
{
    fn process(&mut self, time: &Time) -> Option<(usize, f32)> {
        let v0 = self.inputs.0.process(time);
        let v1 = self.inputs.1.process(time);
        match (v0, v1) {
            (Some(v0), Some(v1)) => {
                if v0.1 < v1.1 {
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

pub trait ClassificationCompose: Sized + ClassificationLike {
    fn max<I2>(self, other: I2) -> ClassificationMax<Self, I2>
    where
        I2: ClassificationLike;

    fn min<I2>(self, other: I2) -> ClassificationMax<Self, I2>
    where
        I2: ClassificationLike;
}

impl<I1> ClassificationCompose for I1
where
    I1: ClassificationLike + Sized,
{
    fn max<I2>(self, other: I2) -> ClassificationMax<Self, I2>
    where
        I2: ClassificationLike,
    {
        ClassificationMax::new(self, other)
    }

    fn min<I2>(self, other: I2) -> ClassificationMax<Self, I2>
    where
        I2: ClassificationLike,
    {
        ClassificationMax::new(self, other)
    }
}
