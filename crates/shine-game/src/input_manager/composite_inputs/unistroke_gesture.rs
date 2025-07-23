use crate::{
    ai::{JackknifeClassifier, JackknifeConfig, JackknifeFeatures, JackknifeTemplateSet},
    input_manager::{ClassificationLike, DualAxisLike, InputSources, UserInput},
};
use bevy::{math::Vec2, time::Time};
use std::borrow::Cow;

/// Unistroke gesture recognizer for the given positional input.
/// Recognizer incorporates all the position up to a None and triggers the appropriate
/// gesture with a score.
pub struct UnistrokeGesture<P = Box<dyn DualAxisLike>>
where
    P: DualAxisLike,
{
    name: Option<String>,
    sensitivity: f32,
    input: P,
    points: Vec<Vec2>,

    classifier: JackknifeClassifier<Vec2>,
    sample_features: Option<JackknifeFeatures<Vec2>>,
    gesture_templates: JackknifeTemplateSet<Vec2>,
}

impl<P> UnistrokeGesture<P>
where
    P: DualAxisLike,
{
    /// Creates a new unistroke gesture recognizer.
    ///
    /// # Arguments
    ///
    /// * `input` - The positional input to use for the gesture recognizer.
    /// * `sensitivity` - The sensitivity of the gesture recognizer measured in the same unit as the input.
    ///
    pub fn new(input: P, sensitivity: f32) -> Self {
        let config = JackknifeConfig::inner_product();

        Self {
            name: None,
            sensitivity,
            input,
            points: Vec::new(),

            classifier: JackknifeClassifier::new(config.clone()),
            sample_features: None,
            gesture_templates: JackknifeTemplateSet::new(config),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_gesture(mut self, gesture: &[Vec2]) -> Self {
        self.gesture_templates.add_template_from_points(gesture);
        self
    }

    pub fn points(&self) -> &[Vec2] {
        &self.points
    }

    pub fn resampled_points(&self) -> Option<&[Vec2]> {
        self.sample_features.as_ref().map(|f| &f.points[..])
    }
}

impl<P> UserInput for UnistrokeGesture<P>
where
    P: DualAxisLike,
{
    fn type_name(&self) -> &'static str {
        "UnistrokeGesture"
    }

    fn name(&self) -> Cow<'_, str> {
        self.name.as_deref().unwrap_or("").into()
    }

    fn visit_recursive<'a>(&'a self, depth: usize, visitor: &mut dyn FnMut(usize, &'a dyn UserInput) -> bool) -> bool {
        visitor(depth, self) && self.input.visit_recursive(depth + 1, visitor)
    }

    fn integrate(&mut self, input: &InputSources) {
        self.input.integrate(input);
    }
}

impl<P> ClassificationLike for UnistrokeGesture<P>
where
    P: DualAxisLike,
{
    fn process(&mut self, time: &Time) -> Option<(usize, f32)> {
        let position = self.input.process(time);
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
            self.sample_features = Some(JackknifeFeatures::from_points(&self.points, &self.classifier.config()));
            let value = self.classifier.classify_features(
                self.sample_features.as_ref().unwrap(),
                self.gesture_templates.templates(),
            );
            self.points.clear();
            value
        } else {
            None
        }
    }
}
