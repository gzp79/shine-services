use futures::{future::BoxFuture, FutureExt};
use std::{
    any::{Any, TypeId},
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
    Event, EventHandler, EventHandlerId, TopicEvent,
};

type HandlerMap<E> = HashMap<EventHandlerId, BoxedHandler<E>>;
// type erased fn(&mut HashMap<EventHandlerId, BoxedHandler<E>>, EventHandlerId, BoxedHandler<E>)
type AddHandler = fn(&mut dyn Any, id: EventHandlerId, Box<dyn Any + 'static>);
// type erased fn(&mut HashMap<EventHandlerId, BoxedHandler<E>>, EventHandlerId)
type RemoveHandler = fn(&mut dyn Any, id: EventHandlerId) -> bool;
// type erased fn(&HashMap<EventHandlerId, BoxedHandler<E>>, &E)
type InvokeHandler = for<'a> fn(&'a dyn Any, &'a (dyn Any + 'static)) -> BoxFuture<'a, ()>;

struct TopicHandler {
    handlers: Arc<RwLock<dyn Any + Send + Sync + 'static>>,
    add_handler: AddHandler,
    remove_handler: RemoveHandler,
    invoke_handler: InvokeHandler,
}

impl TopicHandler {
    fn new<E>() -> Self
    where
        E: Event,
    {
        let handlers = HandlerMap::<E>::default();

        let add_handler: AddHandler = |handlers, id, handler| {
            let handlers = handlers.downcast_mut::<HandlerMap<E>>().unwrap();
            let handler = *handler.downcast::<BoxedHandler<E>>().unwrap();
            handlers.insert(id, handler);
        };
        let remove_handler: RemoveHandler = |handlers, id| {
            let handlers = handlers.downcast_mut::<HandlerMap<E>>().unwrap();
            handlers.remove(&id);
            handlers.is_empty()
        };
        let invoke_handler: InvokeHandler = |handlers, event| {
            let handlers = handlers.downcast_ref::<HandlerMap<E>>().unwrap();
            let event = event.downcast_ref::<E>().unwrap();
            let futures = handlers.iter().map(|(_id, h)| h.handle(event));
            async {
                futures::future::join_all(futures).await;
            }
            .boxed()
        };

        Self {
            handlers: Arc::new(RwLock::new(handlers)),
            add_handler,
            remove_handler,
            invoke_handler,
        }
    }

    async fn add_handler<E, H>(&mut self, id: EventHandlerId, handler: H)
    where
        E: Event,
        H: EventHandler<E>,
    {
        let handler: BoxedHandler<E> = Box::new(WrappedBoxingHandler(handler, PhantomData));
        // second box is required as any can be moved out only from a Box<dyn Any>
        let handler = Box::new(handler);
        let mut handlers = self.handlers.write().await;
        let handlers: &mut dyn Any = &mut *handlers;
        (self.add_handler)(handlers, id, handler);
    }

    async fn remove_handler(&mut self, id: EventHandlerId) -> bool {
        let mut handlers = self.handlers.write().await;
        let handlers: &mut dyn Any = &mut *handlers;
        (self.remove_handler)(handlers, id)
    }

    async fn invoke_handler<'a, E>(&'a self, event: &'a E)
    where
        E: Event,
    {
        let handlers = self.handlers.read().await;
        let handlers: &dyn Any = &*handlers;
        (self.invoke_handler)(handlers, event).await;
    }
}

struct Inner<T>
where
    T: Send + Sync,
{
    next_handler_id: AtomicUsize,
    topics: RwLock<HashMap<TypeId, TopicHandler>>,
    domain: PhantomData<T>,
}

#[derive(Clone)]
pub struct TopicBus<T>(Arc<Inner<T>>)
where
    T: Send + Sync;

impl<T> Default for TopicBus<T>
where
    T: Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TopicBus<T>
where
    T: Send + Sync,
{
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            next_handler_id: AtomicUsize::new(1),
            topics: Default::default(),
            domain: PhantomData,
        }))
    }

    pub async fn subscribe<E, H>(&self, handler: H) -> EventHandlerId
    where
        E: TopicEvent<Topic = T>,
        H: EventHandler<E>,
    {
        let topic_id = TypeId::of::<E>();

        let handler_id = EventHandlerId(self.0.next_handler_id.fetch_add(1, Ordering::Relaxed));
        let mut topics = self.0.topics.write().await;
        match topics.get_mut(&topic_id) {
            Some(topic) => {
                topic.add_handler(handler_id.clone(), handler).await;
            }
            None => {
                let mut topic = TopicHandler::new::<E>();
                topic.add_handler(handler_id.clone(), handler).await;
                topics.insert(topic_id, topic);
            }
        }

        handler_id
    }

    pub async fn unsubscribe(&self, handler_id: &EventHandlerId) {
        let mut topics = self.0.topics.write().await;

        let mut empty_topics = Vec::new();
        for (topic_id, topic) in topics.iter_mut() {
            if topic.remove_handler(handler_id.clone()).await {
                empty_topics.push(*topic_id);
            }
        }

        for topic_id in empty_topics {
            topics.remove(&topic_id);
        }
    }

    pub async fn publish<E>(&self, event: &E)
    where
        E: TopicEvent<Topic = T>,
    {
        let topics = self.0.topics.read().await;
        let topic_id = TypeId::of::<E>();

        if let Some(topic) = topics.get(&topic_id) {
            topic.invoke_handler(event).await;
        }
    }
}
