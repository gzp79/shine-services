pub struct RunningMoments {
    count: usize,
    min: f32,
    max: f32,
    mean: f32,
    m2: f32,
}

impl Default for RunningMoments {
    fn default() -> Self {
        RunningMoments::new()
    }
}

impl RunningMoments {
    pub fn new() -> Self {
        RunningMoments {
            count: 0,
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            mean: 0.0,
            m2: 0.0,
        }
    }

    pub fn add(&mut self, value: f32) {
        self.min = self.min.min(value);
        self.max = self.max.max(value);

        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f32;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    pub fn add_slice(&mut self, values: &[f32]) {
        for &value in values {
            self.add(value);
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn min(&self) -> f32 {
        self.min
    }

    pub fn max(&self) -> f32 {
        self.max
    }

    pub fn mean(&self) -> f32 {
        self.mean
    }

    pub fn variance(&self) -> f32 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f32
        }
    }

    pub fn std_dev(&self) -> f32 {
        self.variance().sqrt()
    }

    pub fn lower_bound(&self) -> f32 {
        self.mean - self.std_dev()
    }

    pub fn upper_bound(&self) -> f32 {
        self.mean + self.std_dev()
    }
}
