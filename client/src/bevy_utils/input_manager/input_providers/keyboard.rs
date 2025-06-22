use crate::bevy_utils::input_manager::{
    ActionLike, AnyInputSource, ButtonLike, InputMap, InputProvider, InputSource, IntegratedInput, UserInput,
};
use bevy::{
    ecs::system::Res,
    input::{keyboard::KeyCode, ButtonInput},
    time::Time,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KeyboardStatus {
    JustPressed,
    Pressed,
    JustReleased,
    Released,
}

impl Default for KeyboardStatus {
    fn default() -> Self {
        Self::Released
    }
}

impl UserInput for KeyboardStatus {}

impl ButtonLike for KeyboardStatus {
    fn pressed(&self) -> bool {
        matches!(self, KeyboardStatus::JustPressed)
    }

    fn released(&self) -> bool {
        matches!(self, KeyboardStatus::JustReleased)
    }

    fn is_down(&self) -> bool {
        matches!(self, KeyboardStatus::JustPressed | KeyboardStatus::Pressed)
    }
}

impl InputSource for ButtonInput<KeyCode> {}

impl InputProvider for ButtonInput<KeyCode> {
    fn integrate<A: ActionLike>(provider: Res<Self>, time: Res<Time>, input_map: &mut InputMap<A>) {
        input_map.integrate(&*provider, &time);
    }
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

impl UserInput for KeyboardInput {}

impl ButtonLike for KeyboardInput {
    fn pressed(&self) -> bool {
        self.status.pressed()
    }

    fn released(&self) -> bool {
        self.status.released()
    }

    fn is_down(&self) -> bool {
        self.status.is_down()
    }
}

impl IntegratedInput for KeyboardInput {
    fn integrate(&mut self, input: &dyn AnyInputSource, _time: &Time) {
        if let Some(keyboard) = input.as_any().downcast_ref::<ButtonInput<KeyCode>>() {
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
