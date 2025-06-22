use crate::input_manager::{ButtonLike, ButtonStatus, InputSource, InputSources, UserInput};
use bevy::input::{keyboard::KeyCode, ButtonInput};

impl InputSource for ButtonInput<KeyCode> {}

/// A keyboard button input.
pub struct KeyboardInput {
    key: KeyCode,
    status: ButtonStatus,
}

impl KeyboardInput {
    pub fn new(key: KeyCode) -> Self {
        Self {
            key,
            status: ButtonStatus::Released,
        }
    }
}

impl UserInput for KeyboardInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(keyboard) = input.get_resource::<ButtonInput<KeyCode>>() {
            if keyboard.just_pressed(self.key) {
                self.status = ButtonStatus::JustPressed;
            } else if keyboard.pressed(self.key) {
                self.status = ButtonStatus::Pressed;
            } else if keyboard.just_released(self.key) {
                self.status = ButtonStatus::JustReleased;
            } else {
                self.status = ButtonStatus::Released;
            }
        }
    }
}

impl ButtonLike for KeyboardInput {
    fn is_down(&self) -> bool {
        matches!(self.status, ButtonStatus::JustPressed | ButtonStatus::Pressed)
    }
}
