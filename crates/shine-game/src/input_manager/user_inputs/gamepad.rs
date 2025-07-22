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
use std::borrow::Cow;

/// Marker resource indicating that gamepad input source is available in [`InputSources`].
///
/// This resource is always present if gamepad input is supported, regardless of whether any gamepad is currently connected.
/// Use this to check for gamepad capability, not connection state.
#[derive(Resource)]
pub struct GamepadManager;

impl InputSource for GamepadManager {}
impl InputSource for Gamepad {}

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

/// Represents button input from a gamepad.
///
/// Returns a boolean value indicating whether the button is pressed.
/// If the gamepad is disconnected or unavailable, returns `None`.
pub struct GamepadButtonInput {
    name: Option<String>,
    gamepad: Entity,
    button: GamepadButton,
    pressed: Option<bool>,
}

impl GamepadButtonInput {
    pub fn new(gamepad: Entity, button: GamepadButton) -> Self {
        Self {
            name: None,
            gamepad,
            button,
            pressed: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for GamepadButtonInput {
    fn type_name(&self) -> &'static str {
        "GamepadButtonInput"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name
            .as_deref()
            .map_or_else(|| format!("{:?}", self.button).into(), Cow::from)
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

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

/// Represents analog stick input from a gamepad (left or right).
///
/// Returns a [`Vec2`] where each component is typically in the range [-1.0, 1.0],
/// corresponding to the stick's X and Y axes in the device's native coordinate system.
///
/// If the gamepad is disconnected or unavailable, returns `None`.
pub struct GamepadStickInput {
    name: Option<String>,
    gamepad: Entity,
    stick: GamepadStick,
    value: Option<Vec2>,
}

impl GamepadStickInput {
    pub fn new(gamepad: Entity, stick: GamepadStick) -> Self {
        Self {
            name: None,
            gamepad,
            stick,
            value: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for GamepadStickInput {
    fn type_name(&self) -> &'static str {
        "GamepadStickInput"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name
            .as_deref()
            .map_or_else(|| format!("{:?}", self.stick).into(), Cow::from)
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

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
