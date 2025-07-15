use crate::input_manager::{ButtonLike, DualAxisLike, InputSources, UserInput};
use bevy::{math::Vec2, time::Time};

/// A dual axis that returns value only when the button is pressed.
pub struct DualAxisChord<B, D>
where
    B: ButtonLike,
    D: DualAxisLike,
{
    button: B,
    dual_axis: D,
}

impl<B, D> DualAxisChord<B, D>
where
    B: ButtonLike,
    D: DualAxisLike,
{
    pub fn new(button: B, dual_axis: D) -> Self {
        Self { button, dual_axis }
    }
}

impl<B, D> UserInput for DualAxisChord<B, D>
where
    B: ButtonLike,
    D: DualAxisLike,
{
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
