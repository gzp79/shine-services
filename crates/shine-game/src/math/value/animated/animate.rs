use crate::math::value::ValueLike;
use std::marker::PhantomData;

// A custom animation trait
pub trait Animate: Send + Sync + 'static {
    type In: ValueLike;
    type Out: ValueLike;

    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out;
}

impl<T, U> Animate for Box<dyn Animate<In = T, Out = U>>
where
    T: ValueLike,
    U: ValueLike,
{
    type In = T;
    type Out = U;

    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out {
        self.as_mut().animate(current, delta_time_s)
    }
}

/// A no-op animation implementation that immediately sets the value to the target without any interpolation or transition.
/// Useful as a default or placeholder animation strategy.
pub struct NullAnimate<T>(PhantomData<T>)
where
    T: ValueLike;

impl<T> Default for NullAnimate<T>
where
    T: ValueLike,
{
    fn default() -> Self {
        NullAnimate(PhantomData)
    }
}

impl<T> Animate for NullAnimate<T>
where
    T: ValueLike,
{
    type In = T;
    type Out = T;

    #[inline(always)]
    fn animate(&mut self, current: Self::In, _delta_time_s: f32) -> Self::Out {
        current
    }
}
