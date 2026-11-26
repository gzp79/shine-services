use super::*;

impl InboxId {
    pub const fn new(ms: u64, seq: u64) -> Self {
        Self { ms, seq }
    }
}
