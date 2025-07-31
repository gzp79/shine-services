use crate::input_manager::{InputSources, KeyboardInput, RadialInputProcess, TypedUserInput, UserInput};
use bevy::input::keyboard::KeyCode;
use std::borrow::Cow;

/// A virtual pad that converts 2 buttons into an axis.
pub struct VirtualPad<U, D>
where
    U: TypedUserInput<bool>,
    D: TypedUserInput<bool>,
{
    name: Option<String>,
    up: U,
    down: D,
}

impl<U, D> VirtualPad<U, D>
where
    U: TypedUserInput<bool>,
    D: TypedUserInput<bool>,
{
    pub fn new(up: U, down: D) -> Self {
        Self { name: None, up, down }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl<U, D> UserInput for VirtualPad<U, D>
where
    U: TypedUserInput<bool>,
    D: TypedUserInput<bool>,
{
    fn type_name(&self) -> &'static str {
        "VirtualPad"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
            && self.up.visit_recursive(depth + 1, visitor)
            && self.down.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.up.integrate(input);
        self.down.integrate(input);
    }
}

impl<U, D> TypedUserInput<f32> for VirtualPad<U, D>
where
    U: TypedUserInput<bool>,
    D: TypedUserInput<bool>,
{
    fn process(&mut self, time_s: f32) -> Option<f32> {
        let mut value = 0.0;
        if self.up.process(time_s).unwrap_or(false) {
            value += 1.0;
        }
        if self.down.process(time_s).unwrap_or(false) {
            value -= 1.0;
        }
        Some(value)
    }
}

impl VirtualPad<KeyboardInput, KeyboardInput> {
    pub fn from_keys(up: KeyCode, down: KeyCode) -> impl TypedUserInput<f32> {
        Self::new(KeyboardInput::new(up), KeyboardInput::new(down)).with_bounds(1.0)
    }
}
