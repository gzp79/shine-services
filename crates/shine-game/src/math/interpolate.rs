use bevy::math::{Quat, Vec2, Vec3};

/// (Linear) interpolation trait for types that can be smoothly interpolated.
pub trait Interpolate: Clone {
    /// return interpolated value between `self` and `other` at time `t`.
    /// For t = 0.0, returns `self`, for t = 1.0, returns `other`.
    fn interpolate(self, other: Self, t: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(self, other: Self, t: f32) -> Self {
        self * (1.0 - t) + other * t
    }
}

impl Interpolate for Vec2 {
    fn interpolate(self, other: Self, t: f32) -> Self {
        Vec2::lerp(self, other, t)
    }
}

impl Interpolate for Vec3 {
    fn interpolate(self, other: Self, t: f32) -> Self {
        Vec3::lerp(self, other, t)
    }
}

impl Interpolate for Quat {
    fn interpolate(self, other: Self, t: f32) -> Self {
        // Technically should be a `slerp` for framerate independence, but the latter
        // will rotate in the negative direction when interpolating a 180..360 degree rotation
        // to the 0..180 range.
        Quat::lerp(self.normalize(), other.normalize(), t).normalize()
    }
}

/// Frame-rate independent exponential decay function
/// See: https://www.rorydriscoll.com/2016/03/07/frame-rate-independent-damping-using-lerp/
#[derive(Debug)]
pub struct ExpSmoothed<T>
where
    T: Interpolate,
{
    smoothness: f32,
    prev: Option<T>,
}

impl<T> Default for ExpSmoothed<T>
where
    T: Interpolate,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ExpSmoothed<T>
where
    T: Interpolate,
{
    pub fn new() -> Self {
        Self { smoothness: 1.0, prev: None }
    }

    pub fn with_start(prev: T) -> Self {
        Self {
            smoothness: 1.0,
            prev: Some(prev),
        }
    }

    /*pub fn smoothness(mut self, smoothness: f32) -> Self {
        self.smoothness = smoothness;
        self
    }*/

    /// Set smoothing factor so that the target is reached in the given time with a precision of 99%
    pub fn duration(mut self, duration_s: f32) -> Self {
        const CONVERGENCE: f32 = 0.01;
        self.smoothness = -duration_s / CONVERGENCE.ln();
        self
    }

    pub fn exp_smooth_towards(&mut self, target: &T, delta_time_s: f32) -> T {
        // Calculate the exponential blending based on frame time
        let t = (-delta_time_s / self.smoothness.max(1e-5)).exp();

        let prev = self.prev.clone().unwrap_or(target.clone());
        let smooth = Interpolate::interpolate(target.clone(), prev, t);

        self.prev = Some(smooth.clone());

        smooth
    }

    pub fn exp_predict_from(&mut self, target: &T, delta_time: f32) -> T {
        let smooth = self.exp_smooth_towards(target, delta_time);
        Interpolate::interpolate(target.clone(), smooth, -1.0)
    }
}
