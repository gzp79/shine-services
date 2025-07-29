use crate::input_manager::{ButtonLike, InputSources, UserInput};
use bevy::time::Time;
use std::borrow::Cow;

/// A trait that processes a [`ButtonLike`] input value.
pub trait ButtonProcessor: Send + Sync + 'static {
    fn type_name(&self) -> &'static str;
    fn process(&mut self, input_value: Option<bool>) -> Option<bool>;
}

impl<F> ButtonProcessor for F
where
    F: Fn(bool) -> bool + Send + Sync + 'static,
{
    fn type_name(&self) -> &'static str {
        "ButtonFunctionProcessor"
    }

    fn process(&mut self, input_value: Option<bool>) -> Option<bool> {
        input_value.map(|v| self(v))
    }
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

impl<I: ButtonLike, P: ButtonProcessor> ButtonLike for ProcessedButton<I, P> {
    fn process(&mut self, time: &Time) -> Option<bool> {
        let value = self.input.process(time);
        self.processor.process(value)
    }
}

/// Helper to add some processor to an [`ButtonLike`] input.
pub trait ButtonProcess: ButtonLike {
    fn with_process<P>(self, process: P) -> ProcessedButton<Self, P>
    where
        Self: Sized,
        P: ButtonProcessor + Send + Sync + 'static;
}

impl<T: ButtonLike> ButtonProcess for T {
    fn with_process<P>(self, process: P) -> ProcessedButton<Self, P>
    where
        Self: Sized,
        P: ButtonProcessor + Send + Sync + 'static,
    {
        ProcessedButton::new(self, process)
    }
}
