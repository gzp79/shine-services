use crate::math::value::{ExpSmoothed, Interpolate, TemporalValue};

pub struct PredictedValue<P>
where
    P: TemporalValue,
    P::Value: Interpolate,
{
    inner: P,
    smoothed: ExpSmoothed<P::Value>,
}

impl<P> PredictedValue<P>
where
    P: TemporalValue,
    P::Value: Interpolate,
{
    pub fn new(inner: P, duration_s: f32) -> Self {
        let start = inner.get().clone();
        Self {
            inner,
            smoothed: ExpSmoothed::new(duration_s, Some(start)),
        }
    }
}

impl<P> TemporalValue for PredictedValue<P>
where
    P: TemporalValue,
    P::Value: Interpolate,
{
    type Value = P::Value;

    fn name(&self) -> Option<&str> {
        self.inner.name()
    }

    fn set(&mut self, value: Self::Value) {
        self.inner.set(value);
    }

    fn get(&self) -> &Self::Value {
        self.inner.get()
    }

    fn update(&mut self, delta_time_s: f32) -> Self::Value {
        self.smoothed.predict_from(self.inner.get(), delta_time_s)
    }
}
