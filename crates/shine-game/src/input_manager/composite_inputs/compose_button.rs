use crate::input_manager::{ButtonLike, InputSources, UserInput};
use bevy::time::Time;
use std::borrow::Cow;

/// A button combination that return pressed state if either button is pressed.
pub struct ButtonAny<I1, I2>
where
    I1: ButtonLike,
    I2: ButtonLike,
{
    name: Option<String>,
    inputs: (I1, I2),
}

impl<I1, I2> ButtonAny<I1, I2>
where
    I1: ButtonLike,
    I2: ButtonLike,
{
    pub fn new(i1: I1, i2: I2) -> Self {
        Self { name: None, inputs: (i1, i2) }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I1, I2> UserInput for ButtonAny<I1, I2>
where
    I1: ButtonLike,
    I2: ButtonLike,
{
    fn type_name(&self) -> &'static str {
        "ButtonAny"
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

impl<I1, I2> ButtonLike for ButtonAny<I1, I2>
where
    I1: ButtonLike,
    I2: ButtonLike,
{
    fn process(&mut self, time: &Time) -> Option<bool> {
        let v0 = self.inputs.0.process(time).unwrap_or(false);
        let v1 = self.inputs.1.process(time).unwrap_or(false);
        Some(v0 | v1)
    }
}

pub trait ButtonCompose: Sized + ButtonLike {
    fn or<I2>(self, other: I2) -> ButtonAny<Self, I2>
    where
        I2: ButtonLike;
}

impl<I1> ButtonCompose for I1
where
    I1: ButtonLike + Sized,
{
    fn or<I2>(self, other: I2) -> ButtonAny<Self, I2>
    where
        I2: ButtonLike,
    {
        ButtonAny::new(self, other)
    }
}
