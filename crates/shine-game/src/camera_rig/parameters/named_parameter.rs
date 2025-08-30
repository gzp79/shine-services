use crate::camera_rig::RigParameter;
use std::borrow::Cow;

pub struct NamedParameter<P>
where
    P: RigParameter,
{
    name: Cow<'static, str>,
    inner: P,
}

impl<P> NamedParameter<P>
where
    P: RigParameter,
{
    pub fn new(name: impl Into<Cow<'static, str>>, inner: P) -> Self {
        Self { name: name.into(), inner }
    }
}

impl<P> RigParameter for NamedParameter<P>
where
    P: RigParameter,
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
