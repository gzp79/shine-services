use crate::input_manager::{AxisLike, ButtonLike, InputSources, UserInput};
use bevy::time::Time;

/// An axis that returns value only when the button is pressed.
pub struct AxisChord<B, A>
where
    B: ButtonLike,
    A: AxisLike,
{
    button: B,
    axis: A,
}

impl<B, A> AxisChord<B, A>
where
    B: ButtonLike,
    A: AxisLike,
{
    pub fn new(button: B, axis: A) -> Self {
        Self { button, axis }
    }
}

impl<B, A> UserInput for AxisChord<B, A>
where
    B: ButtonLike,
    A: AxisLike,
{
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
