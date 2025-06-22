use crate::input_manager::{DualAxisLike, DualAxisProcessor, ProcessedDualAxis};
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

    pub fn contains(&self, input_value: Vec2) -> bool {
        input_value.length() <= self.radius
    }

    pub fn clamp(&self, input_value: Vec2) -> Vec2 {
        input_value.clamp_length_max(self.radius)
    }
}

impl DualAxisProcessor for CircleBounds {
    fn process(&self, input_value: Vec2) -> Vec2 {
        self.clamp(input_value)
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

    pub fn contains(&self, input_value: Vec2) -> bool {
        input_value.length() <= self.radius
    }

    pub fn clamp(&self, input_value: Vec2) -> Vec2 {
        if self.contains(input_value) {
            Vec2::ZERO
        } else {
            input_value
        }
    }
}

impl DualAxisProcessor for CircleDeadZone {
    fn process(&self, input_value: Vec2) -> Vec2 {
        self.clamp(input_value)
    }
}

pub trait CircleBoundsProcessor: DualAxisLike {
    fn with_circle_bounds(self, radius: f32) -> ProcessedDualAxis<Self, CircleBounds>
    where
        Self: Sized;

    fn with_circle_dead_zone(self, radius: f32) -> ProcessedDualAxis<Self, CircleDeadZone>
    where
        Self: Sized;
}

impl<T: DualAxisLike> CircleBoundsProcessor for T {
    fn with_circle_bounds(self, radius: f32) -> ProcessedDualAxis<Self, CircleBounds>
    where
        Self: Sized,
    {
        ProcessedDualAxis::new(self, CircleBounds::new(radius))
    }

    fn with_circle_dead_zone(self, radius: f32) -> ProcessedDualAxis<Self, CircleDeadZone>
    where
        Self: Sized,
    {
        ProcessedDualAxis::new(self, CircleDeadZone::new(radius))
    }
}
