use validator::Validate;

#[derive(Clone, Debug, Validate)]
pub struct CommandStreamConfig {
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
}

impl Default for CommandStreamConfig {
    fn default() -> Self {
        Self {
            poll_limit: 128,
            max_pending: 512,
            page_size: 256,
            max_attempts: 4,
            snapshot_every: 100,
            keep_snapshots: 3,
        }
    }
}
