use crate::math::value::{
    AnimatedVariable, Animation, CurveAnimation, IdentityAnimate, Interpolate, MapAnimation, PredictAnimation,
    SmoothAnimation, TimeAnimation, TweenLike, ValueError, ValueKind, ValueLike, ValueType, Variable,
};
use bevy::math::{Curve, Quat, Vec2, Vec3, Vec4};
use std::borrow::Cow;

pub struct AnimatedValue<T, U = T, A = IdentityAnimate<T>>
where
    T: TweenLike,
    U: TweenLike,
    A: Animation<In = T, Out = U>,
{
    name: Option<Cow<'static, str>>,
    target: T,
    current: U,
    animation: A,
}

impl<T> Default for AnimatedValue<T>
where
    T: TweenLike + Default,
{
    fn default() -> Self {
        AnimatedValue::new(T::default(), T::default())
    }
}

impl<T> AnimatedValue<T, T, IdentityAnimate<T>>
where
    T: TweenLike,
    T: TweenLike,
{
    fn new(current: T, target: T) -> Self {
        Self {
            name: None,
            target,
            current,
            animation: IdentityAnimate::default(),
        }
    }
}

impl AnimatedValue<(), f32, TimeAnimation> {
    pub fn time() -> Self {
        let (animation, current) = TimeAnimation::new_with_init((), 0.0);

        AnimatedValue {
            name: None,
            target: (),
            current,
            animation,
        }
    }
}

impl<T, U, A> AnimatedValue<T, U, A>
where
    T: TweenLike,
    U: TweenLike,
    A: Animation<In = T, Out = U>,
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

    pub fn current(&self) -> &U {
        &self.current
    }

    pub fn animate(&mut self, delta_time_s: f32) -> U {
        let target = self.target.clone();
        let current = self.animation.animate(target, delta_time_s);
        self.current = current.clone();
        current
    }
}

/// Implement Variable for AnimatedVariable by delegating to the target value of the managing animation state.
impl<T, U, A> Variable for AnimatedValue<T, U, A>
where
    T: TweenLike + Variable,
    U: TweenLike,
    A: Animation<In = T, Out = U>,
{
    fn kind(&self) -> ValueKind {
        self.target.kind()
    }

    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn get(&self) -> ValueType {
        self.target.get()
    }

    fn update(&mut self, value: ValueType) -> Result<(), ValueError> {
        self.target.update(value)
    }

    fn update_with(&mut self, update: &dyn Fn(ValueType) -> Result<ValueType, ValueError>) -> Result<(), ValueError> {
        self.target.update_with(update)
    }
}

impl<T, U, A> AnimatedVariable for AnimatedValue<T, U, A>
where
    T: TweenLike + Variable,
    U: TweenLike,
    A: Animation<In = T, Out = U>,
{
    type Value = U;

    fn animate(&mut self, delta_time_s: f32) -> Self::Value {
        self.animate(delta_time_s)
    }
}

/// Builder functions to create animation variables by chaining multiple animations.
impl<T, U, A> AnimatedValue<T, U, A>
where
    T: TweenLike,
    U: TweenLike,
    A: Animation<In = T, Out = U>,
{
    pub fn smooth(self, duration_s: f32) -> AnimatedValue<T, U, impl Animation<In = T, Out = U>>
    where
        U: Interpolate,
    {
        let (animation, current) = SmoothAnimation::new_with_init(self.animation, self.current, duration_s);

        AnimatedValue {
            name: self.name,
            target: self.target,
            current,
            animation,
        }
    }

    pub fn predict(self, duration_s: f32) -> AnimatedValue<T, U, impl Animation<In = T, Out = U>>
    where
        U: Interpolate,
    {
        let (animation, current) = PredictAnimation::new_with_init(self.animation, self.current, duration_s);

        AnimatedValue {
            name: self.name,
            target: self.target,
            current,
            animation,
        }
    }

    pub fn map<V>(
        self,
        map_fn: impl Fn(U) -> V + Send + Sync + 'static,
    ) -> AnimatedValue<T, V, impl Animation<In = T, Out = V>>
    where
        V: TweenLike,
    {
        let (animation, current) = MapAnimation::new_with_init(self.animation, self.current, map_fn);

        AnimatedValue {
            name: self.name,
            target: self.target,
            current,
            animation,
        }
    }

    /// Type erase the animation by boxing it
    pub fn boxed(self) -> AnimatedValue<T, U, Box<dyn Animation<In = T, Out = U>>>
    where
        U: Interpolate,
    {
        AnimatedValue {
            name: self.name,
            target: self.target,
            current: self.current,
            animation: Box::new(self.animation),
        }
    }
}

impl<T, A> AnimatedValue<T, f32, A>
where
    T: TweenLike,
    A: Animation<In = T, Out = f32>,
{
    pub fn curve<V, C>(self, curve: C) -> AnimatedValue<T, V, impl Animation<In = T, Out = V>>
    where
        V: TweenLike,
        C: Curve<V> + Send + Sync + 'static,
    {
        let (animation, current) = CurveAnimation::new_with_init(self.animation, self.current, curve);

        AnimatedValue {
            name: self.name,
            target: self.target,
            current,
            animation,
        }
    }
}

pub trait IntoAnimatedVariable: Variable {
    fn animated(self) -> AnimatedValue<Self>
    where
        Self: Sized + ValueLike,
    {
        AnimatedValue::new(self.clone(), self)
    }
}

impl IntoAnimatedVariable for f32 {}
impl IntoAnimatedVariable for Vec2 {}
impl IntoAnimatedVariable for Vec3 {}
impl IntoAnimatedVariable for Vec4 {}
impl IntoAnimatedVariable for Quat {}
