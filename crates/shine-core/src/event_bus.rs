use futures::{
    future::{join_all, BoxFuture},
    FutureExt,
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    future::Future,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::RwLock;

/// Abstraction for event connected to the D domain.
pub trait Event: Send + Sync {
    type Domain: 'static;
}

/// Handle event synchronously with the event publish.
pub trait EventHandler<E>: Send + Sync
where
    E: Event + Send + Sync + 'static,
{
    fn handle(self, event: &E) -> impl Future<Output = ()> + Send + '_;
}

#[derive(Eq, Hash, PartialEq, Clone, Debug, Default)]
pub struct EventHandlerId(usize);

type BoxedHandler = Box<dyn for<'a> Fn(&'a (dyn Any + Send + Sync + 'static)) -> BoxFuture<'a, ()>>;
type HandlerMap = Arc<RwLock<HashMap<EventHandlerId, BoxedHandler>>>;
type EventHandlerMap = RwLock<HashMap<TypeId, HandlerMap>>;

struct Inner<D>
where
    D: Send + Sync + 'static,
{
    next_handler_id: AtomicUsize,
    event_handlers: EventHandlerMap,
    domain: PhantomData<D>,
}

#[derive(Clone)]
pub struct EventBus<D: Send + Sync + 'static>(Arc<Inner<D>>);

impl<D> EventBus<D>
where
    D: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            next_handler_id: AtomicUsize::new(1),
            event_handlers: Default::default(),
            domain: PhantomData,
        }))
    }

    pub async fn subscribe<E, H>(&self, handler: H) -> EventHandlerId
    where
        E: Event<Domain = D> + 'static,
        H: EventHandler<E> + Clone + 'static,
    {
        let handler: BoxedHandler = Box::new(move |e| {
            let handler = handler.clone();
            async move {
                let e = e.downcast_ref::<E>().expect("downcast failed for event");
                handler.handle(e).await
            }
            .boxed()
        });

        let mut event_handlers = self.0.event_handlers.write().await;

        let handler_id = EventHandlerId(self.0.next_handler_id.fetch_add(1, Ordering::Relaxed));
        let event_type = TypeId::of::<E>();

        match event_handlers.get(&event_type) {
            Some(handlers) => {
                let mut handlers = handlers.write().await;
                handlers.insert(handler_id.clone(), handler);
            }
            None => {
                let mut handlers = HashMap::new();
                handlers.insert(handler_id.clone(), handler);
                event_handlers.insert(event_type, Arc::new(RwLock::new(handlers)));
            }
        };

        handler_id
    }

    pub async fn unsubscribe(&self, handler_id: &EventHandlerId) {
        let event_handlers = self.0.event_handlers.write().await;

        for (_, handlers) in event_handlers.iter() {
            let mut handlers = handlers.write().await;
            handlers.remove(handler_id);
        }
    }

    pub async fn publish<E>(&self, event: &E)
    where
        E: Event<Domain = D> + 'static,
    {
        let event_handlers = self.0.event_handlers.read().await;
        let event_type = TypeId::of::<E>();
        let event: &(dyn Any + Send + Sync + 'static) = event;

        if let Some(handlers) = event_handlers.get(&event_type) {
            let handlers = handlers.read().await;
            let futures = handlers.iter().map(|(_id, h)| (h)(event));
            join_all(futures).await;
        }
    }
}
