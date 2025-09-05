use std::borrow::Cow;

use crate::math::value::{
    Animate, AnimatedVariable, Interpolate, NullAnimate, Predicting, Smoothing, ValueError, ValueLike, ValueType,
    Variable,
};

pub struct AnimationChain<T, U = T, A = NullAnimate<T>>
where
    T: ValueLike,
    U: ValueLike,
    A: Animate<In = T, Out = U>,
{
    name: Option<Cow<'static, str>>,
    target: T,
    current: Option<U>,
    animation: A,
}

impl<T> Default for AnimationChain<T>
where
    T: ValueLike + Default,
{
    fn default() -> Self {
        AnimationChain::new(T::default())
    }
}

impl<T> AnimationChain<T, T, NullAnimate<T>>
where
    T: ValueLike,
    T: ValueLike,
{
    fn new(target: T) -> Self {
        Self {
            name: None,
            target,
            current: None,
            animation: NullAnimate::default(),
        }
    }
}

impl<T, U, A> AnimationChain<T, U, A>
where
    T: ValueLike,
    U: ValueLike,
    A: Animate<In = T, Out = U>,
{
    pub fn with_name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    pub fn set_target(&mut self, target: T) {
        self.target = target;
    }

    pub fn current(&self) -> Option<&U> {
        self.current.as_ref()
    }

    pub fn animate(&mut self, delta_time_s: f32) -> U {
        let target = self.target.clone();
        let current = self.animation.animate(target, delta_time_s);
        self.current = Some(current.clone());
        current
    }
}

impl<T, U, A> Variable for AnimationChain<T, U, A>
where
    T: ValueLike,
    U: ValueLike,
    A: Animate<In = T, Out = U>,
{
    fn name(&self) -> Option<&str> {
        None
    }

    fn get(&self) -> ValueType {
        self.target.clone().into()
    }

    fn update(&mut self, value: ValueType) -> Result<(), ValueError> {
        self.target = value.try_into()?;
        Ok(())
    }

    fn update_with(&mut self, update: &dyn Fn(ValueType) -> Result<ValueType, ValueError>) -> Result<(), ValueError> {
        let new_value = update(self.target.clone().into())?;
        self.target = new_value.try_into()?;
        Ok(())
    }
}

impl<T, U, A> AnimatedVariable for AnimationChain<T, U, A>
where
    T: ValueLike,
    U: ValueLike,
    A: Animate<In = T, Out = U>,
{
    type Value = U;

    fn animate(&mut self, delta_time_s: f32) -> Self::Value {
        AnimationChain::animate(self, delta_time_s)
    }
}

impl<T, U, A> AnimationChain<T, U, A>
where
    T: ValueLike,
    U: ValueLike,
    A: Animate<In = T, Out = U>,
{
    pub fn smooth(self, duration_s: f32) -> AnimationChain<T, U, impl Animate<In = T, Out = U>>
    where
        U: Interpolate,
    {
        AnimationChain {
            name: self.name,
            target: self.target,
            current: self.current,
            animation: Smoothing::new(self.animation, duration_s),
        }
    }

    pub fn predict(self, duration_s: f32) -> AnimationChain<T, U, impl Animate<In = T, Out = U>>
    where
        U: Interpolate,
    {
        AnimationChain {
            name: self.name,
            target: self.target,
            current: self.current,
            animation: Predicting::new(self.animation, duration_s),
        }
    }

    /// Type erase the animation by boxing it
    pub fn boxed(self) -> AnimationChain<T, U, Box<dyn Animate<In = T, Out = U>>>
    where
        U: Interpolate,
    {
        AnimationChain {
            name: self.name,
            target: self.target,
            current: self.current,
            animation: Box::new(self.animation),
        }
    }
}
