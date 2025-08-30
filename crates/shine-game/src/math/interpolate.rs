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
    current: Option<T>,
}

impl<T> ExpSmoothed<T>
where
    T: Interpolate,
{
    /// Create a new exponential smoother that will reach the (99.9% percent of) target value in the given duration.
    /// If `current` is `None`, it will start from the first target value.
    pub fn new(duration_s: f32, current: Option<T>) -> Self {
        const CONVERGENCE: f32 = 0.001;
        let smoothness = -duration_s / CONVERGENCE.ln();
        Self { smoothness, current }
    }

    pub fn smooth_towards(&mut self, target: &T, delta_time_s: f32) -> T {
        // Calculate the exponential blending based on frame time
        let t = (-delta_time_s / self.smoothness.max(1e-5)).exp();

        let prev = self.current.take().unwrap_or_else(|| target.clone());
        let smoothed = Interpolate::interpolate(target.clone(), prev, t);
        self.current = Some(smoothed.clone());

        smoothed
    }

    pub fn predict_from(&mut self, target: &T, delta_time: f32) -> T {
        let smooth = self.smooth_towards(target, delta_time);
        Interpolate::interpolate(target.clone(), smooth, -1.0)
    }
}
