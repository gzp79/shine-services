use crate::ai::{JackknifeConfig, JackknifeFeatures, JackknifeMethod, JackknifePointMath, JackknifeTemplate};
use std::marker::PhantomData;

pub struct JackknifeClassifier<V>
where
    V: JackknifePointMath<V>,
{
    config: JackknifeConfig,

    cost_size: Option<(usize, usize)>,
    cost: Vec<f32>,
    _ph: PhantomData<V>,
}

impl<V> JackknifeClassifier<V>
where
    V: JackknifePointMath<V>,
{
    pub fn new(config: JackknifeConfig) -> Self {
        Self {
            config,
            cost_size: None,
            cost: Vec::new(),
            _ph: PhantomData,
        }
    }

    pub fn config(&self) -> &JackknifeConfig {
        &self.config
    }

    pub fn classify(&mut self, sample_points: &[V], templates: &[JackknifeTemplate<V>]) -> Option<(usize, f32)> {
        if sample_points.len() < 2 {
            log::warn!("Too few sample points, classification not possible");
            return None;
        }

        let sample_features = JackknifeFeatures::from_points(sample_points, &self.config);

        self.classify_features(&sample_features, templates)
    }

    pub fn classify_features(
        &mut self,
        sample_features: &JackknifeFeatures<V>,
        templates: &[JackknifeTemplate<V>],
    ) -> Option<(usize, f32)> {
        let mut correction_factors = Vec::with_capacity(templates.len());
        let mut lower_bounds = Vec::with_capacity(templates.len());

        for (i, template) in templates.iter().enumerate() {
            let mut cf = 1.0;

            let template_features = template.features();

            if self.config.abs_correction {
                cf *= 1.0 / sample_features.cf_abs.dot(&template_features.cf_abs).max(0.01);
            }

            if self.config.extent_correction {
                cf *= 1.0 / sample_features.cf_extent.dot(&template_features.cf_extent).max(0.01);
            }
            correction_factors.push(cf);

            let lb = if self.config.use_lower_bound {
                cf * self.lower_bound(&sample_features.trajectory, template)
            } else {
                // a negative value disables any lower_bound logic
                -1.0
            };
            lower_bounds.push((i, lb));
        }

        // Without lower bounds, we can skip sorting, as it would result in the same order
        if self.config.use_lower_bound {
            lower_bounds.sort_by(|&a, &b| a.1.partial_cmp(&b.1).unwrap());
        }

        let mut best: Option<(usize, f32)> = None;

        for i in 0..lower_bounds.len() {
            let (id, lb) = lower_bounds[i];
            let template = &templates[id];

            /*if lb > template.rejection_threshold {
                continue;
            }*/

            if lb > best.map(|(_, score)| score).unwrap_or(f32::INFINITY) {
                continue;
            }

            let mut score = correction_factors[id];
            score *= self.dtw(&sample_features.trajectory, &template.features().trajectory);

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

        best
    }

    fn lower_bound(&self, trajectory: &[V], template: &JackknifeTemplate<V>) -> f32 {
        todo!()
    }

    fn init_cost(&mut self, size1: usize, size2: usize) {
        let v1 = size1 + 1;
        let v2 = size2 + 1;
        self.cost_size = Some((v1, v2));
        self.cost.clear();
        self.cost.resize(v1 * v2, f32::INFINITY);

        *self.cost_mut(0, 0) = 0.0;
    }

    fn cost(&self, i: usize, j: usize) -> f32 {
        debug_assert!(i < self.cost_size.unwrap().0 && j < self.cost_size.unwrap().1);
        self.cost[i * self.cost_size.unwrap().1 + j]
    }

    fn cost_mut(&mut self, i: usize, j: usize) -> &mut f32 {
        debug_assert!(i < self.cost_size.unwrap().0 && j < self.cost_size.unwrap().1);
        &mut self.cost[i * self.cost_size.unwrap().1 + j]
    }

    fn dtw(&mut self, v1: &[V], v2: &[V]) -> f32 {
        self.init_cost(v1.len(), v2.len());

        // using DP to find solution
        for i in 1..=v1.len() {
            let window = {
                let min = if i > self.config.dtw_radius {
                    i - self.config.dtw_radius
                } else {
                    1
                };

                let max = (i + self.config.dtw_radius).min(v2.len());
                min..=max
            };
            for j in window {
                // pick minimum cost path (neighbor) to extend to this ii, jj element
                let minimum: f32 = (self.cost(i - 1, j)) // repeat v1 element
                    .min(self.cost(i, j - 1)) // repeat v2 element
                    .min(self.cost(i - 1, j - 1)); // don't repeat either
                *self.cost_mut(i, j) = minimum;
                match self.config.method {
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
