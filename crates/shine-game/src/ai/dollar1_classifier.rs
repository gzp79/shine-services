/// Some sources:
/// - <https://github.com/uwdata/gestrec>
/// - <https://faculty.washington.edu/wobbrock/pubs/gi-10.02.pdf>
/// - <https://faculty.washington.edu/wobbrock/pubs/mobilehci-18.pdf>
/// - <https://github.com/angrychill/q-dollar-gesture-godot/blob/main/q-dollar-try/q_recognizer/q_point_cloud_recognizer.gd>
/// - <https://github.com/ISUE/Jackknife/tree/master>
use bevy::math::Vec2;
use std::f32::consts;

pub struct Dollar1ClassifierConfig {
    pub resample_size: usize,
    pub normalize_size: f32,
}

impl Default for Dollar1ClassifierConfig {
    fn default() -> Self {
        Self {
            resample_size: 64,
            normalize_size: 100.0,
        }
    }
}

pub struct Dollar1Classifier {
    config: Dollar1ClassifierConfig,
    resampled_points: Vec<Vec2>,
    feature_vector: Vec<f32>,
}

impl Dollar1Classifier {
    fn path_length(points: &[Vec2]) -> f32 {
        assert!(points.len() > 1);

        points
            .iter()
            .zip(points.iter().skip(1))
            .map(|(a, b)| (a - b).length())
            .sum::<f32>()
    }

    fn centroid(points: &[Vec2]) -> Vec2 {
        assert!(points.len() > 0);

        let mut centroid = Vec2::ZERO;

        for point in points {
            centroid += *point;
        }
        centroid /= points.len() as f32;

        centroid
    }

    fn translate(points: &mut [Vec2], delta: Vec2) {
        for point in points.iter_mut() {
            *point += delta;
        }
    }

    fn rotate(points: &mut [Vec2], angle: f32) {
        let (sin, cos) = angle.sin_cos();
        for point in points.iter_mut() {
            let x = point.x * cos - point.y * sin;
            let y = point.x * sin + point.y * cos;
            point.x = x;
            point.y = y;
        }
    }

    fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        assert!(a.len() == b.len());

        let mut sum_a = 0.0;
        let mut sum_b = 0.0;
        for i in (0..a.len()).step_by(2) {
            sum_a += a[i] * b[i] + a[i + 1] * b[i + 1];
            sum_b += a[i] * b[i + 1] - a[i + 1] * b[i];
        }

        let angle = sum_b.atan2(sum_a).atan();
        log::info!("angle: {}", angle.to_degrees());

        let (sin, cos) = angle.sin_cos();
        let distance = sum_a * cos + sum_b * sin;
        distance.acos() / consts::PI
    }

    /// Standard cosine distance that is more sensitive to orientation differences
    fn standard_cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        assert!(a.len() == b.len());

        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in (0..a.len()).step_by(2) {
            let ax = a[i];
            let ay = a[i + 1];
            let bx = b[i];
            let by = b[i + 1];

            dot_product += ax * bx + ay * by;
            norm_a += ax * ax + ay * ay;
            norm_b += bx * bx + by * by;
        }

        let cosine_similarity = dot_product / (norm_a.sqrt() * norm_b.sqrt());
        // Return distance (1 - similarity) so lower values = more similar
        1.0 - cosine_similarity
    }
}

impl Dollar1Classifier {
    pub fn new(config: Dollar1ClassifierConfig) -> Self {
        Self {
            config,
            resampled_points: Vec::new(),
            feature_vector: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.resampled_points.clear();
        self.feature_vector.clear();
    }

    pub fn set_points(&mut self, points: &[Vec2]) {
        self.clear();

        if points.len() < 2 {
            return;
        }

        // Normalize the points into a unit square
        self.resample(points);
        self.normalize();

        // extract features: <http://yangl.org/protractor/>
        self.vectorize();
    }

    pub fn classify(&self, gestures: &[Vec<f32>]) -> Option<(usize, f32)> {
        if self.feature_vector.len() == 0 {
            log::error!("Classifier is not initialized");
            return None;
        }

        if gestures.iter().any(|g| g.len() != self.feature_vector.len()) {
            log::error!("Gesture feature-vector is not compatible with the classifier");
            return None;
        }

        let candidate = gestures
            .iter()
            .enumerate()
            .map(|(id, gesture)| (id, Self::standard_cosine_distance(&self.feature_vector, &gesture)))
            .inspect(|(id, distance)| log::info!("{}: {}", id, distance))
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());

        candidate
    }

    pub fn feature_vector(&self) -> &[f32] {
        &self.feature_vector
    }

    pub fn resampled_points(&self) -> &[Vec2] {
        &self.resampled_points
    }

    fn resample(&mut self, input_points: &[Vec2]) {
        assert!(input_points.len() > 1);

        // sampling count might not be exact, thus add some extra space to avoid reallocations
        self.resampled_points.clear();
        self.resampled_points.reserve(self.config.resample_size);

        let path_length = Self::path_length(input_points);
        let increment = path_length / (self.config.resample_size - 1) as f32;
        let mut accumulated_length = 0.0;
        let mut previous_point = input_points[0];
        self.resampled_points.push(previous_point);

        // Resample points: if the distance between points is too large, interpolate additional points (up-sample);
        // if points are too close together, skip them to maintain even spacing (down-sample).
        for &current_point in input_points.iter().skip(1) {
            let mut current_length = previous_point.distance(current_point);

            while accumulated_length + current_length >= increment
                && self.resampled_points.len() < self.config.resample_size
            {
                let t = (increment - accumulated_length) / current_length;
                let new_point = previous_point.lerp(current_point, t);

                self.resampled_points.push(new_point);

                previous_point = new_point;
                accumulated_length = 0.0;
                current_length = previous_point.distance(current_point);
            }

            accumulated_length += current_length;
            previous_point = current_point;
        }

        // clean up the last point
        if self.resampled_points.len() < self.config.resample_size {
            self.resampled_points.push(previous_point);
        } else {
            self.resampled_points[self.config.resample_size - 1] = previous_point;
        }
        assert!(self.resampled_points.len() == self.config.resample_size);
    }

    fn normalize(&mut self) {
        let centroid = Self::centroid(&self.resampled_points);

        // quantize the angle to the nearest 45 degrees
        let angle = {
            const ORIENTATIONS: &[f32] = &[
                0.0,
                (consts::PI / 4.0),
                (consts::PI / 2.0),
                (consts::PI * 3.0 / 4.0),
                consts::PI,
                -0.0,
                (-consts::PI / 4.0),
                (-consts::PI / 2.0),
                (-consts::PI * 3.0 / 4.0),
                -consts::PI,
            ];

            let vector = self.resampled_points[0] - centroid;
            let angle = vector.y.atan2(vector.x);

            let mut adjustment = -angle;
            for orientation in ORIENTATIONS {
                let delta = orientation - angle;
                if delta.abs() < adjustment.abs() {
                    adjustment = delta;
                }
            }

            adjustment
        };

        Self::translate(&mut self.resampled_points, -centroid);
        Self::rotate(&mut self.resampled_points, angle);

        // no need for scaling, as the feature vector is normalized
    }

    fn vectorize(&mut self) {
        self.feature_vector.reserve(self.resampled_points.len() * 2);

        let mut sum = 0.0;
        for point in self.resampled_points.iter() {
            self.feature_vector.push(point.x);
            self.feature_vector.push(point.y);
            sum += point.x * point.x + point.y * point.y;
        }

        let magnitude = sum.sqrt();
        for feature in self.feature_vector.iter_mut() {
            *feature /= magnitude;
        }
    }
}
