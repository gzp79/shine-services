use crate::input_manager::{ActionLike, InputMap, InputSource, InputSources, TypedUserInput, UserInput};
use bevy::{
    ecs::{
        error::BevyError,
        system::{Query, Res},
    },
    input::{keyboard::KeyCode, ButtonInput},
    time::Time,
    window::Window,
};
use std::borrow::Cow;

impl InputSource for ButtonInput<KeyCode> {}

pub fn integrate_keyboard_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input_query: Query<&mut InputMap<A>>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    let window = window.single()?;

    let mut input_sources = InputSources::new();
    input_sources.add_resource(window);
    input_sources.add_resource(&*time);
    input_sources.add_resource(&*keyboard);

    for mut input_map in input_query.iter_mut() {
        input_map.integrate(&input_sources);
    }

    Ok(())
}

/// Represents button input from a keyboard key.
///
/// Returns a boolean value indicating whether the key is pressed.
/// If the keyboard input resource is unavailable, returns `None`.
pub struct KeyboardInput {
    name: Option<String>,
    key: KeyCode,
    pressed: bool,
}

impl KeyboardInput {
    pub fn new(key: KeyCode) -> Self {
        Self {
            name: None,
            key,
            pressed: false,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }
}

impl UserInput for KeyboardInput {
    fn type_name(&self) -> &'static str {
        "KeyboardInput"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name
            .as_deref()
            .map_or_else(|| format!("{:?}", self.key).into(), Cow::from)
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if let Some(keyboard) = input.get_resource::<ButtonInput<KeyCode>>() {
            self.pressed = keyboard.pressed(self.key);
        }
    }
}

impl TypedUserInput<bool> for KeyboardInput {
    fn process(&mut self, _time: f32) -> Option<bool> {
        Some(self.pressed)
    }
}
