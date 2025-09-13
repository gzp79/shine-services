use crate::math::value::Animation;

/// A animation that returns the current time in seconds since the animation started.
pub struct TimeAnimation(f32);

impl Default for TimeAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeAnimation {
    pub fn new() -> Self {
        TimeAnimation(0.0)
    }

    pub fn new_with_init(_current: (), duration_s: f32) -> (Self, f32) {
        let mut new = Self::new();
        let init = new.advance(duration_s);
        (new, init)
    }

    fn advance(&mut self, delta_time_s: f32) -> f32 {
        self.0 += delta_time_s;
        self.0
    }
}

impl Animation for TimeAnimation {
    type In = ();
    type Out = f32;

    #[inline(always)]
    fn animate(&mut self, _current: (), delta_time_s: f32) -> Self::Out {
        self.advance(delta_time_s)
    }
}
