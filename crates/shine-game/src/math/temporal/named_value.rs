use crate::math::temporal::TemporalValue;
use std::borrow::Cow;

pub struct NamedValue<P>
where
    P: TemporalValue,
{
    name: Cow<'static, str>,
    inner: P,
}

impl<P> NamedValue<P>
where
    P: TemporalValue,
{
    pub fn new(name: impl Into<Cow<'static, str>>, inner: P) -> Self {
        Self { name: name.into(), inner }
    }
}

impl<P> TemporalValue for NamedValue<P>
where
    P: TemporalValue,
{
    type Value = P::Value;

    fn name(&self) -> Option<&str> {
        Some(self.name.as_ref())
    }

    fn set(&mut self, value: P::Value) {
        self.inner.set(value);
    }

    fn get(&self) -> &P::Value {
        self.inner.get()
    }

    fn update(&mut self, delta_time_s: f32) -> P::Value {
        self.inner.update(delta_time_s)
    }
}
