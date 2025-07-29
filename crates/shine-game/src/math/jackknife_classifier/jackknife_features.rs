use crate::math::{CostMatrix, JackknifeConfig, JackknifeMethod, JackknifePointMath};
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

    /// Vector of correction factor from the normalized distance traversed by each component.
    pub cf_abs: V,

    /// Vector of correction factor from the normalized bounding box size
    pub cf_extent: V,
}

impl<V> JackknifeFeatures<V>
where
    V: JackknifePointMath<V>,
{
    /// Compute the features from a set of input points.
    pub fn from_points(config: &JackknifeConfig, sample_points: &[V]) -> Self {
        let m = sample_points[0].dimension();
        let points = V::resample(sample_points, config.resample_count);

        let mut trajectory = Vec::with_capacity(config.resample_count);
        let mut abs = V::zero(m);
        let mut minimum = points[0].clone();
        let mut maximum = points[0].clone();

        for i in 1..config.resample_count {
            // In-between point direction vector.
            let mut vec = V::from_sub(&points[i], &points[i - 1]);

            abs.add_abs_assign(&vec);
            minimum.min_component_assign(&points[i]);
            maximum.max_component_assign(&points[i]);

            // Save the points or direction vectors,
            // depending on the selected measure.
            match config.method {
                JackknifeMethod::InnerProduct => {
                    vec = vec.normalized();
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
        abs.normalize();
        let extent = V::from_sub(&maximum, &minimum).normalized();

        Self {
            points,
            trajectory,
            cf_abs: abs,
            cf_extent: extent,
        }
    }

    pub fn correction_factor(&self, other: &Self, abs_correction: bool, extent_correction: bool) -> f32 {
        let mut cf = 1.0;

        if abs_correction {
            cf *= 1.0 / self.cf_abs.dot(&other.cf_abs).max(0.01);
        }

        if extent_correction {
            cf *= 1.0 / self.cf_extent.dot(&other.cf_extent).max(0.01);
        }

        cf
    }

    pub fn dwt_score(&self, cost_matrix: &mut CostMatrix, other: &Self, radius: usize, method: JackknifeMethod) -> f32 {
        let distance = match method {
            JackknifeMethod::InnerProduct => |a: &V, b: &V| 1.0 - a.dot(b),
            JackknifeMethod::EuclideanDistance => |a: &V, b: &V| a.distance_square(b),
        };

        cost_matrix.dtw(&self.trajectory, &other.trajectory, radius, distance)
    }
}
