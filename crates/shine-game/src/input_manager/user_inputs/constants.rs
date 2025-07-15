use crate::input_manager::{ButtonLike, InputSources, UserInput};
use bevy::time::Time;

/// A constant button input that is always pressed.
pub struct PressedButton;

impl UserInput for PressedButton {
    fn integrate(&mut self, _input: &InputSources) {}
}

impl ButtonLike for PressedButton {
    fn process(&mut self, _time: &Time) -> Option<bool> {
        Some(true)
    }
}
