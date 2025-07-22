use crate::input_manager::{
    AxisLike, AxisProcessor, DualAxisLike, DualAxisProcessor, ProcessedAxis, ProcessedDualAxis,
};
use bevy::math::Vec2;

/// Clamps the input value to a maximum distance of `radius` from the origin.
/// - For single-axis inputs (`f32`), the value is clamped to the range `[-radius, radius]`.
/// - For dual-axis inputs (`Vec2`), the vector's length is limited to `radius`, preserving its direction.
pub struct RadialClamp {
    radius: f32,
}

impl RadialClamp {
    pub fn new(radius: f32) -> Self {
        assert!(radius >= 0.0);
        Self { radius }
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn contains(&self, input_value: f32) -> bool {
        input_value.abs() <= self.radius
    }

    pub fn clamp(&self, input_value: f32) -> f32 {
        input_value.clamp(-self.radius, self.radius)
    }

    pub fn contains_vec2(&self, input_value: Vec2) -> bool {
        input_value.length() <= self.radius
    }

    pub fn clamp_vec2(&self, input_value: Vec2) -> Vec2 {
        input_value.clamp_length_max(self.radius)
    }
}

impl AxisProcessor for RadialClamp {
    fn type_name(&self) -> &'static str {
        "RadialClamp"
    }

    fn process(&mut self, input_value: Option<f32>) -> Option<f32> {
        input_value.map(|v| self.clamp(v))
    }
}

impl DualAxisProcessor for RadialClamp {
    fn type_name(&self) -> &'static str {
        "RadialClamp"
    }

    fn process(&mut self, input_value: Option<Vec2>) -> Option<Vec2> {
        input_value.map(|v| self.clamp_vec2(v))
    }
}

/// Converts the input value to a dead zone, where values within the specified radius are set to 0.
/// - For single-axis inputs (`f32`), the value is set to 0 if it is within the radius.
/// - For dual-axis inputs (`Vec2`), the vector is set to 0 if its length is within the radius.
pub struct RadialDeadZone {
    radius: f32,
}

impl RadialDeadZone {
    pub fn new(radius: f32) -> Self {
        assert!(radius >= 0.0);
        Self { radius }
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn contains(&self, input_value: f32) -> bool {
        input_value.abs() <= self.radius
    }

    pub fn clamp(&self, input_value: f32) -> f32 {
        if self.contains(input_value) {
            0.0
        } else {
            input_value
        }
    }

    pub fn contains_vec2(&self, input_value: Vec2) -> bool {
        input_value.length() <= self.radius
    }

    pub fn clamp_vec2(&self, input_value: Vec2) -> Vec2 {
        if self.contains_vec2(input_value) {
            Vec2::ZERO
        } else {
            input_value
        }
    }
}

impl AxisProcessor for RadialDeadZone {
    fn type_name(&self) -> &'static str {
        "RadialDeadZone"
    }

    fn process(&mut self, input_value: Option<f32>) -> Option<f32> {
        input_value.map(|v| self.clamp(v))
    }
}

impl DualAxisProcessor for RadialDeadZone {
    fn type_name(&self) -> &'static str {
        "RadialDeadZone"
    }

    fn process(&mut self, input_value: Option<Vec2>) -> Option<Vec2> {
        input_value.map(|v| self.clamp_vec2(v))
    }
}

/// Helper to add radial clamp processing to an [`AxisLike`] input.
pub trait AxisRadialProcessor: AxisLike {
    fn with_bounds(self, radius: f32) -> ProcessedAxis<Self, RadialClamp>
    where
        Self: Sized;

    fn with_dead_zone(self, radius: f32) -> ProcessedAxis<Self, RadialDeadZone>
    where
        Self: Sized;
}

impl<T: AxisLike> AxisRadialProcessor for T {
    fn with_bounds(self, radius: f32) -> ProcessedAxis<Self, RadialClamp>
    where
        Self: Sized,
    {
        ProcessedAxis::new(self, RadialClamp::new(radius))
    }

    fn with_dead_zone(self, radius: f32) -> ProcessedAxis<Self, RadialDeadZone>
    where
        Self: Sized,
    {
        ProcessedAxis::new(self, RadialDeadZone::new(radius))
    }
}

/// Helper to add radial bounds processing to an [`DualAxisLike`] input.
pub trait DualAxisRadialProcessor: DualAxisLike {
    fn with_bounds(self, radius: f32) -> ProcessedDualAxis<Self, RadialClamp>
    where
        Self: Sized;

    fn with_dead_zone(self, radius: f32) -> ProcessedDualAxis<Self, RadialDeadZone>
    where
        Self: Sized;
}

impl<T: DualAxisLike> DualAxisRadialProcessor for T {
    fn with_bounds(self, radius: f32) -> ProcessedDualAxis<Self, RadialClamp>
    where
        Self: Sized,
    {
        ProcessedDualAxis::new(self, RadialClamp::new(radius))
    }

    fn with_dead_zone(self, radius: f32) -> ProcessedDualAxis<Self, RadialDeadZone>
    where
        Self: Sized,
    {
        ProcessedDualAxis::new(self, RadialDeadZone::new(radius))
    }
}
