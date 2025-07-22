use crate::input_manager::{AxisLike, AxisRadialProcessor, ButtonLike, InputSources, KeyboardInput, UserInput};
use bevy::{input::keyboard::KeyCode, time::Time};
use std::borrow::Cow;

/// A virtual pad that converts 2 buttons into an axis.
pub struct VirtualPad<U, D>
where
    U: ButtonLike,
    D: ButtonLike,
{
    name: Option<String>,
    up: U,
    down: D,
}

impl<U, D> VirtualPad<U, D>
where
    U: ButtonLike,
    D: ButtonLike,
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
    U: ButtonLike,
    D: ButtonLike,
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

impl<U, D> AxisLike for VirtualPad<U, D>
where
    U: ButtonLike,
    D: ButtonLike,
{
    fn process(&mut self, time: &Time) -> Option<f32> {
        let mut value = 0.0;
        if self.up.process(time).unwrap_or(false) {
            value += 1.0;
        }
        if self.down.process(time).unwrap_or(false) {
            value -= 1.0;
        }
        Some(value)
    }
}

impl VirtualPad<KeyboardInput, KeyboardInput> {
    pub fn qe() -> impl AxisLike {
        Self::new(KeyboardInput::new(KeyCode::KeyQ), KeyboardInput::new(KeyCode::KeyE)).with_bounds(1.0)
    }
}
