use crate::ai::{
    windowed_range, JackknifeConfig, JackknifeFeatures, JackknifeMethod, JackknifePointMath, JackknifeTemplate,
    JackknifeTemplateSet,
};

/// Some internal state of the classifier.
pub struct JackknifeClassifierInternals<'a> {
    pub correction_factors: &'a [f32],
    pub lower_bounds: &'a [(usize, f32)],
    pub cost_size: (usize, usize),
    pub cost: &'a [f32],
}

pub struct JackknifeClassifier<V>
where
    V: JackknifePointMath<V>,
{
    sample_features: Option<JackknifeFeatures<V>>,

    correction_factors: Vec<f32>,
    lower_bounds: Vec<(usize, f32)>,
    cost_size: (usize, usize),
    cost: Vec<f32>,

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
            cost_size: (0, 0),
            cost: Vec::new(),
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
            cost_size: self.cost_size,
            cost: &self.cost,
        }
    }

    /// Return the result of the last classification.
    pub fn classification(&self) -> Option<(usize, f32)> {
        self.classification
    }

    pub fn clear(&mut self) {
        self.sample_features = None;

        self.cost_size = (0, 0);
        self.correction_factors.clear();
        self.lower_bounds.clear();
        self.cost.clear();

        self.classification = None;
    }

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
                cf * self.lower_bound(&config, &sample_features.trajectory, template)
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

            /*if *lb > template.rejection_threshold {
                continue;
            }*/

            if lb > best.map(|(_, score)| score).unwrap_or(f32::INFINITY) {
                continue;
            }

            let mut score = self.correction_factors[id];
            score *= self.dtw(config, &sample_features.trajectory, &template.features().trajectory);

            /*if score > template.rejection_threshold {
                continue;
            }*/

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

    fn init_cost(&mut self, size1: usize, size2: usize) {
        let v1 = size1 + 1;
        let v2 = size2 + 1;
        self.cost_size = (v1, v2);
        self.cost.resize(v1 * v2, f32::INFINITY);
        self.cost.fill(f32::INFINITY);

        *self.cost_mut(0, 0) = 0.0;
    }

    fn cost(&self, i: usize, j: usize) -> f32 {
        debug_assert!(i < self.cost_size.0 && j < self.cost_size.1);
        self.cost[i * self.cost_size.1 + j]
    }

    fn cost_mut(&mut self, i: usize, j: usize) -> &mut f32 {
        debug_assert!(i < self.cost_size.0 && j < self.cost_size.1);
        &mut self.cost[i * self.cost_size.1 + j]
    }

    fn dtw(&mut self, config: &JackknifeConfig, v1: &[V], v2: &[V]) -> f32 {
        self.init_cost(v1.len(), v2.len());

        // using DP to find solution
        for i in 1..=v1.len() {
            for j in windowed_range(v2.len() + 1, i, config.dtw_radius) {
                // pick minimum cost path (neighbor) to extend to this ii, jj element
                let minimum: f32 = (self.cost(i - 1, j)) // repeat v1 element
                    .min(self.cost(i, j - 1)) // repeat v2 element
                    .min(self.cost(i - 1, j - 1)); // don't repeat either
                *self.cost_mut(i, j) = minimum;
                match config.method {
                    JackknifeMethod::InnerProduct => {
                        *self.cost_mut(i, j) += 1.0 - v1[i - 1].dot(&v2[j - 1]);
                    }
                    JackknifeMethod::EuclideanDistance => {
                        *self.cost_mut(i, j) += v1[i - 1].distance_square(&v2[j - 1]);
                    }
                }
            }
        }

        self.cost(v1.len(), v2.len())
    }
}
