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

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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
                max_row = samples.dwt_scores.len();
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
    rejection_threshold: f32,
}

impl<V> JackknifeTemplate<V>
where
    V: JackknifePointMath<V>,
{
    pub fn from_points(config: &JackknifeConfig, id: GestureId, points: &[V]) -> Self {
        let features = JackknifeFeatures::from_points(config, points);
        Self {
            id,
            features,
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

    pub fn rejection_threshold(&self) -> f32 {
        self.rejection_threshold
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
    ///   - `jitter`: The jitter to apply to the resampled points.
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

        if let Some(TrainLegend(legend)) = legend.as_mut() {
            legend.clear();
        }

        for (i, template) in self.templates.iter().enumerate() {
            log::debug!("Training template {i} for gesture {}", template.id().id());

            sample_moments.clear();
            if let Some(TrainLegend(legend)) = legend.as_mut() {
                legend.push(Vec::with_capacity(self.templates.len()));
            }

            for (j, sample_template) in self.templates.iter().enumerate() {
                log::trace!(
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
                        template.features(),
                        self.config.abs_correction,
                        self.config.extent_correction,
                    );
                    let dwt_score = samples_features.dwt_score(
                        &mut cost_matrix,
                        template.features(),
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

            log::trace!(
                "  Template {i} mean: {}, std_dev: {}",
                sample_moments[i].mean(),
                sample_moments[i].std_dev()
            );

            // Find the smallest threshold for the template from all the negative samples
            let mut threshold = f32::INFINITY;
            for j in 0..sample_moments.len() {
                //skip positive samples
                if template.id() == self.templates[j].id() {
                    continue;
                }

                let th = statistics::bayesian_threshold(
                    sample_moments[i].mean(),
                    sample_moments[i].std_dev(),
                    sample_moments[j].mean(),
                    sample_moments[j].std_dev(),
                );

                if threshold > th {
                    threshold = th;
                }
            }

            log::trace!("  Template {i} threshold: {threshold}");
            thresholds.push(threshold);
        }

        // Apply the training data to the templates
        for (i, template) in self.templates.iter_mut().enumerate() {
            log::debug!("  Template {i} rejection threshold: {}", thresholds[i]);
            template.rejection_threshold = thresholds[i];
        }
    }
}
