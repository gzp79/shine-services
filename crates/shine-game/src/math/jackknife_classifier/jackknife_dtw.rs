use crate::math::JackknifePointMath;
use std::ops;

pub struct CostMatrix {
    size: (usize, usize),
    data: Vec<f32>,
}

impl Default for CostMatrix {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn dtw<F, V>(&mut self, v1: &[V], v2: &[V], radius: usize, distance: F) -> f32
    where
        V: JackknifePointMath<V>,
        F: Fn(&V, &V) -> f32,
    {
        let n = v1.len() + 1;
        let m = v2.len() + 1;

        self.init(n, m);

        for i in 1..n {
            let range_min = if i > radius { i - radius } else { 1 };
            let range_max = (i + radius).min(m - 1);
            for j in range_min..=range_max {
                let minimum: f32 = (self[(i - 1, j)]) // repeat v1 element
                    .min(self[(i, j - 1)]) // repeat v2 element
                    .min(self[(i - 1, j - 1)]); // new element
                self[(i, j)] = minimum;
                self[(i, j)] += distance(&v1[i - 1], &v2[j - 1]);
            }
        }

        self[(n - 1, m - 1)]
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
