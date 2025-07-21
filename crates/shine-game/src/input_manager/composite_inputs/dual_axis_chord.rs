use crate::input_manager::{ButtonLike, DualAxisLike, InputSources, UserInput};
use bevy::{math::Vec2, time::Time};

/// A dual axis that returns value only when the button is pressed.
pub struct DualAxisChord<B, D>
where
    B: ButtonLike,
    D: DualAxisLike,
{
    name: Option<String>,
    button: B,
    dual_axis: D,
}

impl<B, D> DualAxisChord<B, D>
where
    B: ButtonLike,
    D: DualAxisLike,
{
    pub fn new(button: B, dual_axis: D) -> Self {
        Self { name: None, button, dual_axis }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<B, D> UserInput for DualAxisChord<B, D>
where
    B: ButtonLike,
    D: DualAxisLike,
{
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        if self.name.as_deref() == Some(name) {
            Some(self)
        } else {
            self.button.find(name).or_else(|| self.dual_axis.find(name))
        }
    }

    fn integrate(&mut self, input: &InputSources) {
        self.button.integrate(input);
        self.dual_axis.integrate(input);
    }
}

impl<B, D> DualAxisLike for DualAxisChord<B, D>
where
    B: ButtonLike,
    D: DualAxisLike,
{
    fn process(&mut self, time: &Time) -> Option<Vec2> {
        let button = self.button.process(time).unwrap_or(false);
        let value = self.dual_axis.process(time);

        if button {
            value
        } else {
            None
        }
    }
}
