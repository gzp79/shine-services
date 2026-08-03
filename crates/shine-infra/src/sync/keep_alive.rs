use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

/// A wakeable "keep running" flag shared between a keep-alive task and its owner. The task parks on
/// [`wait`](Self::wait) and checks [`is_running`](Self::is_running); owners call [`wake`](Self::wake)
/// to trigger a reconnect and [`stop`](Self::stop) to end the task.
pub struct KeepAlive {
    notify: Notify,
    running: AtomicBool,
}

impl KeepAlive {
    pub fn new() -> Self {
        Self {
            notify: Notify::new(),
            running: AtomicBool::new(true),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Wakes the keep-alive task, typically to trigger a reconnect.
    pub fn wake(&self) {
        self.notify.notify_one();
    }

    /// Parks until the next [`wake`](Self::wake) or [`stop`](Self::stop).
    pub async fn wait(&self) {
        self.notify.notified().await;
    }

    /// Clears the running flag and wakes the task so it shuts down.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.notify.notify_one();
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::new()
    }
}
