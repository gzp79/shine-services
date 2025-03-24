use shine_core::event_bus::{Event, EventBus, EventHandler};
use shine_test::test;
use std::sync::{
    atomic::{AtomicIsize, Ordering},
    Arc,
};

struct UserDomain;

struct UserAddEvent(pub isize);
impl Event for UserAddEvent {
    type Domain = UserDomain;
}

struct UserRemoveEvent(pub isize);
impl Event for UserRemoveEvent {
    type Domain = UserDomain;
}

#[test]
async fn test_event_bus() {
    let bus = EventBus::<UserDomain>::new();
    let add_count = Arc::new(AtomicIsize::new(0));
    let remove_count = Arc::new(AtomicIsize::new(0));

    #[derive(Clone)]
    struct HandleAdd(Arc<AtomicIsize>);
    impl EventHandler<UserAddEvent> for HandleAdd {
        async fn handle(self, event: &UserAddEvent) {
            self.0.fetch_add(event.0, Ordering::Relaxed);
        }
    }

    #[derive(Clone)]
    struct HandleRemove(Arc<AtomicIsize>);
    impl EventHandler<UserRemoveEvent> for HandleRemove {
        async fn handle(self, event: &UserRemoveEvent) {
            self.0.fetch_sub(event.0, Ordering::Relaxed);
        }
    }

    let add_id = bus.subscribe(HandleAdd(add_count.clone())).await;
    bus.subscribe(HandleRemove(remove_count.clone())).await;

    bus.publish(&UserAddEvent(1)).await;
    bus.publish(&UserAddEvent(2)).await;
    bus.publish(&UserAddEvent(3)).await;
    assert_eq!(add_count.load(Ordering::Relaxed), 6);
    assert_eq!(remove_count.load(Ordering::Relaxed), 0);

    bus.unsubscribe(&add_id).await;

    bus.publish(&UserAddEvent(1)).await;
    bus.publish(&UserRemoveEvent(2)).await;
    bus.publish(&UserAddEvent(3)).await;
    assert_eq!(add_count.load(Ordering::Relaxed), 6);
    assert_eq!(remove_count.load(Ordering::Relaxed), -2);
}
