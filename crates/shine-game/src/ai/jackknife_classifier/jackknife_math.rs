use std::ops;

pub trait JackknifePoint: Clone + ops::Index<usize, Output = f32> + ops::IndexMut<usize, Output = f32> {
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

pub struct CostMatrix {
    size: (usize, usize),
    data: Vec<f32>,
}

impl CostMatrix {
    pub fn new() -> Self {
        CostMatrix { size: (0, 0), data: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.size = (0, 0);
        self.data.clear();
    }

    pub fn init(&mut self, width: usize, height: usize) {
        self.size = (width, height);
        self.data.clear();
        self.data.resize(height * width, f32::INFINITY);
        self[(0, 0)] = 0.0;
    }

    pub fn size(&self) -> (usize, usize) {
        self.size
    }
}

impl ops::Index<(usize, usize)> for CostMatrix {
    type Output = f32;

    fn index(&self, index: (usize, usize)) -> &Self::Output {
        let (i, j) = index;
        debug_assert!(i < self.size.0 && j < self.size.1);
        &self.data[i * self.size.1 + j]
    }
}

impl ops::IndexMut<(usize, usize)> for CostMatrix {
    fn index_mut(&mut self, index: (usize, usize)) -> &mut Self::Output {
        let (i, j) = index;
        debug_assert!(i < self.size.0 && j < self.size.1);
        &mut self.data[i * self.size.1 + j]
    }
}

pub trait JackknifePointMath<V>: JackknifePoint {
    fn path_length(points: &[V]) -> f32;
    fn resample(points: &[V], count: usize) -> Vec<V>;
    fn z_normalize(points: &mut [V]);

    fn dtw<F>(cost: &mut CostMatrix, v1: &[V], v2: &[V], radius: usize, distance: F) -> f32
    where
        F: Fn(&V, &V) -> f32;
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

    /// Resample a trajectory uniformly into count equidistant points.
    fn resample(points: &[V], count: usize) -> Vec<V> {
        let path_distance = V::path_length(points);

        let interval = 1.0 / (count - 1) as f32;
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

            // Now we need to interpolate between the last point
            // and the current point.
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
            remaining_distance = path_distance * interval;
        }

        // If we have not enough points, we add the last one.
        if result.len() < count {
            result.push(points[points.len() - 1].clone());
        }

        assert_eq!(result.len(), count);
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

        // now convert variance to standard deviation
        for i in 0..m {
            var[i] = var[i].sqrt();
        }

        // last, z-score normalize all points
        for point in points.iter_mut() {
            *point = V::from_sub(point, &mean).div_component(&var);
        }
    }

    fn dtw<F>(cost: &mut CostMatrix, v1: &[V], v2: &[V], radius: usize, distance: F) -> f32
    where
        F: Fn(&V, &V) -> f32,
    {
        let n = v1.len() + 1;
        let m = v2.len() + 1;

        cost.init(n, m);

        for i in 1..n {
            let range_min = if i > radius { i - radius } else { 1 };
            let range_max = (i + radius).min(m - 1);
            for j in range_min..=range_max {
                let minimum: f32 = (cost[(i - 1, j)]) // repeat v1 element
                    .min(cost[(i, j - 1)]) // repeat v2 element
                    .min(cost[(i - 1, j - 1)]); // new element
                cost[(i, j)] = minimum;
                cost[(i, j)] += distance(&v1[i - 1], &v2[j - 1]);
            }
        }

        cost[(n - 1, m - 1)]
    }
}
