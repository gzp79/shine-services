use crate::math::JackknifePoint;
use bevy::math::Vec3;

impl JackknifePoint for Vec3 {
    fn dimension(&self) -> usize {
        3
    }

    fn zero(dimension: usize) -> Self {
        debug_assert_eq!(dimension, 3);
        Vec3::ZERO
    }
}
