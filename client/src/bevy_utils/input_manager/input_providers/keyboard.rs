use crate::bevy_utils::input_manager::{ButtonLike, InputSources, UserInput};
use bevy::input::{keyboard::KeyCode, ButtonInput};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KeyboardStatus {
    JustPressed,
    Pressed,
    JustReleased,
    Released,
}

pub struct KeyboardInput {
    key: KeyCode,
    status: KeyboardStatus,
}

impl KeyboardInput {
    pub fn new(key: KeyCode) -> Self {
        Self {
            key,
            status: KeyboardStatus::Released,
        }
    }
}

impl UserInput for KeyboardInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(keyboard) = input.get_source::<ButtonInput<KeyCode>>() {
            if keyboard.just_pressed(self.key) {
                self.status = KeyboardStatus::JustPressed;
            } else if keyboard.pressed(self.key) {
                self.status = KeyboardStatus::Pressed;
            } else if keyboard.just_released(self.key) {
                self.status = KeyboardStatus::JustReleased;
            } else {
                self.status = KeyboardStatus::Released;
            }
        }
    }
}

impl ButtonLike for KeyboardInput {
    fn pressed(&self) -> bool {
        matches!(self.status, KeyboardStatus::JustPressed)
    }

    fn released(&self) -> bool {
        matches!(self.status, KeyboardStatus::JustReleased)
    }

    fn is_down(&self) -> bool {
        matches!(self.status, KeyboardStatus::JustPressed | KeyboardStatus::Pressed)
    }
}
