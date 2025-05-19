use shine_infra::sync::{Event, EventBus, EventHandler};
use shine_test::test;
use std::sync::{
    atomic::{AtomicIsize, Ordering},
    Arc,
};

pub enum UserEvent {
    Add(isize),
    Remove(isize),
}

impl Event for UserEvent {}

#[test]
async fn test_event_bus() {
    let bus = EventBus::<UserEvent>::new();
    let add_count = Arc::new(AtomicIsize::new(0));
    let count = Arc::new(AtomicIsize::new(0));

    #[derive(Clone)]
    struct OnUserEvent(Arc<AtomicIsize>);
    impl EventHandler<UserEvent> for OnUserEvent {
        async fn handle(&self, event: &UserEvent) {
            match event {
                UserEvent::Add(count) => self.0.fetch_add(*count, Ordering::Relaxed),
                UserEvent::Remove(count) => self.0.fetch_sub(*count, Ordering::Relaxed),
            };
        }
    }

    #[derive(Clone)]
    struct OnAddEvent(Arc<AtomicIsize>);
    impl EventHandler<UserEvent> for OnAddEvent {
        async fn handle(&self, event: &UserEvent) {
            if let UserEvent::Add(count) = event {
                self.0.fetch_add(*count, Ordering::Relaxed);
            }
        }
    }

    bus.subscribe(OnUserEvent(count.clone())).await;
    let add_id = bus.subscribe(OnAddEvent(add_count.clone())).await;

    bus.publish(&UserEvent::Add(1)).await;
    bus.publish(&UserEvent::Add(2)).await;
    bus.publish(&UserEvent::Add(3)).await;
    bus.publish(&UserEvent::Remove(4)).await;
    assert_eq!(add_count.load(Ordering::Relaxed), 6);
    assert_eq!(count.load(Ordering::Relaxed), 2);

    bus.unsubscribe(&add_id).await;

    bus.publish(&UserEvent::Add(1)).await;
    bus.publish(&UserEvent::Remove(2)).await;
    assert_eq!(add_count.load(Ordering::Relaxed), 6);
    assert_eq!(count.load(Ordering::Relaxed), 1);
}
