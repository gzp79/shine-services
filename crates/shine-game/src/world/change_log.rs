use crate::{
    indexed::{BitSet, TypedIndex},
    world::{Layer, TileIndex},
};

/// A read-only locking view of a `Layer<T>` and its change log. The layer is temporarily locked for
/// exclusive access, and the lock is released when the `ChangeLog` is dropped. Dropping the `ChangeLog`
/// also clears the change log.
pub struct ChangeLog<T> {
    layer: Option<Layer<T>>,
    restore: Option<Box<dyn FnOnce(Layer<T>)>>,
}

impl<T> ChangeLog<T> {
    pub(super) fn new(layer: Layer<T>, restore: impl FnOnce(Layer<T>) + 'static) -> Self {
        Self {
            layer: Some(layer),
            restore: Some(Box::new(restore)),
        }
    }

    pub fn values(&self) -> &[T] {
        self.layer.as_ref().expect("change log already dropped").as_slice()
    }

    pub fn log(&self) -> &BitSet {
        self.layer.as_ref().expect("change log already dropped").log()
    }

    pub fn changed(&self) -> impl Iterator<Item = TileIndex> + '_ {
        self.log().iter_set().map(TileIndex::new)
    }
}

impl<T> Drop for ChangeLog<T> {
    fn drop(&mut self) {
        if let (Some(layer), Some(restore)) = (self.layer.take(), self.restore.take()) {
            restore(layer);
        }
    }
}
