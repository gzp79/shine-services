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
}
