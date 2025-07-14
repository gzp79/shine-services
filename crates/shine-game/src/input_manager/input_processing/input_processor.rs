use crate::input_manager::{AxisLike, ButtonLike, DualAxisLike, InputSources, UserInput};
use bevy::{math::Vec2, time::Time};

/// A trait that processes a [`ButtonLike`] input value.
pub trait ButtonProcessor: Send + Sync + 'static {
    fn process(&mut self, input_value: bool) -> bool;
}

pub struct ProcessedButton<I: ButtonLike, P: ButtonProcessor> {
    pub input: I,
    pub processor: P,
}

impl<I: ButtonLike, P: ButtonProcessor> ProcessedButton<I, P> {
    pub fn new(input: I, processor: P) -> Self {
        Self { input, processor }
    }
}

impl<I: ButtonLike, P: ButtonProcessor> UserInput for ProcessedButton<I, P> {
    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<I: ButtonLike, P: ButtonProcessor> ButtonLike for ProcessedButton<I, P> {
    fn process(&mut self, time: &Time) -> bool {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}

/// A trait that processes a [`AxisLike`] input value.
pub trait AxisProcessor: Send + Sync + 'static {
    fn process(&mut self, input_value: f32) -> f32;
}

pub struct ProcessedAxis<I: AxisLike, P: AxisProcessor> {
    pub input: I,
    pub processor: P,
}

impl<I: AxisLike, P: AxisProcessor> ProcessedAxis<I, P> {
    pub fn new(input: I, processor: P) -> Self {
        Self { input, processor }
    }
}

impl<I: AxisLike, P: AxisProcessor> UserInput for ProcessedAxis<I, P> {
    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<I: AxisLike, P: AxisProcessor> AxisLike for ProcessedAxis<I, P> {
    fn process(&mut self, time: &Time) -> f32 {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}

/// A trait that processes a [`DualAxisLike`] input value.
pub trait DualAxisProcessor: Send + Sync + 'static {
    fn process(&mut self, input_value: Vec2) -> Vec2;
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
    fn process(&mut self, time: &Time) -> Vec2 {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}
