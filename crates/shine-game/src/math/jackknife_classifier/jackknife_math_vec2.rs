use crate::math::JackknifePoint;
use bevy::math::Vec2;

impl JackknifePoint for Vec2 {
    fn dimension(&self) -> usize {
        2
    }

    fn zero(dimension: usize) -> Self {
        debug_assert_eq!(dimension, 2);
        Vec2::ZERO
    }

    fn splat(dimension: usize, value: f32) -> Self {
        debug_assert_eq!(dimension, 2);
        Vec2::splat(value)
    }

    fn from_sub(a: &Self, b: &Self) -> Self {
        a - b
    }

    fn from_add(a: &Self, b: &Self) -> Self {
        a + b
    }

    fn sub(self, other: &Self) -> Self {
        self - other
    }

    fn add(self, other: &Self) -> Self {
        self + other
    }

    fn add_abs(self, other: &Self) -> Self {
        self + other.abs()
    }

    fn mul(self, other: f32) -> Self {
        self * other
    }

    fn div(self, other: f32) -> Self {
        self / other
    }

    fn div_component(self, other: &Self) -> Self {
        self / other
    }

    fn min_component(self, other: &Self) -> Self {
        Vec2::new(self.x.min(other.x), self.y.min(other.y))
    }

    fn max_component(self, other: &Self) -> Self {
        Vec2::new(self.x.max(other.x), self.y.max(other.y))
    }

    fn normalize(self) -> Self {
        self.normalize()
    }

    fn length(&self) -> f32 {
        Vec2::length(*self)
    }

    fn dot(&self, other: &Self) -> f32 {
        Vec2::dot(*self, *other)
    }

    fn distance_square(&self, other: &Self) -> f32 {
        Vec2::distance_squared(*self, *other)
    }

    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Vec2::lerp(*a, *b, t)
    }
}
