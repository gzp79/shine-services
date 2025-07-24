use crate::math::{JackknifeConfig, JackknifeMethod, JackknifePointMath};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Store extracted information and features from a sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "V: Serialize + DeserializeOwned")]
pub struct JackknifeFeatures<V>
where
    V: JackknifePointMath<V>,
{
    /// Input points resampled to N points
    pub points: Vec<V>,

    /// Trajectory used by the DTW algorithm.
    /// Depending on DTW measure it can be either m-dimensional points or direction vectors.
    pub trajectory: Vec<V>,

    /// Correction factor vector from the normalized distance traversed by each component.
    pub cf_abs: V,

    /// Correction factor vector from the normalized bounding box size
    pub cf_extent: V,
}

impl<V> JackknifeFeatures<V>
where
    V: JackknifePointMath<V>,
{
    /// Compute the features from a set of input points.
    pub fn from_points(sample_points: &[V], config: &JackknifeConfig) -> Self {
        let m = sample_points[0].dimension();
        let points = V::resample(sample_points, config.resample_count);

        let mut trajectory = Vec::with_capacity(config.resample_count);
        let mut abs = V::zero(m);
        let mut minimum = points[0].clone();
        let mut maximum = points[0].clone();

        for i in 1..config.resample_count {
            // In-between point direction vector.
            let mut vec = V::from_sub(&points[i], &points[i - 1]);

            abs = abs.add_abs(&vec);
            minimum = minimum.min_component(&points[i]);
            maximum = maximum.max_component(&points[i]);

            // Save the points or direction vectors,
            // depending on the selected measure.
            match config.method {
                JackknifeMethod::InnerProduct => {
                    vec = vec.normalize();
                    trajectory.push(vec);
                }
                JackknifeMethod::EuclideanDistance => {
                    if i == 1 {
                        trajectory.push(points[0].clone());
                    }

                    trajectory.push(points[i].clone());
                }
            }
        }

        if config.z_normalize {
            V::z_normalize(&mut trajectory);
        }
        abs = abs.normalize();
        let extent = V::from_sub(&maximum, &minimum).normalize();

        Self {
            points,
            trajectory,
            cf_abs: abs,
            cf_extent: extent,
        }
    }
}
