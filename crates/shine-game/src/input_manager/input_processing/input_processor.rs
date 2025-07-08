use crate::input_manager::{DualAxisLike, InputSources, UserInput};
use bevy::math::Vec2;

/// A trait that processes a [`DualAxisLike`] input value.
pub trait DualAxisProcessor: Send + Sync + 'static {
    fn process(&self, input_value: Vec2) -> Vec2;
}

pub struct ProcessedDualAxis<I: DualAxisLike, P: DualAxisProcessor> {
    pub input: I,
    pub processor: P,
}

impl<I: DualAxisLike, P: DualAxisProcessor> ProcessedDualAxis<I, P> {
    pub fn new(input: I, processor: P) -> Self {
        Self { input, processor }
    }
}

impl<I: DualAxisLike, P: DualAxisProcessor> UserInput for ProcessedDualAxis<I, P> {
    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<I: DualAxisLike, P: DualAxisProcessor> DualAxisLike for ProcessedDualAxis<I, P> {
    fn value_pair(&self) -> Vec2 {
        self.processor.process(self.input.value_pair())
    }
}
