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
use std::borrow::Cow;

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

    let mut input_sources = InputSources::new();
    input_sources.add_resource(window);
    input_sources.add_resource(&*time);
    input_sources.add_resource(&*mouse);
    input_sources.add_resource(&*accumulated_mouse_motion);

    for mut input_map in input_query.iter_mut() {
        input_map.integrate(&input_sources);
    }

    Ok(())
}

/// Represents button input from a mouse button.
///
/// Returns a boolean value indicating whether the button is pressed.
pub struct MouseButtonInput {
    name: Option<String>,
    key: MouseButton,
    pressed: bool,
}

impl MouseButtonInput {
    pub fn new(key: MouseButton) -> Self {
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

impl UserInput for MouseButtonInput {
    fn type_name(&self) -> &'static str {
        "MouseButtonInput"
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
        if let Some(mouse) = input.get_resource::<ButtonInput<MouseButton>>() {
            self.pressed = mouse.pressed(self.key);
        }
    }
}

impl ButtonLike for MouseButtonInput {
    fn process(&mut self, _time: &Time) -> Option<bool> {
        Some(self.pressed)
    }
}

/// Represents mouse motion input (delta movement).
///
/// Returns a [`Vec2`] where each component is in UI space (pixels), with Y axis pointing down.
/// This matches the convention of screen/UI coordinates, not world coordinates.
pub struct MouseMotion {
    name: Option<String>,
    value: Vec2,
}

impl Default for MouseMotion {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents mouse motion input (delta movement).
///
/// Returns a [`Vec2`] where each component is in UI space (pixels), with Y axis pointing down.
/// This matches the convention of screen/UI coordinates, not world coordinates.
impl MouseMotion {
    pub fn new() -> Self {
        Self { name: None, value: Vec2::ZERO }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for MouseMotion {
    fn type_name(&self) -> &'static str {
        "MouseMotion"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if let Some(motion) = input.get_resource::<AccumulatedMouseMotion>() {
            self.value = Vec2::new(motion.delta.x, motion.delta.y);
        }
    }
}

impl DualAxisLike for MouseMotion {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        Some(self.value)
    }
}

/// Represents mouse position input in screen coordinates.
///
/// Returns a [`Vec2`] where each component is in screen space (pixels), with Y axis pointing down.
/// This matches the convention of screen/UI coordinates, not world coordinates.
pub struct MousePosition {
    name: Option<String>,
    value: Option<Vec2>,
}

impl Default for MousePosition {
    fn default() -> Self {
        Self::new()
    }
}

impl MousePosition {
    pub fn new() -> Self {
        Self { name: None, value: None }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl UserInput for MousePosition {
    fn type_name(&self) -> &'static str {
        "MousePosition"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if let Some(window) = input.get_resource::<Window>() {
            self.value = window.cursor_position()
        }
    }
}

impl DualAxisLike for MousePosition {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}
