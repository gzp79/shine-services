#[derive(Clone, Debug)]
pub struct CommandStreamConfig {
    /// Most inbox entries read in one sync.
    pub poll_limit: usize,
    /// Most commands waiting to be persisted before the inbox is not read any further.
    pub max_pending: usize,
    /// Most commands read from the log in one page when catching up or loading.
    pub page_size: i64,
    /// How many times a sync tries to persist, catching up between the tries, before it leaves the rest
    /// to the next sync.
    pub max_attempts: u32,
}

impl Default for CommandStreamConfig {
    fn default() -> Self {
        Self {
            poll_limit: 128,
            max_pending: 512,
            page_size: 256,
            max_attempts: 4,
        }
    }
}
