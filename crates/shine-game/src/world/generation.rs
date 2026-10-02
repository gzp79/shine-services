use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

/// A shared, mutable version counter.
#[derive(Clone)]
pub struct Generation(Rc<Cell<u64>>);

impl Generation {
    pub fn new(generation: u64) -> Self {
        Self(Rc::new(Cell::new(generation)))
    }

    pub fn get(&self) -> u64 {
        self.0.get()
    }

    pub fn downgrade(&self) -> WeakGeneration {
        WeakGeneration(Rc::downgrade(&self.0))
    }
}

/// A weak reference to a `Generation`. `get` returns the current value while the `Generation` is
/// alive, `None` otherwise.
#[derive(Clone)]
pub struct WeakGeneration(Weak<Cell<u64>>);

impl WeakGeneration {
    pub fn get(&self) -> Option<u64> {
        self.0.upgrade().map(|cell| cell.get())
    }
}

/// Captures a `Generation`'s value and reports whether it still matches. Reads invalid once the
/// generation advances or its owner is dropped.
#[derive(Clone)]
pub struct GenerationGuard {
    generation: WeakGeneration,
    captured: u64,
}

impl GenerationGuard {
    pub fn new(generation: &Generation) -> Self {
        Self {
            generation: generation.downgrade(),
            captured: generation.get(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.generation.get() == Some(self.captured)
    }
}
