use std::future::Future;
use tokio::{
    sync::watch,
    time::{self, Duration, MissedTickBehavior},
};
use tracing::Instrument;

/// A soft queue limit with hysteresis: blocks at the high watermark and releases at the low one.
///
/// A spawned task polls the async count callback every `poll_period`. Waiting is blocked until the
/// first count arrives. Dropping every handle stops the polling task.
#[derive(Clone)]
pub struct Backpressure {
    rx: watch::Receiver<bool>,
    low_watermark: usize,
    high_watermark: usize,
}

impl Backpressure {
    /// # Panics
    /// If `low_watermark >= high_watermark` or `poll_period` is zero.
    pub fn new<F, Fut>(low_watermark: usize, high_watermark: usize, poll_period: Duration, count: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = usize> + Send + 'static,
    {
        assert!(
            low_watermark < high_watermark,
            "low_watermark must be less than high_watermark"
        );
        assert!(!poll_period.is_zero(), "poll_period must be non-zero");

        let (tx, rx) = watch::channel(true);
        tokio::spawn(async move {
            let mut interval = time::interval(poll_period);
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
            let mut blocked = false;
            loop {
                let pending = tokio::select! {
                    () = tx.closed() => return,
                    pending = async { interval.tick().await; count().await } => pending,
                };
                blocked = if blocked {
                    pending > low_watermark
                } else {
                    pending >= high_watermark
                };
                tx.send_if_modified(|current| {
                    if *current == blocked {
                        false
                    } else {
                        *current = blocked;
                        true
                    }
                });
            }
        });

        Self {
            rx,
            low_watermark,
            high_watermark,
        }
    }

    /// Waits until the queue can accept more messages. Cancellation does not change the limit.
    /// This only borrows the handle, so multiple producers may wait concurrently.
    pub async fn wait(&self) {
        let mut rx = self.rx.clone();
        if !*rx.borrow_and_update() {
            return;
        }

        let span = tracing::info_span!(
            "backpressure.wait",
            low_watermark = self.low_watermark,
            high_watermark = self.high_watermark
        );
        async move {
            let _ = rx.wait_for(|blocked| !*blocked).await;
        }
        .instrument(span)
        .await
    }

    /// Whether the queue is blocked, including while the initial count is being obtained.
    pub fn is_blocked(&self) -> bool {
        *self.rx.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::time::timeout;

    const POLL: Duration = Duration::from_millis(1);

    async fn state(backpressure: &Backpressure, blocked: bool) {
        timeout(
            Duration::from_secs(2),
            backpressure.rx.clone().wait_for(|value| *value == blocked),
        )
        .await
        .unwrap()
        .unwrap();
    }

    fn counter(pending: &Arc<AtomicUsize>) -> impl Fn() -> std::future::Ready<usize> + Send + 'static {
        let pending = pending.clone();
        move || std::future::ready(pending.load(Ordering::SeqCst))
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn throttles_at_high_watermark_until_low_watermark_and_releases_all_waiters() {
        let pending = Arc::new(AtomicUsize::new(4));
        let backpressure = Backpressure::new(3, 5, POLL, counter(&pending));
        state(&backpressure, false).await;
        backpressure.wait().await;

        pending.store(5, Ordering::SeqCst);
        state(&backpressure, true).await;
        assert!(backpressure.is_blocked());
        assert!(timeout(Duration::from_millis(10), backpressure.wait()).await.is_err());

        pending.store(4, Ordering::SeqCst);
        assert!(timeout(Duration::from_millis(10), backpressure.wait()).await.is_err());

        let first = backpressure.clone();
        let second = backpressure.clone();
        let first_waiter = tokio::spawn(async move { first.wait().await });
        let second_waiter = tokio::spawn(async move { second.wait().await });
        pending.store(3, Ordering::SeqCst);
        state(&backpressure, false).await;
        timeout(Duration::from_secs(2), first_waiter).await.unwrap().unwrap();
        timeout(Duration::from_secs(2), second_waiter).await.unwrap().unwrap();
        assert!(!backpressure.is_blocked());
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn waits_for_initial_count_and_stops_polling_when_all_handles_are_dropped() {
        let (ready_tx, ready_rx) = watch::channel(false);
        let pending = Arc::new(AtomicUsize::new(0));
        let tracked = pending.clone();
        let backpressure = Backpressure::new(3, 5, POLL, move || {
            let mut ready = ready_rx.clone();
            let pending = tracked.clone();
            async move {
                ready.wait_for(|ready| *ready).await.unwrap();
                pending.load(Ordering::SeqCst)
            }
        });
        assert!(backpressure.is_blocked());
        assert!(timeout(Duration::from_millis(10), backpressure.wait()).await.is_err());

        ready_tx.send_replace(true);
        timeout(Duration::from_secs(2), backpressure.wait()).await.unwrap();

        drop(backpressure);
        timeout(Duration::from_secs(2), async {
            while Arc::strong_count(&pending) > 1 {
                time::sleep(POLL).await;
            }
        })
        .await
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "low_watermark must be less than high_watermark")]
    fn rejects_equal_watermarks() {
        Backpressure::new(5, 5, POLL, || async { 0 });
    }

    #[test]
    #[should_panic(expected = "poll_period must be non-zero")]
    fn rejects_zero_poll_period() {
        Backpressure::new(3, 5, Duration::ZERO, || async { 0 });
    }
}
