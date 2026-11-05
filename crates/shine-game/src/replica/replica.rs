use crate::replica::{fold_chain_hash, stable_hash_of, Command, ReplayState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Everything about a command except its own payload: its position in the log, its author, and the
/// chain hash through it (see `Replica::apply_next`). What `ReplayState::apply` receives next to the
/// command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandHeader {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    /// The chain hash of all the commands up to and including this one.
    pub chain_hash: u64,
}

/// A decoded command at its position in the log. A command that cannot be decoded never gets this far:
/// it means an inconsistent store or a missing validation at ingest, and is an error of the caller.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandEnvelope<C> {
    pub header: CommandHeader,
    pub command: C,
    pub command_hash: u64,
}

impl<C: Command> CommandEnvelope<C> {
    /// Builds an envelope for `command` at `version`, folding its content hash onto `chain_hash_before`
    /// (the chain hash through the previous command) to fill in `CommandHeader::chain_hash`.
    pub fn new(version: i64, command_id: Uuid, user_id: Uuid, chain_hash_before: u64, command: C) -> Self {
        let command_hash = command.content_hash();
        let chain_hash = fold_chain_hash(chain_hash_before, version, command_hash);
        Self {
            header: CommandHeader {
                version,
                command_id,
                user_id,
                chain_hash,
            },
            command,
            command_hash,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// Integrated: the state received the command (what it did with it is its own business).
    Applied,
    /// The version was already applied; nothing changed.
    Skipped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ApplyError {
    #[error("expected version {expected}, got {got}")]
    Gap { expected: i64, got: i64 },
    /// The header's `chain_hash` does not match what this replica's own history would produce.
    #[error("chain hash diverged")]
    Diverged,
}

/// State version information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub version: i64,
    pub chain_hash: u64,
    pub state_hash: u64,
}

/// A state with the version of the last command applied to it, and the chain hash through that
/// version.
#[derive(Clone, Debug)]
pub struct Replica<S: ReplayState> {
    state: S,
    version: i64,
    chain_hash: u64,
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
            chain_hash: 0,
        }
    }

    /// Resumes from a snapshot at `version`, with the chain hash through it (from `Checkpoint`/
    /// `CommandHeader::chain_hash` at that version, not recomputed here).
    pub fn from_snapshot(version: i64, state: S, chain_hash: u64) -> Self {
        Self { state, version, chain_hash }
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

    /// The chain hash through this replica's own `version`.
    pub fn chain_hash(&self) -> u64 {
        self.chain_hash
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            version: self.version,
            chain_hash: self.chain_hash,
            state_hash: self.hash(),
        }
    }

    /// Applies the next command of the log. Checks the version is contiguous and that the header's
    /// `chain_hash` continues this replica's own chain, so a tampered, reordered, or diverged command
    /// is rejected before it ever reaches `ReplayState::apply`.
    pub fn apply_next(&mut self, envelope: &CommandEnvelope<S::Command>) -> Result<ApplyOutcome, ApplyError> {
        let header = &envelope.header;
        if header.version <= self.version {
            return Ok(ApplyOutcome::Skipped);
        }
        if header.version != self.version + 1 {
            return Err(ApplyError::Gap {
                expected: self.version + 1,
                got: header.version,
            });
        }
        let expected_chain_hash = fold_chain_hash(self.chain_hash, header.version, envelope.command_hash);
        if header.chain_hash != expected_chain_hash {
            return Err(ApplyError::Diverged);
        }
        self.version = header.version;
        self.chain_hash = header.chain_hash;
        self.state.apply(header, &envelope.command);
        Ok(ApplyOutcome::Applied)
    }
}
