use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{sync::watch, task::JoinHandle};

struct Inner {
    shutdown_started: AtomicBool,
    failed: watch::Sender<Option<String>>,
}

/// Watches the background tasks that must live as long as their owner. They end on their own only when the
/// owner shuts down, so one that ends before is a failure: whatever it did is not done any more, and the
/// owner is better restarted than left half alive. The supervisor only records it; what to do about it
/// (typically a graceful shutdown of the service) is the owner's decision.
///
/// Cheap to clone, all clones share one state.
#[derive(Clone)]
pub struct TaskSupervisor {
    inner: Arc<Inner>,
}

impl Default for TaskSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskSupervisor {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                shutdown_started: AtomicBool::new(false),
                failed: watch::channel(None).0,
            }),
        }
    }

    /// The owner has started to shut down on purpose: its tasks are expected to end from now on. Call it before
    /// anything that makes them end.
    pub fn shutdown_started(&self) {
        self.inner.shutdown_started.store(true, Ordering::SeqCst);
    }

    /// Spawns a task that is expected to run until the owner stops: it is reported as failed if it returns
    /// before. A task that is aborted is not reported.
    pub fn spawn<F>(&self, name: impl Into<String>, task: F) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let supervisor = self.clone();
        let name = name.into();
        tokio::spawn(async move {
            task.await;
            supervisor.task_ended(&name);
        })
    }

    /// A task ended. Unless the shutdown has started, that is a failure. For the tasks that are not started
    /// with [`spawn`](Self::spawn), or that are expected to end in some cases and not in others.
    pub fn task_ended(&self, task: &str) {
        if self.inner.shutdown_started.load(Ordering::SeqCst) {
            return;
        }
        tracing::error!("The task {task} stopped unexpectedly");
        // The first failure is the one that is kept, the later ones are most likely its consequence.
        self.inner.failed.send_if_modified(|failed| {
            if failed.is_some() {
                return false;
            }
            *failed = Some(task.to_owned());
            true
        });
    }

    /// Waits until a task has ended unexpectedly, and returns its name. Never returns otherwise.
    pub async fn wait_failed(&self) -> String {
        let mut failed = self.inner.failed.subscribe();
        loop {
            if let Some(task) = failed.borrow_and_update().clone() {
                return task;
            }
            if failed.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    const SHORT: Duration = Duration::from_millis(100);

    #[shine_test::test(flavor = "current_thread")]
    async fn a_task_that_ends_unexpectedly_is_reported() {
        let supervisor = TaskSupervisor::new();

        supervisor.task_ended("session checker");

        assert_eq!(
            timeout(SHORT, supervisor.wait_failed()).await,
            Ok("session checker".to_string())
        );
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn the_first_failure_is_the_one_kept() {
        let supervisor = TaskSupervisor::new();

        supervisor.task_ended("session checker");
        supervisor.task_ended("heartbeat");

        assert_eq!(
            timeout(SHORT, supervisor.wait_failed()).await,
            Ok("session checker".to_string())
        );
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn a_task_that_ends_once_the_shutdown_started_is_not_a_failure() {
        let supervisor = TaskSupervisor::new();

        supervisor.shutdown_started();
        supervisor.task_ended("session checker");

        assert!(timeout(SHORT, supervisor.wait_failed()).await.is_err());
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn a_failure_that_comes_while_waiting_is_reported() {
        let supervisor = TaskSupervisor::new();
        let waiting = tokio::spawn({
            let supervisor = supervisor.clone();
            async move { supervisor.wait_failed().await }
        });
        tokio::task::yield_now().await;

        supervisor.task_ended("heartbeat");

        assert_eq!(timeout(SHORT, waiting).await.unwrap().unwrap(), "heartbeat".to_string());
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn a_spawned_task_that_returns_is_reported_by_name() {
        let supervisor = TaskSupervisor::new();

        supervisor.spawn("worker", async {});

        assert_eq!(timeout(SHORT, supervisor.wait_failed()).await, Ok("worker".to_string()));
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn a_spawned_task_that_returns_once_the_shutdown_started_is_not_reported() {
        let supervisor = TaskSupervisor::new();
        supervisor.shutdown_started();

        supervisor.spawn("worker", async {}).await.unwrap();

        assert!(timeout(SHORT, supervisor.wait_failed()).await.is_err());
    }

    #[shine_test::test(flavor = "current_thread")]
    async fn a_spawned_task_that_is_aborted_is_not_reported() {
        let supervisor = TaskSupervisor::new();
        let task = supervisor.spawn("worker", std::future::pending());

        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());

        assert!(timeout(SHORT, supervisor.wait_failed()).await.is_err());
    }
}
