use crate::math::value::{Animation, TweenLike};

/// An animation that maps the output of another animation through a function.
pub struct MapAnimation<A, F> {
    prev: A,
    map_fn: F,
}

impl<A, F, T, U> MapAnimation<A, F>
where
    T: TweenLike,
    U: TweenLike,
    A: Animation<Out = T>,
    F: Fn(T) -> U + Send + Sync + 'static,
{
    pub fn new(prev: A, map_fn: F) -> Self {
        MapAnimation { prev, map_fn }
    }

    pub fn new_with_init(prev: A, initial: T, map_fn: F) -> (Self, U) {
        let new = Self::new(prev, map_fn);
        let init = (new.map_fn)(initial);
        (new, init)
    }
}

impl<A, F, T, U> Animation for MapAnimation<A, F>
where
    T: TweenLike,
    U: TweenLike,
    A: Animation<Out = T>,
    F: Fn(T) -> U + Send + Sync + 'static,
{
    type In = A::In;
    type Out = U;

    fn animate(&mut self, current: Self::In, delta_time_s: f32) -> Self::Out {
        let output = self.prev.animate(current, delta_time_s);
        (self.map_fn)(output)
    }
}
