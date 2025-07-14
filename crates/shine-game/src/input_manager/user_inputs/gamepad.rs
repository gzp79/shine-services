use crate::input_manager::{ButtonLike, DualAxisLike, InputSource, InputSources, UserInput};
use bevy::{
    ecs::{entity::Entity, resource::Resource},
    input::gamepad::{Gamepad, GamepadButton},
    math::Vec2,
    time::Time,
};

/// A utility resource to distinct if indicate if input source contains gamepad information or not
/// independent of any gamepad being connected or not.
#[derive(Resource)]
pub struct GamepadManager;

impl InputSource for GamepadManager {}
impl InputSource for Gamepad {}

/// A gamepad button input.
pub struct GamepadButtonInput {
    gamepad: Entity,
    button: GamepadButton,
    pressed: bool,
}

impl GamepadButtonInput {
    pub fn new(gamepad: Entity, button: GamepadButton) -> Self {
        Self {
            gamepad,
            button,
            pressed: false,
        }
    }
}

impl UserInput for GamepadButtonInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(gamepad) = input.get_component::<Gamepad>(self.gamepad) {
            self.pressed = gamepad.pressed(self.button);
        } else if let Some(_gamepad_settings) = input.get_resource::<GamepadManager>() {
            // we have gamepad in the input store, but our gamepad is not found
            self.pressed = false;
        }
    }
}

impl ButtonLike for GamepadButtonInput {
    fn process(&mut self, _time: &Time) -> bool {
        self.pressed
    }
}

pub struct GamepadStickInput {
    gamepad: Entity,
    left: bool,
    value: Vec2,
}

impl GamepadStickInput {
    pub fn new(gamepad: Entity, left: bool) -> Self {
        Self {
            gamepad,
            left,
            value: Vec2::ZERO,
        }
    }
}

impl UserInput for GamepadStickInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(gamepad) = input.get_component::<Gamepad>(self.gamepad) {
            self.value = if self.left {
                gamepad.left_stick()
            } else {
                gamepad.right_stick()
            };
        } else if let Some(_gamepad_manager) = input.get_resource::<GamepadManager>() {
            // we have gamepad in the input store, but our gamepad is not found
            self.value = Vec2::ZERO;
        }
    }
}

impl DualAxisLike for GamepadStickInput {
    fn process(&mut self, _time: &Time) -> Vec2 {
        self.value
    }
}
