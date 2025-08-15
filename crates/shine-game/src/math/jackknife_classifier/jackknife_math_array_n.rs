use crate::math::JackknifePoint;

impl<const N: usize> JackknifePoint for [f32; N] {
    fn dimension(&self) -> usize {
        N
    }

    fn zero(dimension: usize) -> Self {
        debug_assert_eq!(dimension, N);
        [0.0; N]
    }
}
