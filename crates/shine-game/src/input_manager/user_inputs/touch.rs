use crate::input_manager::{ActionLike, DualAxisLike, InputMap, InputSource, InputSources, UserInput};
use bevy::{
    ecs::{
        error::BevyError,
        system::{Query, Res},
    },
    input::touch::Touches,
    math::Vec2,
    time::Time,
    window::Window,
};

impl InputSource for Touches {}

pub fn integrate_touch_inputs<A>(
    time: Res<Time>,
    window: Query<&Window>,
    touches: Res<Touches>,
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
        input_source.add_resource(&*touches);

        input_map.integrate(input_source);
    }

    Ok(())
}

/// Represents touch position input for the first finger in screen coordinates.
///
/// Returns a [`Vec2`] where each component is in screen space (pixels), with Y axis pointing down.
/// This matches the convention of screen/UI coordinates, not world coordinates.
pub struct TouchPosition {
    id: Option<u64>,
    value: Option<Vec2>,
}

impl Default for TouchPosition {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchPosition {
    pub fn new() -> Self {
        Self { id: None, value: None }
    }
}

impl UserInput for TouchPosition {
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

impl DualAxisLike for TouchPosition {
    fn process(&mut self, _time: &Time) -> Option<Vec2> {
        self.value
    }
}
