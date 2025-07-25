use crate::math::{statistics, CostMatrix, JackknifeConfig, JackknifeFeatures, JackknifeMethod, JackknifePointMath};
use rand::Rng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GestureId(pub usize);

impl GestureId {
    pub fn id(&self) -> usize {
        self.0
    }
}

impl From<usize> for GestureId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<GestureId> for usize {
    fn from(gesture_id: GestureId) -> Self {
        gesture_id.id()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "V: Serialize + DeserializeOwned")]
pub struct JackknifeTemplate<V>
where
    V: JackknifePointMath<V>,
{
    id: GestureId,
    features: JackknifeFeatures<V>,
    bounds: Vec<(V, V)>,
    rejection_threshold: f32,
}

impl<V> JackknifeTemplate<V>
where
    V: JackknifePointMath<V>,
{
    pub fn from_points(config: &JackknifeConfig, id: GestureId, points: &[V]) -> Self {
        let features = JackknifeFeatures::from_points(config, points);
        let bounds = Self::find_bounds(&features.trajectory, config.dtw_radius);
        Self {
            id,
            features,
            bounds,
            // an positive infinite value disables rejection
            rejection_threshold: f32::INFINITY,
        }
    }

    pub fn id(&self) -> GestureId {
        self.id
    }

    pub fn features(&self) -> &JackknifeFeatures<V> {
        &self.features
    }

    pub fn resampled_points(&self) -> &[V] {
        &self.features.points
    }

    pub fn bounds(&self) -> &[(V, V)] {
        &self.bounds
    }

    pub fn rejection_threshold(&self) -> f32 {
        self.rejection_threshold
    }

    /// Find the min and max value within the radius (Sakoe-Chiba band).
    fn find_bounds(trajectory: &[V], radius: usize) -> Vec<(V, V)> {
        let mut bounds = Vec::with_capacity(trajectory.len());

        let dimension = trajectory[0].dimension();

        // For each component
        for i in 0..trajectory.len() {
            let mut maximum = V::splat(dimension, f32::NEG_INFINITY);
            let mut minimum = V::splat(dimension, f32::INFINITY);

            let range_min = i.saturating_sub(radius);
            let range_max = (i + radius + 1).min(trajectory.len());

            for point in trajectory.iter().take(range_max).skip(range_min) {
                minimum = minimum.min_component(point);
                maximum = maximum.max_component(point);
            }

            bounds.push((minimum, maximum));
        }

        bounds
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "V: Serialize + DeserializeOwned")]
pub struct JackknifeTemplateSet<V>
where
    V: JackknifePointMath<V>,
{
    config: JackknifeConfig,
    templates: Vec<JackknifeTemplate<V>>,
}

impl<V> JackknifeTemplateSet<V>
where
    V: JackknifePointMath<V>,
{
    pub fn new(config: JackknifeConfig) -> Self {
        Self { config, templates: Vec::new() }
    }

    pub fn config(&self) -> &JackknifeConfig {
        &self.config
    }

    pub fn templates(&self) -> &[JackknifeTemplate<V>] {
        &self.templates
    }

    /// Add a new sample for a gesture from points. Multiple samples can be added for the same gesture ID.
    pub fn add_template(&mut self, id: GestureId, points: &[V]) -> &mut Self {
        let template = JackknifeTemplate::from_points(&self.config, id, points);
        self.templates.push(template);
        self
    }

    /// Learn rejection template for each gesture.
    ///
    /// * Parameters:
    ///   - `batch_size`: The number of samples to generate for each template.
    ///   - `beta`: The target F-score for the rejection threshold.
    ///   - `gpsr_count`: The number of the resampled GPSR points.
    ///   - `variance`: The variance on the intervals to use for stochastic resampling.
    ///   - `gpsr_remove`: The number of GPSR points removal for corner cuts.
    pub fn train(&mut self, batch_size: usize, beta: f32, gpsr_count: usize, variance: f32, gpsr_remove: usize) {
        let mut cost_matrix = CostMatrix::new();

        // I. Create negative samples
        let mut negative_per_templates = Vec::new();
        negative_per_templates.resize_with(self.templates.len(), || Vec::with_capacity(batch_size));

        for _ in 0..batch_size {
            // create a negative sample by splicing two random templates
            let sample_features = {
                let mut rng = rand::rng();
                let idx1 = rng.random_range(0..self.templates.len());
                let idx2 = rng.random_range(0..self.templates.len());
                if idx1 == idx2 {
                    continue;
                }

                let cnt = self.config.resample_count / 2;
                let mut synthetic = Vec::with_capacity(self.config.resample_count);
                synthetic.extend_from_slice(&self.templates[idx1].resampled_points()[..cnt]);
                synthetic.extend_from_slice(&self.templates[idx2].resampled_points()[cnt..]);

                JackknifeFeatures::from_points(&self.config, &synthetic)
            };

            // find negative score for each template
            for (i, template) in self.templates.iter().enumerate() {
                let dwt_score = match self.config.method {
                    JackknifeMethod::InnerProduct => cost_matrix.dtw(
                        &sample_features.trajectory,
                        &template.features().trajectory,
                        self.config.dtw_radius,
                        |a: &V, b: &V| 1.0 - a.dot(b),
                    ),
                    JackknifeMethod::EuclideanDistance => cost_matrix.dtw(
                        &sample_features.trajectory,
                        &template.features().trajectory,
                        self.config.dtw_radius,
                        |a: &V, b: &V| a.distance_square(b),
                    ),
                };

                if dwt_score.is_finite() {
                    negative_per_templates[i].push(dwt_score);
                } else {
                    log::warn!("Non-finite DWT score for negative sample: {dwt_score}");
                }
            }
        }

        // II. Create positive samples
        let mut positive_per_templates = Vec::new();
        positive_per_templates.resize_with(self.templates.len(), || Vec::with_capacity(batch_size));

        for (i, template) in self.templates.iter().enumerate() {
            for _ in 0..batch_size {
                let synthetic = V::stochastic_variance(template.resampled_points(), gpsr_count, variance, gpsr_remove);

                let sample_features = JackknifeFeatures::from_points(&self.config, &synthetic);
                let dwt_score = match self.config.method {
                    JackknifeMethod::InnerProduct => cost_matrix.dtw(
                        &sample_features.trajectory,
                        &template.features().trajectory,
                        self.config.dtw_radius,
                        |a: &V, b: &V| 1.0 - a.dot(b),
                    ),
                    JackknifeMethod::EuclideanDistance => cost_matrix.dtw(
                        &sample_features.trajectory,
                        &template.features().trajectory,
                        self.config.dtw_radius,
                        |a: &V, b: &V| a.distance_square(b),
                    ),
                };

                if dwt_score.is_finite() {
                    positive_per_templates[i].push(dwt_score);
                } else {
                    log::warn!("Non-finite DWT score for positive sample: {dwt_score}");
                }
            }
        }

        // III. Compute rejection threshold for each template with the given target F-score.
        for (i, template) in self.templates.iter_mut().enumerate() {
            let id = template.id.id();
            let negative_scores = &mut negative_per_templates[i];
            let positive_scores = &mut positive_per_templates[i];

            /*{
                negative_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
                positive_scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
                statistics::dump_classification_scores_to_csv(
                    &format!("../temp/jackknife_template_{}_scores.csv", id),
                    positive_scores,
                    negative_scores,
                )
                .unwrap();
            }*/

            // let's see how good the classifier is
            let auc = statistics::roc_auc(positive_scores, negative_scores);
            log::debug!("Template ID: {id}, ROC AUC: {auc:.3}");

            // Find the optimal threshold that maximizes F-score
            template.rejection_threshold =
                statistics::find_optimal_threshold(positive_scores, negative_scores, beta).unwrap_or(f32::INFINITY);
            log::debug!(
                "Template ID: {id}, Rejection Threshold: {}",
                template.rejection_threshold
            );
        }
    }
}
