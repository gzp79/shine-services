use std::{cell::Cell, rc::{Rc, Weak}};

/// A shared, mutable version counter. Clones read the same value; a `WeakGeneration` observes it
/// while any `Generation` is alive.
#[derive(Clone)]
pub struct Generation(Rc<Cell<u64>>);

impl Generation {
    pub(super) fn new(value: u64) -> Self {
        Self(Rc::new(Cell::new(value)))
    }

    pub fn get(&self) -> u64 {
        self.0.get()
    }

    pub fn downgrade(&self) -> WeakGeneration {
        WeakGeneration(Rc::downgrade(&self.0))
    }
}

/// A weak reference to a `Generation`. `get` returns the current value while a `Generation` is
/// alive, `None` otherwise.
#[derive(Clone)]
pub struct WeakGeneration(Weak<Cell<u64>>);

impl WeakGeneration {
    pub fn get(&self) -> Option<u64> {
        self.0.upgrade().map(|cell| cell.get())
    }
}
