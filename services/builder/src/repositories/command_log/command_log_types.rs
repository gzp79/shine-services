// Not read by anything outside this module's own tests yet: the service layer that calls through
// `CommandLogDb` is a later round.
#![allow(dead_code)]

use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamHead {
    /// Highest persisted version.
    pub version: i64,
    /// Chain hash, folded by the appender over each command's envelope.
    pub chain_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCommand {
    pub version: i64,
    pub command_id: Uuid,
    pub inbox_id: Option<String>,
    pub user_id: Uuid,
    pub data: String,
    pub command_hash: u64,
    pub chain_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub version: i64,
    pub hash: u64,
    pub chain_hash: u64,
    pub data: Vec<u8>,
}

/// The result of `check_consistency`: `checked_up_to_version` is the head version read at the
/// start of the check, before any of the rows it then re-verified were read. A concurrent append
/// past that point is simply not part of this pass — the check never blocks on, or is corrupted
/// by, writes happening at the same time; a later check sees them instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsistencyReport {
    pub checked_up_to_version: i64,
    pub issues: Vec<ConsistencyIssue>,
}

/// One thing `check_consistency` found wrong. Finding one does not stop the check: everything
/// detectable in a single pass is reported together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsistencyIssue {
    /// The stored commands are not a dense run from 1: `found` is the first version seen after
    /// `after`.
    Gap { after: i64, found: i64 },
    /// `version`'s stored `chain_hash` is not the fold of the command before it onto its own
    /// `command_hash`.
    CommandChainBroken { version: i64 },
    /// The snapshot at `version` has a `chain_hash` that disagrees with the command stored at the
    /// same version.
    SnapshotDrift { version: i64 },
    /// The stream head's `chain_hash` disagrees with the last stored command's `chain_hash`.
    HeadMismatch {
        head_chain_hash: u64,
        last_command_chain_hash: u64,
    },
}
