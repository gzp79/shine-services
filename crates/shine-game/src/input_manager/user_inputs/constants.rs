use crate::input_manager::{ButtonLike, InputSources, UserInput};
use bevy::time::Time;
use std::borrow::Cow;

/// A constant button input that is always pressed.
pub struct PressedButton;

impl UserInput for PressedButton {
    fn type_name(&self) -> &'static str {
        "PressedButton"
    }

    fn name(&self) -> Cow<'_, str> {
        "".into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, _input: &InputSources) {}
}

impl ButtonLike for PressedButton {
    fn process(&mut self, _time: &Time) -> Option<bool> {
        Some(true)
    }
}
