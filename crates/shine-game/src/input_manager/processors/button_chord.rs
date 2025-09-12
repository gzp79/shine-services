use crate::input_manager::{InputDrivers, InputProcessor, KeyboardInput, TypedInputProcessor};
use bevy::input::keyboard::KeyCode;
use std::borrow::Cow;

/// A button chord that processes multiple buttons as a single input.
pub struct ButtonChord<B1, B2>
where
    B1: TypedInputProcessor<bool>,
    B2: TypedInputProcessor<bool>,
{
    name: Option<String>,
    chord: (B1, B2),
}

impl<B1, B2> ButtonChord<B1, B2>
where
    B1: TypedInputProcessor<bool>,
    B2: TypedInputProcessor<bool>,
{
    pub fn new(b1: B1, b2: B2) -> Self {
        Self { name: None, chord: (b1, b2) }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl ButtonChord<KeyboardInput, KeyboardInput> {
    pub fn from_keys(b1: KeyCode, b2: KeyCode) -> Self {
        Self::new(KeyboardInput::new(b1), KeyboardInput::new(b2))
    }
}

impl<B1, B2> InputProcessor for ButtonChord<B1, B2>
where
    B1: TypedInputProcessor<bool>,
    B2: TypedInputProcessor<bool>,
{
    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(
        &'a self,
        depth: usize,
        visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool,
    ) -> bool {
        visitor(depth, self)
            && self.chord.0.visit_recursive(depth + 1, visitor)
            && self.chord.1.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputDrivers) {
        self.chord.0.integrate(input);
        self.chord.1.integrate(input);
    }
}

impl<B1, B2> TypedInputProcessor<bool> for ButtonChord<B1, B2>
where
    B1: TypedInputProcessor<bool>,
    B2: TypedInputProcessor<bool>,
{
    fn process(&mut self, time_s: f32) -> Option<bool> {
        let v0 = self.chord.0.process(time_s).unwrap_or(false);
        let v1 = self.chord.1.process(time_s).unwrap_or(false);
        Some(v0 & v1)
    }
}
