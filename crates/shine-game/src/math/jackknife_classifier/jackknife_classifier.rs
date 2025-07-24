use crate::math::{
    CostMatrix, JackknifeConfig, JackknifeFeatures, JackknifeMethod, JackknifePointMath, JackknifeTemplate,
    JackknifeTemplateSet,
};

/// Some internal state of the classifier.
pub struct JackknifeClassifierInternals<'a> {
    pub correction_factors: &'a [f32],
    pub lower_bounds: &'a [(usize, f32)],
    pub cost_matrix: &'a CostMatrix,
}

pub struct JackknifeClassifier<V>
where
    V: JackknifePointMath<V>,
{
    sample_features: Option<JackknifeFeatures<V>>,

    correction_factors: Vec<f32>,
    lower_bounds: Vec<(usize, f32)>,
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
            lower_bounds: Vec::new(),
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
            lower_bounds: &self.lower_bounds,
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
        self.lower_bounds.clear();
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

        let sample_features = JackknifeFeatures::from_points(sample_points, config);

        self.correction_factors.reserve(templates.len());
        self.lower_bounds.reserve(templates.len());

        for (i, template) in templates.iter().enumerate() {
            let mut cf = 1.0;

            let template_features = template.features();

            if config.abs_correction {
                cf *= 1.0 / sample_features.cf_abs.dot(&template_features.cf_abs).max(0.01);
            }

            if config.extent_correction {
                cf *= 1.0 / sample_features.cf_extent.dot(&template_features.cf_extent).max(0.01);
            }
            self.correction_factors.push(cf);

            let lb = if config.use_lower_bound {
                cf * self.lower_bound(config, &sample_features.trajectory, template)
            } else {
                // a negative value disables any lower_bound logic
                -1.0
            };
            self.lower_bounds.push((i, lb));
        }

        // Without lower bounds, we can skip sorting, as it would result in the same order
        if config.use_lower_bound {
            self.lower_bounds.sort_by(|&a, &b| a.1.partial_cmp(&b.1).unwrap());
        }

        let mut best: Option<(usize, f32)> = None;

        for i in 0..self.lower_bounds.len() {
            let (id, lb) = self.lower_bounds[i];
            let template = &templates[id];

            if lb > template.rejection_threshold() {
                continue;
            }

            if lb > best.map(|(_, score)| score).unwrap_or(f32::INFINITY) {
                continue;
            }

            let mut score = self.correction_factors[id];
            let dwt_score = match config.method {
                JackknifeMethod::InnerProduct => self.cost_matrix.dtw(
                    &sample_features.trajectory,
                    &template.features().trajectory,
                    config.dtw_radius,
                    |a: &V, b: &V| 1.0 - a.dot(b),
                ),
                JackknifeMethod::EuclideanDistance => self.cost_matrix.dtw(
                    &sample_features.trajectory,
                    &template.features().trajectory,
                    config.dtw_radius,
                    |a: &V, b: &V| a.distance_square(b),
                ),
            };
            score *= dwt_score;

            if score > template.rejection_threshold() {
                continue;
            }

            if let Some((_, best_score)) = best {
                if score < best_score {
                    best = Some((id, score));
                }
            } else {
                best = Some((id, score));
            }
        }

        self.sample_features = Some(sample_features);
        self.classification = best;

        self.classification
    }

    fn lower_bound(&self, config: &JackknifeConfig, trajectory: &[V], template: &JackknifeTemplate<V>) -> f32 {
        let mut lb = 0.0;

        let dimension = trajectory[0].dimension();
        let bounds = template.bounds();

        for (point, bound) in trajectory.iter().zip(bounds.iter()) {
            let mut cost = 0.0;
            let (lower, upper) = bound;

            for j in 0..dimension {
                match config.method {
                    JackknifeMethod::InnerProduct => {
                        if point[j] < 0.0 {
                            cost += point[j] * lower[j];
                        } else {
                            cost += point[j] * upper[j];
                        }
                    }
                    JackknifeMethod::EuclideanDistance => {
                        let mut diff = 0.0;

                        if point[j] < lower[j] {
                            diff = point[j] - lower[j];
                        } else if point[j] > upper[j] {
                            diff = point[j] - upper[j];
                        }

                        cost += diff * diff;
                    }
                }
            }

            // inner products are bounded
            if config.method == JackknifeMethod::InnerProduct {
                cost = 1.0 - cost.clamp(-1.0, 1.0);
            }

            lb += cost;
        }

        lb
    }
}
