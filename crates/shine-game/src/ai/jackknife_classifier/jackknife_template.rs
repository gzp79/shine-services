use core::f32;

use crate::ai::{JackknifeConfig, JackknifeFeatures, JackknifePointMath};
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
        let features = JackknifeFeatures::from_points(points, config);
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

            let range_min = if i >= radius { i - radius } else { 0 };
            let range_max = (i + radius + 1).min(trajectory.len());

            for j in range_min..range_max {
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

    /// Add a new sample for a gesture from points. Multiple samples can be added for the same gesture ID.
    pub fn add_template(&mut self, id: GestureId, points: &[V]) -> &mut Self {
        let template = JackknifeTemplate::from_points(&self.config, id, points);
        self.templates.push(template);
        self
    }
}
