use crate::input_manager::{InputDrivers, InputProcessor, TypedInputProcessor};
use std::borrow::Cow;

/// An axis that returns value only when the button is pressed.
pub struct AxisChord<B, A>
where
    B: TypedInputProcessor<bool>,
    A: TypedInputProcessor<f32>,
{
    name: Option<String>,
    button: B,
    axis: A,
}

impl<B, A> AxisChord<B, A>
where
    B: TypedInputProcessor<bool>,
    A: TypedInputProcessor<f32>,
{
    pub fn new(button: B, axis: A) -> Self {
        Self { name: None, button, axis }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<B, A> InputProcessor for AxisChord<B, A>
where
    B: TypedInputProcessor<bool>,
    A: TypedInputProcessor<f32>,
{
    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(
        &'a self,
        depth: usize,
        visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool,
    ) -> bool {
        visitor(depth, self)
            && self.button.visit_recursive(depth + 1, visitor)
            && self.axis.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputDrivers) {
        self.button.integrate(input);
        self.axis.integrate(input);
    }
}

impl<B, A> TypedInputProcessor<f32> for AxisChord<B, A>
where
    B: TypedInputProcessor<bool>,
    A: TypedInputProcessor<f32>,
{
    fn process(&mut self, time_s: f32) -> Option<f32> {
        let button = self.button.process(time_s).unwrap_or(false);
        let value = self.axis.process(time_s);

        if button {
            value
        } else {
            None
        }
    }
}
