use crate::input_manager::{InputDrivers, InputProcessor, TypedInputProcessor};
use std::borrow::Cow;

/// A constant button input that is always pressed.
pub struct PressedButton;

impl InputProcessor for PressedButton {
    fn name(&self) -> Cow<'_, str> {
        "".into()
    }

    fn visit_recursive<'a>(
        &'a self,
        depth: usize,
        visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool,
    ) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, _input: &InputDrivers) {}
}

impl TypedInputProcessor<bool> for PressedButton {
    fn process(&mut self, _time: f32) -> Option<bool> {
        Some(true)
    }
}
