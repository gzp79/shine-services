use crate::ai::JackknifePoint;
use bevy::math::Vec3;

impl JackknifePoint for Vec3 {
    fn dimension(&self) -> usize {
        3
    }

    fn zero(dimension: usize) -> Self {
        debug_assert_eq!(dimension, 3);
        Vec3::ZERO
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
        Vec3::new(self.x.min(other.x), self.y.min(other.y), self.z.min(other.z))
    }

    fn max_component(self, other: &Self) -> Self {
        Vec3::new(self.x.max(other.x), self.y.max(other.y), self.z.max(other.z))
    }

    fn normalize(self) -> Self {
        self.normalize()
    }

    fn length(&self) -> f32 {
        Vec3::length(*self)
    }

    fn dot(&self, other: &Self) -> f32 {
        Vec3::dot(*self, *other)
    }

    fn distance_square(&self, other: &Self) -> f32 {
        Vec3::distance_squared(*self, *other)
    }

    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Vec3::lerp(*a, *b, t)
    }
}
