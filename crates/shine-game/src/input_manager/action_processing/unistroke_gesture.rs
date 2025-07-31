use crate::{
    input_manager::{ActionLike, ActionStates, AttachedRecognizers, AttachedToGestureSet, DetectedGesture, GestureSet},
    math::{GestureId, JackknifeClassifier, JackknifeClassifierInternals},
};
use bevy::{
    ecs::{component::Component, entity::Entity, error::BevyError, query::With, system::Query},
    math::Vec2,
};

/// Process position inputs for unistroke gesture recognition.
/// The input positions are collected up to the first None and then classified
/// against a set of templates using the Jackknife algorithm.
#[derive(Component)]
#[require(DetectedGesture)]
pub struct UnistrokeGesture<A>
where
    A: ActionLike,
{
    sensitivity: f32,
    input_action: A,
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

            points: Vec::new(),
            classifier: JackknifeClassifier::new(),
            value: None,
        }
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

    pub fn internal(&self) -> JackknifeClassifierInternals<'_> {
        self.classifier.internal()
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
            self.classifier.classify(gesture_set, &self.points);

            self.value = self
                .classifier
                .classification()
                .map(|(idx, score)| (gesture_set.templates()[idx].id(), idx, score));
            self.points.clear();
            log::debug!("Unistroke gesture recognition result: {:?}", self.value);

            self.value.as_ref().map(|(gesture_id, _, _)| *gesture_id)
        } else {
            self.points.clear();
            None
        }
    }
}

pub fn detect_unistroke_gesture<A>(
    mut gesture_q: Query<(
        &GestureSet,
        &ActionStates<A>,
        &mut UnistrokeGesture<A>,
        &mut DetectedGesture,
    )>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    for (gesture_set, action_state, mut recognizer, mut detection_result) in gesture_q.iter_mut() {
        let pos = action_state.try_dual_axis_value(&recognizer.input_action);
        detection_result.0 = recognizer.add_point(pos, gesture_set);
    }

    Ok(())
}

/// Assume the gesture and state is on the parent and the children stores the recognizer.
/// It allows to use the same gesture recognizer for multiple actions.
pub fn detect_attached_unistroke_gesture<A>(
    mut gesture_q: Query<(Entity, &GestureSet, &ActionStates<A>)>,
    mut recognizer_q: Query<(Entity, &mut UnistrokeGesture<A>, &mut DetectedGesture), With<AttachedToGestureSet>>,
    attachments_q: Query<&AttachedRecognizers>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    for (gesture_entity, gesture_set, action_state) in gesture_q.iter_mut() {
        for recognizer_entity in attachments_q.iter_descendants(gesture_entity) {
            let (_, mut recognizer, mut detection_result) = recognizer_q.get_mut(recognizer_entity)?;

            let pos = action_state.try_dual_axis_value(&recognizer.input_action);
            detection_result.0 = recognizer.add_point(pos, gesture_set);
        }
    }

    Ok(())
}
