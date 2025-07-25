use core::fmt;
use rand::Rng;
use std::ops;

pub trait JackknifePoint:
    Clone + ops::Index<usize, Output = f32> + ops::IndexMut<usize, Output = f32> + fmt::Debug
{
    fn dimension(&self) -> usize;

    #[must_use]
    fn zero(dimension: usize) -> Self;
    #[must_use]
    fn splat(dimension: usize, value: f32) -> Self;

    #[must_use]
    fn from_sub(a: &Self, b: &Self) -> Self;
    #[must_use]
    fn from_add(a: &Self, b: &Self) -> Self;

    #[must_use]
    fn sub(self, other: &Self) -> Self;
    #[must_use]
    fn add(self, other: &Self) -> Self;
    #[must_use]
    fn add_abs(self, other: &Self) -> Self;
    #[must_use]
    fn mul(self, other: f32) -> Self;
    #[must_use]
    fn div(self, other: f32) -> Self;

    #[must_use]
    fn div_component(self, other: &Self) -> Self;
    #[must_use]
    fn min_component(self, other: &Self) -> Self;
    #[must_use]
    fn max_component(self, other: &Self) -> Self;

    #[must_use]
    fn normalize(self) -> Self;
    fn length(&self) -> f32;
    fn dot(&self, other: &Self) -> f32;
    fn distance_square(&self, other: &Self) -> f32;

    /// Interpolate between two points, where 0 <= t <= 1,
    /// t = 0 => a, and t = 1 => b.
    #[must_use]
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
}

pub trait JackknifePointMath<V>: JackknifePoint {
    fn path_length(points: &[V]) -> f32;
    fn resample(points: &[V], count: usize) -> Vec<V>;
    fn stochastic_resample(points: &[V], count: usize, variance: f32) -> Vec<V>;
    fn stochastic_variance(points: &[V], n: usize, variance: f32, remove: usize) -> Vec<V>;
    fn z_normalize(points: &mut [V]);
}

impl<V> JackknifePointMath<V> for V
where
    V: JackknifePoint,
{
    fn path_length(points: &[V]) -> f32 {
        let mut length = 0.0;
        for i in 1..points.len() {
            length += V::from_sub(&points[i], &points[i - 1]).length();
        }
        length
    }

    /// Uniformly resample the trajectory into `count` equally spaced points.
    fn resample(points: &[V], count: usize) -> Vec<V> {
        let path_distance = V::path_length(points);

        let interval = 1.0 / (count - 1) as f32;
        let interval_distance = path_distance * interval;
        let mut remaining_distance = path_distance * interval;
        let mut prev = points[0].clone();
        let mut result = Vec::new();

        result.push(points[0].clone());
        let mut i = 1;
        while i < points.len() && result.len() < count {
            let distance = V::from_sub(&points[i], &prev).length();

            if distance < remaining_distance {
                prev = points[i].clone();
                remaining_distance -= distance;
                i += 1;
                continue;
            }

            // Interpolate a new point between the last point and the current point.
            let mut ratio = remaining_distance / distance;
            if ratio > 1.0 || ratio.is_nan() {
                ratio = 1.0;
            }
            let new_point = V::lerp(&prev, &points[i], ratio);

            result.push(new_point.clone());

            // If we have enough points, we can stop.
            if result.len() == count {
                break;
            }

            // Setup for the next interval.
            prev = new_point;
            remaining_distance = interval_distance;
        }

        // If we have not enough points, we add the last one.
        if result.len() < count {
            result.push(points[points.len() - 1].clone());
        }

        assert_eq!(result.len(), count);
        result
    }

    /// Resample a trajectory into `count` points, adding random variance to the spacing between points.
    fn stochastic_resample(points: &[V], count: usize, variance: f32) -> Vec<V> {
        let path_distance = V::path_length(points);

        let intervals = {
            let mut intervals = Vec::with_capacity(count - 1);
            let mut rng = rand::rng();
            let mut sum = 0.0;
            // the variance of the uniform distribution is 1/12, thus we scale it to the desired variance
            let b = (12.0 * variance).sqrt();
            for _ in 0..(count - 1) {
                let rr = rng.random_range(0.0..1.0);
                let value = 1.0 + rr * b;
                sum += value;
                intervals.push(value);
            }
            intervals.iter_mut().for_each(|v| *v /= sum);
            debug_assert!(intervals.iter().sum::<f32>() - 1.0 < 1e-4);
            intervals
        };

        let mut remaining_distance = path_distance * intervals[0];
        let mut prev = points[0].clone();
        let mut result = Vec::new();

        result.push(points[0].clone());
        let mut i = 1;
        while i < points.len() && result.len() < count {
            let distance = V::from_sub(&points[i], &prev).length();

            if distance < remaining_distance {
                prev = points[i].clone();
                remaining_distance -= distance;
                i += 1;
                continue;
            }

            // Interpolate a new point between the last point and the current point.
            let mut ratio = remaining_distance / distance;
            if ratio > 1.0 || ratio.is_nan() {
                ratio = 1.0;
            }
            let new_point = V::lerp(&prev, &points[i], ratio);

            result.push(new_point.clone());

            // If we have enough points, we can stop.
            if result.len() == count {
                break;
            }

            // Setup for the next interval.
            prev = new_point;
            remaining_distance = path_distance * intervals[result.len() - 1];
        }

        // If we have not enough points, we add the last one.
        if result.len() < count {
            result.push(points[points.len() - 1].clone());
        }

        assert_eq!(result.len(), count);
        result
    }

    /// Perform gesture path stochastic resampling (GPSR) to create a synthetic variation of the given trajectory.
    ///
    /// Eugene M. Taranta II, Mehran Maghoumi, Corey R. Pittman, Joseph J. LaViola Jr.
    /// "A Rapid Prototyping Approach to Synthetic Data Generation For Improved 2D Gesture Recognition"
    /// Proceedings of the 29th Annual Symposium on User Interface Software and Technology
    /// 2016
    ///
    fn stochastic_variance(points: &[V], count: usize, variance: f32, remove: usize) -> Vec<V> {
        let dimension = points[0].dimension();

        // Create a non-uniformly resampled trajectory.
        let mut resample = V::stochastic_resample(points, count + remove, variance);

        // Remove random points to simulate cutting corners.
        let mut rng = rand::rng();
        for _ in 0..remove {
            let remove_index = rng.random_range(0..resample.len());
            resample.remove(remove_index);
        }

        // Convert the resampled trajectory into a set of direction vectors.
        let mut result = Vec::with_capacity(count);

        let mut point = V::zero(dimension);
        result.push(point.clone());

        for i in 1..count {
            let delta = V::from_sub(&resample[i], &resample[i - 1]).normalize();
            point = point.add(&delta);
            result.push(point.clone());
        }

        result
    }

    fn z_normalize(points: &mut [V])
    where
        V: JackknifePointMath<V>,
    {
        let n = points.len();
        let m = points[0].dimension();

        // Estimate the component-wise mean.
        let mut mean = V::zero(m);
        for point in points.iter() {
            mean = mean.add(point);
        }
        mean = mean.div(n as f32);

        // Estimate the component-wise variance
        let mut var = V::zero(m);
        for point in points.iter() {
            for j in 0..m {
                let diff = point[j] - mean[j];
                var[j] += diff * diff;
            }
        }
        var = var.div((n - 1) as f32);

        // If variance is zero, all values are nearly identical, so setting the divisor to 1 avoids division by zero and preserves the normalized value as approximately zero.
        const EPS: f32 = 1e-8;
        for i in 0..m {
            var[i] = if var[i] < EPS { 1.0 } else { var[i].sqrt() };
        }

        // last, z-score normalize all points
        for point in points.iter_mut() {
            *point = V::from_sub(point, &mean).div_component(&var);
        }
    }
}
