use crate::input_manager::{InputSources, TypedUserInput, UserInput};
use std::{borrow::Cow, marker::PhantomData};

pub trait MapInput<T, U>: Send + Sync + 'static
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
{
    fn name(&self) -> Cow<'_, str>;
    fn map_value(&mut self, input_value: Option<T>) -> Option<U>;
}

pub struct MappedInput<T, U, I, M>
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
    I: TypedUserInput<T>,
    M: MapInput<T, U>,
{
    name: Option<String>,
    input: I,
    map: M,
    _ph: PhantomData<(T, U)>,
}

impl<T, U, I, M> MappedInput<T, U, I, M>
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
    I: TypedUserInput<T>,
    M: MapInput<T, U>,
{
    pub fn new(input: I, map: M) -> Self {
        Self {
            name: None,
            input,
            map,
            _ph: PhantomData,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<T, U, I, M> UserInput for MappedInput<T, U, I, M>
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
    I: TypedUserInput<T>,
    M: MapInput<T, U>,
{
    fn type_name(&self) -> &'static str {
        "MappedInput"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().map(Cow::from).unwrap_or(self.map.name())
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self) && self.input.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<T, U, I, M> TypedUserInput<U> for MappedInput<T, U, I, M>
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
    I: TypedUserInput<T>,
    M: MapInput<T, U>,
{
    fn process(&mut self, time_s: f32) -> Option<U> {
        let value = self.input.process(time_s);
        self.map.map_value(value)
    }
}

impl<T, U, F> MapInput<T, U> for F
where
    T: Send + Sync + 'static,
    U: Send + Sync + 'static,
    F: Fn(T) -> U + Send + Sync + 'static,
{
    fn name(&self) -> Cow<'_, str> {
        Cow::from("FunctionMap")
    }

    fn map_value(&mut self, input_value: Option<T>) -> Option<U> {
        input_value.map(self)
    }
}
