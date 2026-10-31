use std::time::Duration;
use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct StreamConfig {
    /// Most inbox entries read in one sync.
    #[validate(range(min = 1))]
    pub poll_limit: usize,
    /// Most commands waiting to be persisted before the inbox is not read any further.
    #[validate(range(min = 1))]
    pub max_pending: usize,
    /// Most commands read from the log in one page when catching up or loading.
    #[validate(range(min = 1))]
    pub page_size: i64,
    /// How many times a sync tries to persist, catching up between the tries, before it leaves the rest
    /// to the next sync.
    #[validate(range(min = 1))]
    pub max_attempts: u32,
    /// A snapshot is taken at every version that is a multiple of this.
    #[validate(range(min = 1))]
    pub snapshot_every: i64,
    /// How many of the latest snapshots stay in the log, with the commands from the oldest of them.
    #[validate(range(min = 1))]
    pub keep_snapshots: i64,

    /// Interval of the tick that looks at the inbox and the log even when no wake came, the safety net
    /// for a wake that was lost.
    pub backstop: Duration,
    /// Least time between two passes triggered by wakes, so a steady flow of commands is worked in
    /// batches.
    pub min_wake_spacing: Duration,
    /// How long the worker stays loaded after its last subscriber left.
    pub unload_grace: Duration,
    /// How many passes in a row may fail before the stream is given up.
    pub max_failures: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            poll_limit: 128,
            max_pending: 512,
            page_size: 256,
            max_attempts: 4,
            snapshot_every: 100,
            keep_snapshots: 3,

            backstop: Duration::from_secs(10),
            min_wake_spacing: Duration::from_millis(50),
            unload_grace: Duration::from_secs(60),
            max_failures: 5,
        }
    }
}

impl StreamConfig {
    pub fn quiet() -> Self {
        StreamConfig {
            backstop: Duration::from_secs(3600),
            min_wake_spacing: Duration::ZERO,
            unload_grace: Duration::from_secs(3600),
            max_failures: 3,
            ..Default::default()
        }
    }
}
