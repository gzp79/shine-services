use std::future::Future;

pub trait Event: Send + Sync + 'static {}

pub trait TopicEvent: Event {
    type Topic;
}

pub trait EventHandler<E>: Send + Sync + 'static
where
    E: Event + Send + Sync,
{
    fn handle<'a>(&'a self, event: &'a E) -> impl Future<Output = ()> + Send + 'a;
}

#[derive(Eq, Hash, PartialEq, Clone, Debug, Default)]
pub struct EventHandlerId(pub(in crate::sync) usize);
