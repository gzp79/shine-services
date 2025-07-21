use crate::input_manager::{AxisLike, ButtonLike, InputSources, UserInput};
use bevy::time::Time;

/// An axis that returns value only when the button is pressed.
pub struct AxisChord<B, A>
where
    B: ButtonLike,
    A: AxisLike,
{
    name: Option<String>,
    button: B,
    axis: A,
}

impl<B, A> AxisChord<B, A>
where
    B: ButtonLike,
    A: AxisLike,
{
    pub fn new(button: B, axis: A) -> Self {
        Self { name: None, button, axis }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<B, A> UserInput for AxisChord<B, A>
where
    B: ButtonLike,
    A: AxisLike,
{
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        if self.name.as_deref() == Some(name) {
            Some(self)
        } else {
            self.button.find(name).or_else(|| self.axis.find(name))
        }
    }

    fn integrate(&mut self, input: &InputSources) {
        self.button.integrate(input);
        self.axis.integrate(input);
    }
}

impl<B, A> AxisLike for AxisChord<B, A>
where
    B: ButtonLike,
    A: AxisLike,
{
    fn process(&mut self, time: &Time) -> Option<f32> {
        let button = self.button.process(time).unwrap_or(false);
        let value = self.axis.process(time);

        if button {
            value
        } else {
            None
        }
    }
}
