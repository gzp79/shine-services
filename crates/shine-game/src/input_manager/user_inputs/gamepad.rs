use crate::input_manager::{ActionLike, ButtonLike, DualAxisLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::{
        entity::Entity,
        resource::Resource,
        system::{Query, Res},
    },
    input::gamepad::{Gamepad, GamepadButton},
    math::Vec2,
    time::Time,
    window::Window,
};

/// A utility resource to distinct if input source contains gamepad information or not.
/// This resurce is always present if gamepad input is available in the INputSource independent of the gamepad connection state.
#[derive(Resource)]
pub struct GamepadManager;

impl InputSource for GamepadManager {}
impl InputSource for Gamepad {}

/// A gamepad button input.
pub struct GamepadButtonInput {
    gamepad: Entity,
    button: GamepadButton,
    pressed: Option<bool>,
}

impl GamepadButtonInput {
    pub fn new(gamepad: Entity, button: GamepadButton) -> Self {
        Self { gamepad, button, pressed: None }
    }
}

impl UserInput for GamepadButtonInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(gamepad) = input.get_component::<Gamepad>(self.gamepad) {
            self.pressed = Some(gamepad.pressed(self.button));
        } else if let Some(_gamepad_settings) = input.get_resource::<GamepadManager>() {
            // we have gamepad in the input store, but our gamepad is not found
            self.pressed = None;
        }
    }
}

impl ButtonLike for GamepadButtonInput {
    fn process(&mut self, _time: &Time) -> Option<bool> {
        self.pressed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadStick {
    Left,
    Right,
}

pub struct GamepadStickInput {
    gamepad: Entity,
    stick: GamepadStick,
    value: Option<Vec2>,
}

impl GamepadStickInput {
    pub fn new(gamepad: Entity, stick: GamepadStick) -> Self {
        Self { gamepad, stick, value: None }
    }
}

impl UserInput for GamepadStickInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(gamepad) = input.get_component::<Gamepad>(self.gamepad) {
            self.value = Some(match self.stick {
                GamepadStick::Left => gamepad.left_stick(),
                GamepadStick::Right => gamepad.right_stick(),
            });
        } else if let Some(_gamepad_manager) = input.get_resource::<GamepadManager>() {
            // we have gamepad in the input store, but our gamepad is not found
            self.value = None;
        }
    }
}

impl DualAxisLike for GamepadStickInput {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}

pub fn integrate_gamepad_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    gamepads: Query<(Entity, &Gamepad)>,
    gamepad_manager: Res<GamepadManager>,
    mut input_query: Query<&mut InputMap<A>>,
) where
    A: ActionLike,
{
    let window = window.single().expect("Only single window is supported");

    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();

        input_source.add_resource(window);
        input_source.add_resource(&*time);
        input_source.add_resource(&*gamepad_manager);
        for (entity, gamepad) in gamepads.iter() {
            input_source.add_component(entity, gamepad);
        }

        input_map.integrate(input_source);
    }
}
