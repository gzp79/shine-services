use crate::input_manager::{ActionLike, DualAxisLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::system::{Query, Res},
    input::touch::Touches,
    math::Vec2,
    time::Time,
    window::Window,
};

impl InputSource for Touches {}

/// Return touch position for the first finger in screen coordinates.
pub struct TouchPositionInput {
    id: Option<u64>,
    value: Option<Vec2>,
}

impl Default for TouchPositionInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchPositionInput {
    pub fn new() -> Self {
        Self { id: None, value: None }
    }
}

impl UserInput for TouchPositionInput {
    fn integrate(&mut self, input: &InputSources) {
        if let Some(touches) = input.get_resource::<Touches>() {
            // check if the touch is still active
            if let Some(id) = self.id {
                if touches.get_pressed(id).is_none() {
                    self.id = None;
                }
            }

            // Assign new id from the first active touch
            for touch in touches.iter() {
                if self.id.is_none() {
                    self.id = Some(touch.id());
                    break;
                }
            }

            self.value = self.id.and_then(|id| touches.get_pressed(id)).map(|t| t.position());
        }
    }
}

impl DualAxisLike for TouchPositionInput {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}

pub fn integrate_touch_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    touches: Res<Touches>,
    mut input_query: Query<&mut InputMap<A>>,
) where
    A: ActionLike,
{
    let window = window.single().expect("Only single window is supported");

    for mut input_map in input_query.iter_mut() {
        let mut input_source = InputSources::new();

        input_source.add_resource(window);
        input_source.add_resource(&*time);
        input_source.add_resource(&*touches);

        input_map.integrate(input_source);
    }
}
