use futures::future::join_all;
use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::RwLock;

use super::{
    wrapper::{BoxedHandler, WrappedBoxingHandler},
    Event, EventHandler, EventHandlerId,
};

struct Inner<E>
where
    E: Event,
{
    next_handler_id: AtomicUsize,
    handlers: RwLock<HashMap<EventHandlerId, BoxedHandler<E>>>,
}

#[derive(Clone)]
pub struct EventBus<E>(Arc<Inner<E>>)
where
    E: Event;

impl<E> Default for EventBus<E>
where
    E: Event,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<E> EventBus<E>
where
    E: Event,
{
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            next_handler_id: AtomicUsize::new(1),
            handlers: Default::default(),
        }))
    }

    pub async fn subscribe<H>(&self, handler: H) -> EventHandlerId
    where
        H: EventHandler<E>,
    {
        let handler: BoxedHandler<E> = Box::new(WrappedBoxingHandler(handler, PhantomData));
        let handler_id = EventHandlerId(self.0.next_handler_id.fetch_add(1, Ordering::Relaxed));
        let mut handlers = self.0.handlers.write().await;
        handlers.insert(handler_id.clone(), handler);

        handler_id
    }

    pub async fn unsubscribe(&self, handler_id: &EventHandlerId) {
        let mut handlers = self.0.handlers.write().await;
        handlers.remove(handler_id);
    }

    pub async fn publish(&self, event: &E) {
        let handlers = self.0.handlers.read().await;
        let futures = handlers.values().map(|h| h.handle(event));
        join_all(futures).await;
    }
}
