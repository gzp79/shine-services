use crate::math::JackknifePoint;

impl<const N: usize> JackknifePoint for [f32; N] {
    fn dimension(&self) -> usize {
        N
    }

    fn zero(dimension: usize) -> Self {
        debug_assert_eq!(dimension, N);
        [0.0; N]
    }

    fn splat(dimension: usize, value: f32) -> Self {
        debug_assert_eq!(dimension, N);
        [value; N]
    }

    fn from_sub(a: &Self, b: &Self) -> Self {
        let mut result = [0.0; N];
        for i in 0..N {
            result[i] = a[i] - b[i];
        }
        result
    }

    fn from_add(a: &Self, b: &Self) -> Self {
        let mut result = [0.0; N];
        for i in 0..N {
            result[i] = a[i] + b[i];
        }
        result
    }

    fn sub(mut self, other: &Self) -> Self {
        for i in 0..N {
            self[i] -= other[i];
        }
        self
    }

    fn add(mut self, other: &Self) -> Self {
        for i in 0..N {
            self[i] += other[i];
        }
        self
    }

    fn add_abs(mut self, other: &Self) -> Self {
        for i in 0..N {
            self[i] += other[i].abs();
        }
        self
    }

    fn mul(mut self, other: f32) -> Self {
        for item in self.iter_mut() {
            *item *= other;
        }
        self
    }

    fn div(mut self, other: f32) -> Self {
        for item in self.iter_mut() {
            *item /= other;
        }
        self
    }

    fn div_component(mut self, other: &Self) -> Self {
        for i in 0..N {
            self[i] /= other[i];
        }
        self
    }

    fn min_component(mut self, other: &Self) -> Self {
        for i in 0..N {
            self[i] = self[i].min(other[i]);
        }
        self
    }

    fn max_component(mut self, other: &Self) -> Self {
        for i in 0..N {
            self[i] = self[i].max(other[i]);
        }
        self
    }

    fn normalize(self) -> Self {
        let length = self.length();
        self.div(length)
    }

    fn length(&self) -> f32 {
        self.dot(self).sqrt()
    }

    fn dot(&self, other: &Self) -> f32 {
        let mut result = 0.0;
        for i in 0..N {
            result += self[i] * other[i];
        }
        result
    }

    fn distance_square(&self, other: &Self) -> f32 {
        let mut result = 0.0;
        for i in 0..N {
            let diff = self[i] - other[i];
            result += diff * diff;
        }
        result
    }

    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let mut result = [0.0; N];
        for i in 0..N {
            result[i] = a[i] + (b[i] - a[i]) * t;
        }
        result
    }
}
