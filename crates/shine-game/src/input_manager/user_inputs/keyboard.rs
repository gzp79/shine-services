use crate::input_manager::{ActionLike, ButtonLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::system::{Query, Res},
    input::{keyboard::KeyCode, ButtonInput},
    time::Time,
    window::Window,
};

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
    fn process(&mut self, _time: &Time) -> Option<bool> {
        Some(self.pressed)
    }
}

pub fn integrate_keyboard_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input_query: Query<&mut InputMap<A>>,
) where
    A: ActionLike,
{
    let window = window.single().expect("Only single window is supported");

    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();

        input_source.add_resource(window);
        input_source.add_resource(&*time);
        input_source.add_resource(&*keyboard);

        input_map.integrate(input_source);
    }
}
