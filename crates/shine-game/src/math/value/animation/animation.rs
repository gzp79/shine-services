use bevy::math::{Quat, Vec2, Vec3, Vec4};

pub trait TweenLike: Clone + Send + Sync + 'static {}
impl<T> TweenLike for T where T: Clone + Send + Sync + 'static {}

// A custom animation trait
pub trait Animation: Send + Sync + 'static {
    type In: TweenLike;
    type Out: TweenLike;

    // Advance the animation by delta_time_s seconds, given the current input value.
    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out;
}

impl<T, U> Animation for Box<dyn Animation<In = T, Out = U>>
where
    T: TweenLike,
    U: TweenLike,
{
    type In = T;
    type Out = U;

    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out {
        self.as_mut().animate(current, delta_time_s)
    }
}

macro_rules! impl_animation_for_primitive {
    ($target_type:ty) => {
        impl Animation for $target_type {
            type In = ();
            type Out = $target_type;

            #[inline(always)]
            fn animate(&mut self, _current: (), _delta_time_s: f32) -> Self::Out {
                *self
            }
        }
    };
}

impl_animation_for_primitive!(f32);
impl_animation_for_primitive!(Vec2);
impl_animation_for_primitive!(Vec3);
impl_animation_for_primitive!(Vec4);
impl_animation_for_primitive!(Quat);
