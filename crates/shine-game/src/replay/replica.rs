use crate::replay::{stable_hash_of, Checkpoint, CommandContext, CommandEnvelope, ReplayState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    /// Rejected or undecodable; the version is still consumed.
    Ignored,
    /// The version was already applied; nothing changed.
    AlreadyApplied,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("expected version {expected}, got {got}")]
pub struct Gap {
    pub expected: i64,
    pub got: i64,
}

/// A state with the version of the last command applied to it.
#[derive(Clone, Debug)]
pub struct Replica<S: ReplayState> {
    state: S,
    version: i64,
}

impl<S: ReplayState> Default for Replica<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: ReplayState> Replica<S> {
    pub fn new() -> Self {
        Self {
            state: S::default(),
            version: 0,
        }
    }

    pub fn from_snapshot(version: i64, state: S) -> Self {
        Self { state, version }
    }

    pub fn version(&self) -> i64 {
        self.version
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn hash(&self) -> u64 {
        stable_hash_of(&self.state)
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            version: self.version,
            hash: self.hash(),
        }
    }

    /// Applies the next command of the log.
    pub fn apply_next(&mut self, envelope: &CommandEnvelope<S::Command>) -> Result<ApplyOutcome, Gap> {
        if envelope.version <= self.version {
            return Ok(ApplyOutcome::AlreadyApplied);
        }
        if envelope.version != self.version + 1 {
            return Err(Gap {
                expected: self.version + 1,
                got: envelope.version,
            });
        }
        self.version = envelope.version;
        let ctx = CommandContext {
            version: envelope.version,
            command_id: envelope.command_id,
            user_id: envelope.user_id,
        };
        match &envelope.command {
            Some(command) if self.state.apply(&ctx, command).is_ok() => Ok(ApplyOutcome::Applied),
            _ => Ok(ApplyOutcome::Ignored),
        }
    }
}
