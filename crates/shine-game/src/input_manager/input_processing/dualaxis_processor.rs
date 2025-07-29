use crate::input_manager::{DualAxisLike, InputSources, UserInput};
use bevy::{math::Vec2, time::Time};
use std::borrow::Cow;

/// A trait that processes a [`DualAxisLike`] input value.
pub trait DualAxisProcessor: Send + Sync + 'static {
    fn type_name(&self) -> &'static str;
    fn process(&mut self, input_value: Option<Vec2>) -> Option<Vec2>;
}

impl<F> DualAxisProcessor for F
where
    F: Fn(Vec2) -> Vec2 + Send + Sync + 'static,
{
    fn type_name(&self) -> &'static str {
        "DualAxisFunctionProcessor"
    }

    fn process(&mut self, input_value: Option<Vec2>) -> Option<Vec2> {
        input_value.map(|v| self(v))
    }
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

impl<I: DualAxisLike, P: DualAxisProcessor> DualAxisLike for ProcessedDualAxis<I, P> {
    fn process(&mut self, time: &Time) -> Option<Vec2> {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}

/// Helper to add some processor to an [`DualAxisLike`] input.
pub trait DualAxisProcess: DualAxisLike {
    fn with_process<P>(self, process: P) -> ProcessedDualAxis<Self, P>
    where
        Self: Sized,
        P: DualAxisProcessor + Send + Sync + 'static;
}

impl<T: DualAxisLike> DualAxisProcess for T {
    fn with_process<P>(self, process: P) -> ProcessedDualAxis<Self, P>
    where
        Self: Sized,
        P: DualAxisProcessor + Send + Sync + 'static,
    {
        ProcessedDualAxis::new(self, process)
    }
}
