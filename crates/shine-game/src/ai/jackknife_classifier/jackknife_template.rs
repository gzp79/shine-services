use crate::ai::{windowed_range, JackknifeConfig, JackknifeFeatures, JackknifePointMath};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "V: Serialize + DeserializeOwned")]
pub struct JackknifeTemplate<V>
where
    V: JackknifePointMath<V>,
{
    features: JackknifeFeatures<V>,

    bounds: Vec<(V, V)>,
}

impl<V> JackknifeTemplate<V>
where
    V: JackknifePointMath<V>,
{
    pub fn from_points(config: &JackknifeConfig, points: &[V]) -> Self {
        let features = JackknifeFeatures::from_points(points, config);
        let bounds = Self::find_bounds(&features.trajectory, config.dtw_radius);
        Self { features, bounds }
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

    /// Find the min and max value within the radius (Sakoe-Chiba band).
    fn find_bounds(trajectory: &[V], radius: usize) -> Vec<(V, V)> {
        let mut bounds = Vec::with_capacity(trajectory.len());

        let dimension = trajectory[0].dimension();

        // For each component
        for i in 0..trajectory.len() {
            let mut maximum = V::splat(dimension, f32::NEG_INFINITY);
            let mut minimum = V::splat(dimension, f32::INFINITY);

            for j in windowed_range(trajectory.len(), i, radius) {
                minimum = minimum.min_component(&trajectory[j]);
                maximum = maximum.max_component(&trajectory[j]);
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

    pub fn add_template(&mut self, points: &[V]) -> &mut Self {
        let template = JackknifeTemplate::from_points(&self.config, points);
        self.templates.push(template);
        self
    }
}
