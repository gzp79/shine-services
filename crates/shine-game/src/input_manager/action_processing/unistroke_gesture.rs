use crate::{
    ai::{GestureId, JackknifeClassifier, JackknifeTemplateSet},
    input_manager::{ActionLike, ActionState},
};
use bevy::{
    ecs::{
        component::Component,
        error::BevyError,
        system::{Query, Res},
    },
    math::Vec2,
    time::Time,
};
use std::collections::HashMap;

/// A set of gesture templates to recognized.
#[derive(Component)]
pub struct GestureSet {
    pub template_set: JackknifeTemplateSet<Vec2>,
}

/// Process position inputs for unistroke gesture recognition.
/// The input positions are collected up to the first None and then classified
/// against a set of templates using the Jackknife algorithm.
#[derive(Component)]
pub struct UnistrokeGesture<A>
where
    A: ActionLike,
{
    sensitivity: f32,
    input_action: A,
    output_buttons: HashMap<GestureId, A>,
    points: Vec<Vec2>,

    classifier: JackknifeClassifier<Vec2>,
    value: Option<(GestureId, usize, f32)>,
}

impl<A> UnistrokeGesture<A>
where
    A: ActionLike,
{
    /// Creates a new unistroke gesture recognizer.
    ///
    /// # Arguments
    ///
    /// * `input` - The positional input action to use for the gesture recognizer.
    /// * `sensitivity` - The sensitivity of the gesture recognizer measured in the same unit as the input.
    ///
    pub fn new(input_action: A, sensitivity: f32) -> Self {
        Self {
            sensitivity,
            input_action,
            output_buttons: HashMap::new(),

            points: Vec::new(),
            classifier: JackknifeClassifier::new(),
            value: None,
        }
    }

    pub fn with_button_target(mut self, gesture_id: GestureId, button: A) -> Self {
        self.output_buttons.insert(gesture_id, button);
        self
    }

    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    pub fn resampled_points(&self) -> Option<&[Vec2]> {
        self.classifier.sample_features().map(|f| &f.points[..])
    }

    pub fn classification(&self) -> Option<(GestureId, usize, f32)> {
        self.value
    }

    fn add_point(&mut self, position: Option<Vec2>, gesture_set: &GestureSet) -> Option<GestureId> {
        if let Some(position) = position {
            if self
                .points
                .last()
                .map(|prev| (prev - position).length_squared() > self.sensitivity)
                .unwrap_or(true)
            {
                self.points.push(position);
            }
            None
        } else if self.points.len() > 1 {
            self.classifier.classify(&gesture_set.template_set, &self.points);

            self.value = self
                .classifier
                .classification()
                .map(|(idx, score)| (gesture_set.template_set.templates()[idx].id(), idx, score));
            self.points.clear();
            log::debug!("Unistroke gesture recognition result: {:?}", self.value);

            self.value.as_ref().map(|(gesture_id, _, _)| *gesture_id)
        } else {
            None
        }
    }
}

pub fn detect_unistroke_gesture<A>(
    mut gesture_q: Query<(&GestureSet, &mut ActionState<A>, &mut UnistrokeGesture<A>)>,
    time: Res<Time>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    for (gesture_set, mut action_state, mut recognizer) in gesture_q.iter_mut() {
        let pos = action_state.try_dual_axis_value(&recognizer.input_action);
        let gesture = recognizer.add_point(pos, gesture_set);

        for (&gesture_id, action) in recognizer.output_buttons.iter() {
            let pressed = Some(gesture_id) == gesture;
            action_state
                .set_button(action.clone())
                .update(Some(pressed), time.elapsed_secs());
        }
    }

    Ok(())
}
