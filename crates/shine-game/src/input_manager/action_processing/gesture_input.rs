use crate::{
    input_manager::{ActionLike, DetectedGesture, InputMap, InputSource, InputSources, TypedUserInput, UserInput},
    math::GestureId,
};
use bevy::{
    ecs::{
        entity::Entity,
        error::BevyError,
        system::{Query, Res},
    },
    time::Time,
};
use std::borrow::Cow;

impl InputSource for DetectedGesture {}

pub fn integrate_gesture_inputs<A>(
    time: Res<Time>,
    recognizer_q: Query<(Entity, &DetectedGesture)>,
    mut input_query: Query<&mut InputMap<A>>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    let mut input_sources = InputSources::new();
    input_sources.add_resource(&*time);

    input_sources.add_marker::<DetectedGesture>();
    for (entity, detected_gesture) in recognizer_q.iter() {
        input_sources.add_component(entity, detected_gesture);
    }

    for mut input_map in input_query.iter_mut() {
        input_map.integrate(&input_sources);
    }

    Ok(())
}

/// Represents boolean input from gestures recognition.
///
/// Returns a boolean value indicating whether the gesture is recognized.
/// If no gesture is available, returns `None`.
pub struct GestureInput {
    name: Option<String>,
    gesture: GestureId,
    pressed: bool,
}

impl GestureInput {
    pub fn new(gesture: GestureId) -> Self {
        Self {
            name: None,
            gesture,
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

impl UserInput for GestureInput {
    fn type_name(&self) -> &'static str {
        "GestureInput"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name
            .as_deref()
            .map_or_else(|| format!("{:?}", self.gesture).into(), Cow::from)
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self)
    }

    fn integrate(&mut self, input: &InputSources) {
        if input.has_marker::<DetectedGesture>() {
            // When gesture input is available, reset the pressed state and see if any detector has the gesture.
            self.pressed = false;

            for detected_gestures in input.get_all_components::<DetectedGesture>() {
                if detected_gestures.0 == Some(self.gesture) {
                    self.pressed = true;
                    break;
                }
            }
        }
    }
}

impl TypedUserInput<bool> for GestureInput {
    fn process(&mut self, _time_s: f32) -> Option<bool> {
        Some(self.pressed)
    }
}
