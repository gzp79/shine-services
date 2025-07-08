use crate::input_manager::{ButtonLike, DualAxisLike, InputSource, InputSources, UserInput};
use bevy::{
    input::{
        mouse::{AccumulatedMouseMotion, MouseButton},
        ButtonInput,
    },
    math::Vec2,
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
    fn is_down(&self) -> bool {
        self.pressed
    }
}

pub struct MouseMotionInput {
    value: Vec2,
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
    fn value_pair(&self) -> Vec2 {
        self.value
    }
}

/// Return mouse position in screen coordinates
pub struct MousePositionInput {
    value: Vec2,
}

impl MousePositionInput {
    pub fn new() -> Self {
        Self { value: Vec2::ZERO }
    }
}

impl UserInput for MousePositionInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(window) = input.get_resource::<Window>() {
            // if cursor if off-screen, preserve the last position
            if let Some(pos) = window.cursor_position() {
                self.value = pos;
            }
        }
    }
}

impl DualAxisLike for MousePositionInput {
    fn value_pair(&self) -> Vec2 {
        self.value
    }
}

/// Return normalized mouse position.
/// The value for the smaller dimension is in the range [-1.0, 1.0],
/// the larger dimension is kept proportional to keep the aspect ratio.
pub struct MouseNormalizedPositionInput {
    value: Vec2,
}

impl MouseNormalizedPositionInput {
    pub fn new() -> Self {
        Self { value: Vec2::ZERO }
    }
}

impl UserInput for MouseNormalizedPositionInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(window) = input.get_resource::<Window>() {
            // if cursor if off-screen, preserve the last position
            if let Some(pos) = window.cursor_position() {
                let (w, h) = (window.width(), window.height());
                let s = (w.min(h) / 2.0).max(1.0);
                self.value = Vec2::new((pos.x - w / 2.0) / s, (pos.y - h / 2.0) / s);
                // Invert the y-axis because in the input system, upward movement is positive
                self.value.y = -self.value.y;
            }
        }
    }
}

impl DualAxisLike for MouseNormalizedPositionInput {
    fn value_pair(&self) -> Vec2 {
        self.value
    }
}
