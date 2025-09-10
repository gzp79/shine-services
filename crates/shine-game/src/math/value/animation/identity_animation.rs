use crate::math::value::{Animation, TweenLike};
use std::marker::PhantomData;

/// A identity animation that return the input value as output.
pub struct IdentityAnimate<T>(PhantomData<T>)
where
    T: TweenLike;

impl<T> Default for IdentityAnimate<T>
where
    T: TweenLike,
{
    fn default() -> Self {
        IdentityAnimate(PhantomData)
    }
}

impl<T> Animation for IdentityAnimate<T>
where
    T: TweenLike,
{
    type In = T;
    type Out = T;

    #[inline(always)]
    fn animate(&mut self, current: Self::In, _delta_time_s: f32) -> Self::Out {
        current
    }
}
