use shine_infra::sync::{Event, EventHandler, TopicBus, TopicEvent};
use shine_test::test;
use std::sync::{
    atomic::{AtomicIsize, Ordering},
    Arc,
};

struct UserEvent;

struct UserAdd(isize);
impl Event for UserAdd {}
impl TopicEvent for UserAdd {
    type Topic = UserEvent;
}

struct UserRemove(isize);
impl Event for UserRemove {}
impl TopicEvent for UserRemove {
    type Topic = UserEvent;
}

#[test]
async fn test_topic_bus() {
    let bus = TopicBus::<UserEvent>::new();
    let add_count = Arc::new(AtomicIsize::new(0));
    let count = Arc::new(AtomicIsize::new(0));

    #[derive(Clone)]
    struct OnUserAdd(Arc<AtomicIsize>);
    impl EventHandler<UserAdd> for OnUserAdd {
        async fn handle(&self, event: &UserAdd) {
            self.0.fetch_add(event.0, Ordering::Relaxed);
        }
    }
    #[derive(Clone)]
    struct OnUserRemove(Arc<AtomicIsize>);
    impl EventHandler<UserRemove> for OnUserRemove {
        async fn handle(&self, event: &UserRemove) {
            self.0.fetch_sub(event.0, Ordering::Relaxed);
        }
    }

    bus.subscribe(OnUserAdd(count.clone())).await;
    bus.subscribe(OnUserRemove(count.clone())).await;
    let add_id = bus.subscribe(OnUserAdd(add_count.clone())).await;

    bus.publish(&UserAdd(1)).await;
    bus.publish(&UserAdd(2)).await;
    bus.publish(&UserAdd(3)).await;
    bus.publish(&UserRemove(4)).await;
    assert_eq!(add_count.load(Ordering::Relaxed), 6);
    assert_eq!(count.load(Ordering::Relaxed), 2);

    bus.unsubscribe(&add_id).await;

    bus.publish(&UserAdd(1)).await;
    bus.publish(&UserRemove(2)).await;
    assert_eq!(add_count.load(Ordering::Relaxed), 6);
    assert_eq!(count.load(Ordering::Relaxed), 1);
}
