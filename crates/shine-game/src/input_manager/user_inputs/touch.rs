use crate::input_manager::{DualAxisLike, InputSource, InputSources, UserInput};
use bevy::{input::touch::Touches, math::Vec2};

impl InputSource for Touches {}

/// Return touch position in screen coordinates.
/// When there is no touch, an extreme Vec2:MAX value is returned.
pub struct TouchPositionInput {
    id: Option<u64>,
    value: Vec2,
}

impl TouchPositionInput {
    pub fn new() -> Self {
        Self { id: None, value: Vec2::ZERO }
    }
}

impl UserInput for TouchPositionInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(touches) = input.get_resource::<Touches>() {
            let finger = if let Some(id) = self.id {
                touches.iter().find(|f| f.id() == id)
            } else {
                touches.iter().next()
            };

            if let Some(finger) = finger {
                self.value = finger.position();
            } else {
                self.value = Vec2::MAX;
            }
        }
    }
}

impl DualAxisLike for TouchPositionInput {
    fn value_pair(&self) -> Vec2 {
        self.value
    }
}

/*
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
}*/
