use crate::input_manager::{
    AxisLike, AxisProcessor, DualAxisLike, DualAxisProcessor, ProcessedAxis, ProcessedDualAxis,
};
use bevy::math::Vec2;

pub struct CircleBounds {
    radius: f32,
}

impl CircleBounds {
    pub fn new(max: f32) -> Self {
        assert!(max >= 0.0);
        Self { radius: max }
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

    pub fn contains2(&self, input_value: Vec2) -> bool {
        input_value.length() <= self.radius
    }

    pub fn clamp2(&self, input_value: Vec2) -> Vec2 {
        input_value.clamp_length_max(self.radius)
    }
}

impl AxisProcessor for CircleBounds {
    fn process(&mut self, input_value: Option<f32>) -> Option<f32> {
        input_value.map(|v| self.clamp(v))
    }
}

impl DualAxisProcessor for CircleBounds {
    fn process(&mut self, input_value: Option<Vec2>) -> Option<Vec2> {
        input_value.map(|v| self.clamp2(v))
    }
}

pub struct CircleDeadZone {
    radius: f32,
}

impl CircleDeadZone {
    pub fn new(max: f32) -> Self {
        assert!(max >= 0.0);
        Self { radius: max }
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

    pub fn contains2(&self, input_value: Vec2) -> bool {
        input_value.length() <= self.radius
    }

    pub fn clamp2(&self, input_value: Vec2) -> Vec2 {
        if self.contains2(input_value) {
            Vec2::ZERO
        } else {
            input_value
        }
    }
}

impl AxisProcessor for CircleDeadZone {
    fn process(&mut self, input_value: Option<f32>) -> Option<f32> {
        input_value.map(|v| self.clamp(v))
    }
}

impl DualAxisProcessor for CircleDeadZone {
    fn process(&mut self, input_value: Option<Vec2>) -> Option<Vec2> {
        input_value.map(|v| self.clamp2(v))
    }
}

/// Helper to add circle bounds processing to an [`AxisLike`] input.
pub trait AxisCircleBoundsProcessor: AxisLike {
    fn with_bounds(self, radius: f32) -> ProcessedAxis<Self, CircleBounds>
    where
        Self: Sized;

    fn with_dead_zone(self, radius: f32) -> ProcessedAxis<Self, CircleDeadZone>
    where
        Self: Sized;
}

impl<T: AxisLike> AxisCircleBoundsProcessor for T {
    fn with_bounds(self, radius: f32) -> ProcessedAxis<Self, CircleBounds>
    where
        Self: Sized,
    {
        ProcessedAxis::new(self, CircleBounds::new(radius))
    }

    fn with_dead_zone(self, radius: f32) -> ProcessedAxis<Self, CircleDeadZone>
    where
        Self: Sized,
    {
        ProcessedAxis::new(self, CircleDeadZone::new(radius))
    }
}

/// Helper to add circle bounds processing to an [`DualAxisLike`] input.
pub trait DualAxisCircleBoundsProcessor: DualAxisLike {
    fn with_bounds(self, radius: f32) -> ProcessedDualAxis<Self, CircleBounds>
    where
        Self: Sized;

    fn with_dead_zone(self, radius: f32) -> ProcessedDualAxis<Self, CircleDeadZone>
    where
        Self: Sized;
}

impl<T: DualAxisLike> DualAxisCircleBoundsProcessor for T {
    fn with_bounds(self, radius: f32) -> ProcessedDualAxis<Self, CircleBounds>
    where
        Self: Sized,
    {
        ProcessedDualAxis::new(self, CircleBounds::new(radius))
    }

    fn with_dead_zone(self, radius: f32) -> ProcessedDualAxis<Self, CircleDeadZone>
    where
        Self: Sized,
    {
        ProcessedDualAxis::new(self, CircleDeadZone::new(radius))
    }
}
