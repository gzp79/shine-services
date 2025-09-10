use crate::math::value::{Animation, TweenLike};
use bevy::math::Curve;
use std::marker::PhantomData;

/// An animation that uses a `bevy::math::Curve` to animate values.
pub struct CurveAnimation<T, C, A>
where
    T: TweenLike,
    C: Curve<T> + Send + Sync + 'static,
    A: Animation<Out = f32>,
{
    prev: A,
    curve: C,
    _phantom: PhantomData<T>,
}

impl<T, C, A> CurveAnimation<T, C, A>
where
    T: TweenLike,
    C: Curve<T> + Send + Sync + 'static,
    A: Animation<Out = f32>,
{
    pub fn new(prev: A, curve: C) -> Self {
        CurveAnimation {
            prev,
            curve,
            _phantom: PhantomData,
        }
    }

    pub fn new_with_init(prev: A, initial: f32, curve: C) -> (Self, T) {
        let new = Self::new(prev, curve);
        let init = new.sample_wrapped(initial);
        (new, init)
    }

    fn sample_wrapped(&self, t: f32) -> T {
        let (start, end) = (self.curve.domain().start(), self.curve.domain().end());

        let t = if self.curve.domain().has_finite_end() && t > end {
            if self.curve.domain().has_finite_start() {
                let range = end - start;
                start + (t - start) % range
            } else {
                // If no finite start, just clamp to end
                end
            }
        } else {
            t
        };

        self.curve.sample(t).unwrap()
    }
}

impl<T, C, A> Animation for CurveAnimation<T, C, A>
where
    T: TweenLike,
    C: Curve<T> + Send + Sync + 'static,
    A: Animation<Out = f32>,
{
    type In = A::In;
    type Out = T;

    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out {
        let t = self.prev.animate(current, delta_time_s);
        self.sample_wrapped(t)
    }
}
