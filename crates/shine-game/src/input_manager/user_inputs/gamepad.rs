use crate::input_manager::{ButtonLike, ButtonStatus, DualAxisLike, InputSource, InputSources, UserInput};
use bevy::{
    ecs::entity::Entity,
    input::gamepad::{Gamepad, GamepadButton},
    math::Vec2,
};

impl InputSource for Gamepad {}

/// A gamepad button input.
pub struct GamepadButtonInput {
    gamepad: Entity,
    button: GamepadButton,
    status: ButtonStatus,
}

impl GamepadButtonInput {
    pub fn new(gamepad: Entity, button: GamepadButton) -> Self {
        Self {
            gamepad,
            button,
            status: ButtonStatus::Released,
        }
    }
}

impl UserInput for GamepadButtonInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(gamepad) = input.get_component::<Gamepad>(self.gamepad) {
            if gamepad.just_pressed(self.button) {
                self.status = ButtonStatus::JustPressed;
            } else if gamepad.pressed(self.button) {
                self.status = ButtonStatus::Pressed;
            } else if gamepad.just_released(self.button) {
                self.status = ButtonStatus::JustReleased;
            }
        }
    }
}

impl ButtonLike for GamepadButtonInput {
    fn is_down(&self) -> bool {
        matches!(self.status, ButtonStatus::JustPressed | ButtonStatus::Pressed)
    }
}

pub struct GamepadAxisInput {
    gamepad: Entity,
    left: bool,
    value: Vec2,
}

impl GamepadAxisInput {
    pub fn new(gamepad: Entity, left: bool) -> Self {
        Self {
            gamepad,
            left,
            value: Vec2::ZERO,
        }
    }
}

impl UserInput for GamepadAxisInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(gamepad) = input.get_component::<Gamepad>(self.gamepad) {
            self.value = if self.left {
                gamepad.left_stick()
            } else {
                gamepad.right_stick()
            };
        }
    }
}

impl DualAxisLike for GamepadAxisInput {
    fn value_pair(&self) -> Vec2 {
        self.value
    }
}
