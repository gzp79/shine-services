use crate::input_manager::{ActionLike, ButtonLike, DualAxisLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::{
        error::BevyError,
        system::{Query, Res},
    },
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

pub fn integrate_mouse_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    mut input_query: Query<&mut InputMap<A>>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    let window = window.single()?;

    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();

        input_source.add_resource(window);
        input_source.add_resource(&*time);
        input_source.add_resource(&*mouse);
        input_source.add_resource(&*accumulated_mouse_motion);

        input_map.integrate(input_source);
    }

    Ok(())
}

/// Represents button input from a mouse button.
///
/// Returns a boolean value indicating whether the button is pressed.
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

/// Represents mouse motion input (delta movement).
///
/// Returns a [`Vec2`] where each component is in UI space (pixels), with Y axis pointing down.
/// This matches the convention of screen/UI coordinates, not world coordinates.
impl MouseMotionInput {
    pub fn new() -> Self {
        Self { value: Vec2::ZERO }
    }
}

impl UserInput for MouseMotionInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(motion) = input.get_resource::<AccumulatedMouseMotion>() {
            self.value = Vec2::new(motion.delta.x, motion.delta.y);
        }
    }
}

impl DualAxisLike for MouseMotionInput {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        Some(self.value)
    }
}

/// Represents mouse position input in screen coordinates.
///
/// Returns a [`Vec2`] where each component is in screen space (pixels), with Y axis pointing down.
/// This matches the convention of screen/UI coordinates, not world coordinates.
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
