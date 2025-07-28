use crate::math::{
    statistics::{self, RunningMoments},
    CostMatrix, JackknifeConfig, JackknifeFeatures, JackknifePointMath,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{io, ops};

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

pub struct TrainingSamples<V>
where
    V: JackknifePointMath<V>,
{
    pub is_positive: bool,
    pub samples: Vec<Vec<V>>,
    pub dwt_scores: Vec<f32>,
    pub corrected_scores: Vec<f32>,
}

impl<V> TrainingSamples<V>
where
    V: JackknifePointMath<V>,
{
    fn new(is_positive: bool, sample_count: usize) -> Self {
        Self {
            is_positive,
            samples: Vec::with_capacity(sample_count),
            dwt_scores: Vec::with_capacity(sample_count),
            corrected_scores: Vec::with_capacity(sample_count),
        }
    }
}

pub struct TrainLegend<V>(pub Vec<Vec<TrainingSamples<V>>>)
where
    V: JackknifePointMath<V>;

impl<V> Default for TrainLegend<V>
where
    V: JackknifePointMath<V>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<V> TrainLegend<V>
where
    V: JackknifePointMath<V>,
{
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn dump<F>(&self, file: &mut dyn io::Write, i: usize, header: F) -> io::Result<()>
    where
        F: Fn(usize, bool, bool) -> String,
    {
        let template = &self.0[i];
        // write header
        let mut max_row = 0;
        for (j, samples) in template.iter().enumerate() {
            write!(
                file,
                "{},{},",
                header(j, samples.is_positive, false),
                header(j, samples.is_positive, true),
            )?;

            if samples.dwt_scores.len() > max_row {
                max_row = samples.samples.len();
            }
            if samples.corrected_scores.len() > max_row {
                max_row = samples.corrected_scores.len();
            }
        }
        writeln!(file)?;

        // write values
        for i in 0..max_row {
            for samples in template.iter() {
                let dwt_score = samples.dwt_scores.get(i).map(|v| v.to_string()).unwrap_or_default();
                let corrected_score = samples
                    .corrected_scores
                    .get(i)
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                write!(file, "{dwt_score},{corrected_score},")?;
            }
            writeln!(file)?;
        }
        Ok(())
    }
}

impl<V> ops::Deref for TrainLegend<V>
where
    V: JackknifePointMath<V>,
{
    type Target = Vec<Vec<TrainingSamples<V>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V> ops::Index<usize> for TrainLegend<V>
where
    V: JackknifePointMath<V>,
{
    type Output = Vec<TrainingSamples<V>>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
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
    moments: (f32, f32),
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
            rejection_threshold: f32::INFINITY,
            moments: (f32::INFINITY, f32::NEG_INFINITY),
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

    pub fn moments(&self) -> (f32, f32) {
        self.moments
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
                minimum.min_component_assign(point);
                maximum.max_component_assign(point);
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
    ///   - `gpsr_count`: The number of the resampled GPSR points.
    ///   - `variance`: The variance on the intervals to use for stochastic resampling.
    ///   - `gpsr_remove`: The number of GPSR points removal for corner cuts.
    pub fn train(
        &mut self,
        batch_size: usize,
        gpsr_count: usize,
        variance: f32,
        gpsr_remove: usize,
        jitter: f32,
        mut legend: Option<&mut TrainLegend<V>>,
    ) {
        let mut cost_matrix = CostMatrix::new();
        let mut sample_moments = Vec::new();
        let mut thresholds = Vec::with_capacity(self.templates.len());
        let mut moments = Vec::with_capacity(self.templates.len());

        if let Some(TrainLegend(legend)) = legend.as_mut() {
            legend.clear();
        }

        for (i, template) in self.templates.iter().enumerate() {
            log::info!("Training template {i} for gesture {}", template.id().id());

            sample_moments.clear();
            if let Some(TrainLegend(legend)) = legend.as_mut() {
                legend.push(Vec::with_capacity(self.templates.len()));
            }

            for (j, sample_template) in self.templates.iter().enumerate() {
                log::info!(
                    "  Generating scores for template {j} for gesture {}",
                    sample_template.id().id()
                );
                if let Some(TrainLegend(legend)) = legend.as_mut() {
                    legend[i].push(TrainingSamples::new(template.id() == sample_template.id(), batch_size));
                }

                let mut moments = RunningMoments::new();
                for _ in 0..batch_size {
                    let synthetic_sample = V::stochastic_variance(
                        sample_template.resampled_points(),
                        gpsr_count,
                        variance,
                        gpsr_remove,
                        jitter,
                    );
                    let samples_features = JackknifeFeatures::from_points(&self.config, &synthetic_sample);

                    let cf = samples_features.correction_factor(
                        &template.features(),
                        self.config.abs_correction,
                        self.config.extent_correction,
                    );
                    let dwt_score = samples_features.dwt_score(
                        &mut cost_matrix,
                        &template.features(),
                        self.config.dtw_radius,
                        self.config.method,
                    );

                    moments.add(cf * dwt_score);

                    if let Some(TrainLegend(legend)) = legend.as_mut() {
                        legend[i][j].samples.push(synthetic_sample);
                        legend[i][j].dwt_scores.push(dwt_score);
                        legend[i][j].corrected_scores.push(cf * dwt_score);
                    }
                }

                sample_moments.push(moments);
            }

            log::info!(
                "  Template {i} mean: {}, std_dev: {}",
                sample_moments[i].mean(),
                sample_moments[i].std_dev()
            );

            moments.push((sample_moments[i].mean(), sample_moments[i].std_dev()));

            // find the closest (negative) sample
            let min_sample_id = sample_moments
                .iter()
                .enumerate()
                .filter(|(i, _)| self.templates[*i].id() != template.id()) // only negative samples
                .map(|(i, m)| (i, m.mean())) // we are interested in the mean
                .min_by(|(_, m1), (_, m2)| m1.partial_cmp(m2).unwrap()) // find the minimum
                .map(|(i, _)| i);

            let threshold = if let Some(j) = min_sample_id {
                log::info!(
                    "  Closest template {j} mean: {}, std_dev: {}",
                    sample_moments[j].mean(),
                    sample_moments[j].std_dev()
                );

                statistics::bayesian_threshold(
                    sample_moments[i].mean(),
                    sample_moments[i].std_dev(),
                    1.0,
                    sample_moments[j].mean(),
                    sample_moments[j].std_dev(),
                    1.0,
                )
            } else {
                log::warn!("  No samples generated for template {i}, skipping rejection threshold");
                f32::INFINITY
            };

            log::info!("  Template {i} threshold: {threshold}",);
            thresholds.push(threshold);
        }

        // Apply th training data to the templates
        for (i, template) in self.templates.iter_mut().enumerate() {
            log::info!(
                "  Template {} rejection threshold: {}, moments: {:?}",
                template.id().id(),
                thresholds[i],
                moments[i]
            );
            template.rejection_threshold = thresholds[i];
            template.moments = moments[i];
        }
    }
}
