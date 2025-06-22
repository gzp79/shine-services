use crate::input_manager::{ButtonLike, InputSource, InputSources, UserInput};
use bevy::input::{keyboard::KeyCode, ButtonInput};

impl InputSource for ButtonInput<KeyCode> {}

/// A keyboard button input.
pub struct KeyboardInput {
    key: KeyCode,
    pressed: bool,
}

impl KeyboardInput {
    pub fn new(key: KeyCode) -> Self {
        Self { key, pressed: false }
    }
}

impl UserInput for KeyboardInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(keyboard) = input.get_resource::<ButtonInput<KeyCode>>() {
            self.pressed = keyboard.pressed(self.key);
        }
    }
}

impl ButtonLike for KeyboardInput {
    fn is_down(&self) -> bool {
        self.pressed
    }
}
