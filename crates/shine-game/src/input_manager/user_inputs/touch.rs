use crate::input_manager::{DualAxisLike, InputSource, InputSources, UserInput};
use bevy::{input::touch::Touches, math::Vec2, time::Time};

impl InputSource for Touches {}

/// Return touch position for the first finger in screen coordinates.
pub struct TouchPositionInput {
    id: Option<u64>,
    value: Option<Vec2>,
}

impl TouchPositionInput {
    pub fn new() -> Self {
        Self { id: None, value: None }
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

            self.value = finger.map(|f| f.position());
        }
    }
}

impl DualAxisLike for TouchPositionInput {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}
