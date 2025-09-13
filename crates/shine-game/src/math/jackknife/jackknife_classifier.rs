use crate::math::jackknife::{CostMatrix, JackknifeFeatures, JackknifePointMath, JackknifeTemplateSet};
use bevy::log;

/// Some internal state of the classifier.
pub struct JackknifeClassifierInternals<'a> {
    pub correction_factors: &'a [f32],
    pub cost_matrix: &'a CostMatrix,
}

pub struct JackknifeClassifier<V>
where
    V: JackknifePointMath<V>,
{
    sample_features: Option<JackknifeFeatures<V>>,

    correction_factors: Vec<f32>,
    cost_matrix: CostMatrix,
    classification: Option<(usize, f32)>,
}

impl<V> Default for JackknifeClassifier<V>
where
    V: JackknifePointMath<V>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<V> JackknifeClassifier<V>
where
    V: JackknifePointMath<V>,
{
    pub fn new() -> Self {
        Self {
            sample_features: None,
            correction_factors: Vec::new(),
            cost_matrix: CostMatrix::new(),
            classification: None,
        }
    }

    /// Return the features of the last processed sample.
    pub fn sample_features(&self) -> Option<&JackknifeFeatures<V>> {
        self.sample_features.as_ref()
    }

    /// Return the internal state of the classifier.
    pub fn internal(&self) -> JackknifeClassifierInternals<'_> {
        JackknifeClassifierInternals {
            correction_factors: &self.correction_factors,
            cost_matrix: &self.cost_matrix,
        }
    }

    /// Return the result of the last classification.
    pub fn classification(&self) -> Option<(usize, f32)> {
        self.classification
    }

    pub fn clear(&mut self) {
        self.sample_features = None;

        self.correction_factors.clear();
        self.cost_matrix.clear();

        self.classification = None;
    }

    /// Classifies a sample against the template set, returning the best-matching template index and its cost.
    /// Note: The index refers to the template's position in the set, not its GestureId.
    pub fn classify(&mut self, template_set: &JackknifeTemplateSet<V>, sample_points: &[V]) -> Option<(usize, f32)> {
        self.clear();

        if sample_points.len() < 2 {
            log::warn!("Too few sample points, classification not possible");
            return None;
        }

        let templates = template_set.templates();
        let config = template_set.config();

        let sample_features = JackknifeFeatures::from_points(config, sample_points);

        self.correction_factors.reserve(templates.len());
        for template in templates.iter() {
            let template_features = template.features();
            let cf =
                sample_features.correction_factor(template_features, config.abs_correction, config.extent_correction);
            self.correction_factors.push(cf);
        }

        let mut best_score = f32::INFINITY;
        let mut best_i = None;
        for (i, template) in templates.iter().enumerate() {
            let dwt_score = sample_features.dwt_score(
                &mut self.cost_matrix,
                template.features(),
                config.dtw_radius,
                config.method,
            );
            if !dwt_score.is_finite() {
                log::error!("Internal error, non-finite DWT score for template {i}: {dwt_score}");
                log::error!("config.method: {:?}", config.method);
                log::error!("config.dtw_radius: {}", config.dtw_radius);
                log::error!("Sample features: {:?}", sample_features.trajectory);
                log::error!("Template features: {:?}", template.features().trajectory);
                continue;
            }
            let score = self.correction_factors[i] * dwt_score;

            log::debug!(
                "Template {:?} - Score: {score}, Threshold: {}",
                template.id(),
                template.rejection_threshold()
            );

            if score > template.rejection_threshold() {
                continue;
            }

            if score < best_score {
                best_i = Some(i);
                best_score = score;
            }
        }

        self.sample_features = Some(sample_features);
        self.classification = best_i.map(|id| (id, best_score));

        self.classification
    }
}
