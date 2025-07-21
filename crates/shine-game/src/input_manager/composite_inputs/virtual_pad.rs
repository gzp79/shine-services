use crate::input_manager::{AxisLike, AxisRadialProcessor, ButtonLike, InputSources, KeyboardInput, UserInput};
use bevy::{input::keyboard::KeyCode, time::Time};

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
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn find(&self, name: &str) -> Option<&dyn UserInput> {
        if self.name.as_deref() == Some(name) {
            Some(self)
        } else {
            self.up.find(name).or_else(|| self.down.find(name))
        }
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
