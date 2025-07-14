use crate::input_manager::{DualAxisLike, InputSource, InputSources, UserInput};
use bevy::{input::touch::Touches, math::Vec2, time::Time};

impl InputSource for Touches {}

/// Return touch position for the first finger in screen coordinates.
/// When there is no touch, an extreme Vec2::MAX value is returned.
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
    fn process(&mut self, _time: &Time) -> Vec2 {
        self.value
    }
}
