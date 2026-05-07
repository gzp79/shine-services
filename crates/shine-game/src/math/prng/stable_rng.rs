use std::{cell::RefCell, rc::Rc};

/// Minimal RNG trait for deterministic, cross-platform random generation.
pub trait StableRng {
    fn next_u32(&mut self) -> u32;

    /// Wraps this RNG in a `Rc<RefCell<Self>>` for shared access.
    fn into_rc(self) -> Rc<RefCell<Self>>
    where
        Self: Sized,
    {
        Rc::new(RefCell::new(self))
    }
}

pub trait StableRngExt: StableRng {
    /// Returns a deterministic float in [0, 1) with uniform spacing.
    fn float_unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / (1u32 << 24) as f32)
    }

    /// Returns a deterministic float in [-1, 1).
    fn float_signed(&mut self) -> f32 {
        self.float_unit() * 2.0 - 1.0
    }

    /// Returns a deterministic float in [min, max).
    fn float_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.float_unit() * (max - min)
    }

    /// Returns a deterministic i32.
    fn next_i32(&mut self) -> i32 {
        self.next_u32() as i32
    }

    /// Returns a deterministic i32 in [min, max).
    fn i32_range(&mut self, min: i32, max: i32) -> i32 {
        let d = (max - min) as u32;
        min + (self.next_u32() % d) as i32
    }
}

impl<T: StableRng + ?Sized> StableRngExt for T {}

impl<T: StableRng + ?Sized> StableRng for Box<T> {
    fn next_u32(&mut self) -> u32 {
        (**self).next_u32()
    }
}

impl<T: StableRng + ?Sized> StableRng for Rc<RefCell<T>> {
    fn next_u32(&mut self) -> u32 {
        self.borrow_mut().next_u32()
    }
}
