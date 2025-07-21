use crate::input_manager::{AxisLike, ButtonLike, DualAxisLike, InputSources, UserInput};
use bevy::{math::Vec2, time::Time};

/// A trait that processes a [`ButtonLike`] input value.
pub trait ButtonProcessor: Send + Sync + 'static {
    fn process(&mut self, input_value: Option<bool>) -> Option<bool>;
}

pub struct ProcessedButton<I: ButtonLike, P: ButtonProcessor> {
    name: Option<String>,
    input: I,
    processor: P,
}

impl<I: ButtonLike, P: ButtonProcessor> ProcessedButton<I, P> {
    pub fn new(input: I, processor: P) -> Self {
        Self { name: None, input, processor }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I: ButtonLike, P: ButtonProcessor> UserInput for ProcessedButton<I, P> {
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        if self.name.as_deref() == Some(name) {
            Some(self)
        } else {
            self.input.find(name)
        }
    }

    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<I: ButtonLike, P: ButtonProcessor> ButtonLike for ProcessedButton<I, P> {
    fn process(&mut self, time: &Time) -> Option<bool> {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}

/// A trait that processes a [`AxisLike`] input value.
pub trait AxisProcessor: Send + Sync + 'static {
    fn process(&mut self, input_value: Option<f32>) -> Option<f32>;
}

pub struct ProcessedAxis<I: AxisLike, P: AxisProcessor> {
    name: Option<String>,
    input: I,
    processor: P,
}

impl<I: AxisLike, P: AxisProcessor> ProcessedAxis<I, P> {
    pub fn new(input: I, processor: P) -> Self {
        Self { name: None, input, processor }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I: AxisLike, P: AxisProcessor> UserInput for ProcessedAxis<I, P> {
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        if self.name.as_deref() == Some(name) {
            Some(self)
        } else {
            self.input.find(name)
        }
    }

    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<I: AxisLike, P: AxisProcessor> AxisLike for ProcessedAxis<I, P> {
    fn process(&mut self, time: &Time) -> Option<f32> {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}

/// A trait that processes a [`DualAxisLike`] input value.
pub trait DualAxisProcessor: Send + Sync + 'static {
    fn process(&mut self, input_value: Option<Vec2>) -> Option<Vec2>;
}

pub struct ProcessedDualAxis<I: DualAxisLike, P: DualAxisProcessor> {
    name: Option<String>,
    input: I,
    processor: P,
}

impl<I: DualAxisLike, P: DualAxisProcessor> ProcessedDualAxis<I, P> {
    pub fn new(input: I, processor: P) -> Self {
        Self { name: None, input, processor }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<I: DualAxisLike, P: DualAxisProcessor> UserInput for ProcessedDualAxis<I, P> {
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        if self.name.as_deref() == Some(name) {
            Some(self)
        } else {
            self.input.find(name)
        }
    }

    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<I: DualAxisLike, P: DualAxisProcessor> DualAxisLike for ProcessedDualAxis<I, P> {
    fn process(&mut self, time: &Time) -> Option<Vec2> {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}
