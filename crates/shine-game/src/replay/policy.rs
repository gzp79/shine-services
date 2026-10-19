/// Versions at which every instance snapshots and pushes a checkpoint.
#[derive(Clone, Copy, Debug)]
pub struct CheckpointPolicy {
    every: i64,
}

impl CheckpointPolicy {
    pub fn new(every: i64) -> Self {
        Self { every: every.max(1) }
    }

    pub fn every(&self) -> i64 {
        self.every
    }

    pub fn is_checkpoint(&self, version: i64) -> bool {
        version > 0 && version % self.every == 0
    }
}
