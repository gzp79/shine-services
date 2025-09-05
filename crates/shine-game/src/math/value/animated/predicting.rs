use crate::math::value::{Animate, ExpSmoothed, Interpolate, ValueLike};

pub struct Predicting<T, A>
where
    T: ValueLike + Interpolate,
    A: Animate<Out = T>,
{
    prev: A,
    smoothed: ExpSmoothed<T>,
}

impl<T, A> Predicting<T, A>
where
    T: ValueLike + Interpolate,
    A: Animate<Out = T>,
{
    pub fn new(prev: A, duration_s: f32) -> Self {
        Predicting {
            prev,
            smoothed: ExpSmoothed::new(duration_s, None),
        }
    }
}

impl<T, A> Animate for Predicting<T, A>
where
    T: ValueLike + Interpolate,
    A: Animate<Out = T>,
{
    type In = A::In;
    type Out = T;

    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out {
        let v = self.prev.animate(current, delta_time_s);
        self.smoothed.predict_from(&v, delta_time_s)
    }
}
