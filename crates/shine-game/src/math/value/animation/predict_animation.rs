use crate::math::value::{Animation, ExpSmoothed, Interpolate, TweenLike};

/// Perform an extrapolation using the value from an exponential decay smoothing on the output of another animation.
pub struct PredictAnimation<T, A>
where
    T: Interpolate + TweenLike,
    A: Animation<Out = T>,
{
    prev: A,
    smoothed: ExpSmoothed<T>,
}

impl<T, A> PredictAnimation<T, A>
where
    T: Interpolate + TweenLike,
    A: Animation<Out = T>,
{
    pub fn new(prev: A, duration_s: f32) -> Self {
        PredictAnimation {
            prev,
            smoothed: ExpSmoothed::new(duration_s, None),
        }
    }

    pub fn new_with_init(prev: A, initial: T, duration_s: f32) -> (Self, T) {
        let mut new = Self::new(prev, duration_s);
        let init = new.smoothed.predict_from(&initial, 0.0);
        (new, init)
    }
}

impl<T, A> Animation for PredictAnimation<T, A>
where
    T: Interpolate + TweenLike,
    A: Animation<Out = T>,
{
    type In = A::In;
    type Out = T;

    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out {
        let v = self.prev.animate(current, delta_time_s);
        self.smoothed.predict_from(&v, delta_time_s)
    }
}
