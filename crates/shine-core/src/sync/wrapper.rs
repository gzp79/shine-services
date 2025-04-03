use futures::{future::BoxFuture, FutureExt};
use std::marker::PhantomData;

use super::{Event, EventHandler};

/// Helper trait to make the EventHandler object safe.
pub trait WrappedHandler<E>: Send + Sync + 'static
where
    E: Event,
{
    fn handle<'a>(&'a self, event: &'a E) -> BoxFuture<'a, ()>;
}

/// The wrapper to make the EventHandler object safe by boxing the future.
pub struct WrappedBoxingHandler<E, H>(pub H, pub PhantomData<E>)
where
    E: Event,
    H: EventHandler<E>;

impl<E, H> WrappedHandler<E> for WrappedBoxingHandler<E, H>
where
    E: Event,
    H: EventHandler<E>,
{
    fn handle<'a>(&'a self, event: &'a E) -> BoxFuture<'a, ()> {
        self.0.handle(event).boxed()
    }
}

pub type BoxedHandler<E> = Box<dyn WrappedHandler<E>>;
