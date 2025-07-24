use crate::{
    ai::{JackknifeClassifier, JackknifeTemplateSet},
    input_manager::{ActionLike, ActionState},
};
use bevy::{
    ecs::{component::Component, error::BevyError, system::Query},
    math::Vec2,
};

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
    points: Vec<Vec2>,

    classifier: JackknifeClassifier<Vec2>,
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
        }
    }

    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    pub fn resampled_points(&self) -> Option<&[Vec2]> {
        self.classifier.sample_features().map(|f| &f.points[..])
    }

    pub fn classification(&self) -> Option<(usize, f32)> {
        self.classifier.classification()
    }

    fn add_point(&mut self, position: Option<Vec2>, gesture_set: &GestureSet) -> Option<(usize, f32)> {
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
            self.points.clear();
            self.classifier.classification()
        } else {
            None
        }
    }
}

pub fn detect_unistroke_gesture<A>(
    mut gesture_q: Query<(&GestureSet, &ActionState<A>, &mut UnistrokeGesture<A>)>,
) -> Result<(), BevyError>
where
    A: ActionLike,
{
    for (gesture_set, action_state, mut recognizer) in gesture_q.iter_mut() {
        let pos = action_state.try_dual_axis_value(&recognizer.input_action);
        if let Some((index, score)) = recognizer.add_point(pos, gesture_set) {
            log::info!("Unistroke gesture recognized: (index: {index}, confidence: {score})");
        }
    }

    Ok(())
}
