use crate::input_manager::{AxisLike, InputSources, UserInput};
use bevy::time::Time;
use std::borrow::Cow;

/// A trait that processes a [`AxisLike`] input value.
pub trait AxisProcessor: Send + Sync + 'static {
    fn type_name(&self) -> &'static str;
    fn process(&mut self, input_value: Option<f32>) -> Option<f32>;
}

impl<F> AxisProcessor for F
where
    F: Fn(f32) -> f32 + Send + Sync + 'static,
{
    fn type_name(&self) -> &'static str {
        "AxisFunctionProcessor"
    }

    fn process(&mut self, input_value: Option<f32>) -> Option<f32> {
        input_value.map(|v| self(v))
    }
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
    fn type_name(&self) -> &'static str {
        self.processor.type_name()
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self) && self.input.visit_recursive(depth + 1, visitor)
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

/// Helper to add some processor to an [`AxisLike`] input.
pub trait AxisProcess: AxisLike {
    fn with_process<P>(self, process: P) -> ProcessedAxis<Self, P>
    where
        Self: Sized,
        P: AxisProcessor + Send + Sync + 'static;
}

impl<T: AxisLike> AxisProcess for T {
    fn with_process<P>(self, process: P) -> ProcessedAxis<Self, P>
    where
        Self: Sized,
        P: AxisProcessor + Send + Sync + 'static,
    {
        ProcessedAxis::new(self, process)
    }
}
