use crate::input_manager::{ActionLike, ButtonLike, DualAxisLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::system::{Query, Res},
    input::{
        mouse::{AccumulatedMouseMotion, MouseButton},
        ButtonInput,
    },
    math::Vec2,
    time::Time,
    window::Window,
};

impl InputSource for ButtonInput<MouseButton> {}
impl InputSource for AccumulatedMouseMotion {}
impl InputSource for Window {}

/// A keyboard button input.
pub struct MouseButtonInput {
    key: MouseButton,
    pressed: bool,
}

impl MouseButtonInput {
    pub fn new(key: MouseButton) -> Self {
        Self { key, pressed: false }
    }
}

impl UserInput for MouseButtonInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(keyboard) = input.get_resource::<ButtonInput<MouseButton>>() {
            self.pressed = keyboard.pressed(self.key);
        }
    }
}

impl ButtonLike for MouseButtonInput {
    fn process(&mut self, _time: &Time) -> Option<bool> {
        Some(self.pressed)
    }
}

pub struct MouseMotionInput {
    value: Vec2,
}

impl Default for MouseMotionInput {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseMotionInput {
    pub fn new() -> Self {
        Self { value: Vec2::ZERO }
    }
}

impl UserInput for MouseMotionInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(motion) = input.get_resource::<AccumulatedMouseMotion>() {
            self.value = Vec2::new(motion.delta.x, motion.delta.y);
            // Invert the y-axis because in the input system, upward movement is positive
            self.value.y = -self.value.y;
        }
    }
}

impl DualAxisLike for MouseMotionInput {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        Some(self.value)
    }
}

/// Return mouse position in screen coordinates
pub struct MousePositionInput {
    value: Option<Vec2>,
}

impl Default for MousePositionInput {
    fn default() -> Self {
        Self::new()
    }
}

impl MousePositionInput {
    pub fn new() -> Self {
        Self { value: None }
    }
}

impl UserInput for MousePositionInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(window) = input.get_resource::<Window>() {
            self.value = window.cursor_position()
        }
    }
}

impl DualAxisLike for MousePositionInput {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}

pub fn integrate_mouse_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    mut input_query: Query<&mut InputMap<A>>,
) where
    A: ActionLike,
{
    let window = window.single().expect("Only single window is supported");

    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();

        input_source.add_resource(window);
        input_source.add_resource(&*time);
        input_source.add_resource(&*mouse);
        input_source.add_resource(&*accumulated_mouse_motion);

        input_map.integrate(input_source);
    }
}
