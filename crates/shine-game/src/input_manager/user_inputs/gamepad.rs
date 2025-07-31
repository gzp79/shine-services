use crate::input_manager::{ActionLike, InputMap, InputSource, InputSources, TypedUserInput, UserInput};
use bevy::{
    ecs::{
        entity::Entity,
        system::{Query, Res},
    },
    input::gamepad::{Gamepad, GamepadButton},
    math::Vec2,
    time::Time,
    window::Window,
};
use std::borrow::Cow;

impl InputSource for Gamepad {}

pub fn integrate_gamepad_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut input_query: Query<&mut InputMap<A>>,
) where
    A: ActionLike,
{
    let window = window.single().expect("Only single window is supported");

    let mut input_sources = InputSources::new();
    input_sources.add_resource(window);
    input_sources.add_resource(&*time);

    input_sources.add_marker::<Gamepad>();
    for (entity, gamepad) in gamepads.iter() {
        input_sources.add_component(entity, gamepad);
    }

    for mut input_map in input_query.iter_mut() {
        input_map.integrate(&input_sources);
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
        } else if input.has_marker::<Gamepad>() {
            // we have gamepad in the input store, but our gamepad is not found
            self.pressed = None;
        }
    }
}

impl TypedUserInput<bool> for GamepadButtonInput {
    fn process(&mut self, _time: f32) -> Option<bool> {
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
        } else if input.has_marker::<Gamepad>() {
            // we have gamepad in the input store, but our gamepad is not found
            self.value = None;
        }
    }
}

impl TypedUserInput<Vec2> for GamepadStickInput {
    fn process(&mut self, _time: f32) -> Option<Vec2> {
        self.value
    }
}
