use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::Notify, time::timeout};

/// How long [`ListenerEvents::wait_for`] waits for a message.
const WAIT: Duration = Duration::from_secs(5);

/// Records every call of a listener handler: `Some(payload)` is a message, `None` a resync nudge
/// (sent after each (re)connect and on the optional periodic wake). The handler is synchronous, so
/// no task needs to be spawned to record a call.
#[derive(Clone, Default)]
pub struct ListenerEvents {
    events: Arc<Mutex<Vec<Option<String>>>>,
    notify: Arc<Notify>,
}

impl ListenerEvents {
    pub fn new() -> Self {
        Self::default()
    }

    /// The handler to register with `listen`.
    pub fn handler(&self) -> impl Fn(Option<&str>) + Send + Sync + 'static {
        let this = self.clone();
        move |payload| {
            this.events.lock().unwrap().push(payload.map(str::to_string));
            this.notify.notify_one();
        }
    }

    /// Waits until a message with the `expected` payload was received; panics on timeout.
    pub async fn wait_for(&self, expected: &str) {
        let wait = async {
            loop {
                if self
                    .events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|e| e.as_deref() == Some(expected))
                {
                    return;
                }
                self.notify.notified().await;
            }
        };
        if timeout(WAIT, wait).await.is_err() {
            panic!(
                "timed out waiting for payload {expected:?}, received so far: {:?}",
                self.events.lock().unwrap()
            );
        }
    }

    /// Asserts that exactly one message with the `expected` payload was received and nothing else
    /// but `None` nudges. Returns the number of nudges.
    pub fn assert_only(&self, expected: &str) -> usize {
        let events = self.events.lock().unwrap();
        let mut messages = 0;
        let mut nudges = 0;
        for event in events.iter() {
            match event.as_deref() {
                None => nudges += 1,
                Some(payload) if payload == expected => messages += 1,
                Some(other) => panic!("unexpected payload {other:?}, expected only {expected:?}: {events:?}"),
            }
        }
        assert_eq!(messages, 1, "expected exactly one {expected:?} message: {events:?}");
        nudges
    }
}
