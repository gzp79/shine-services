use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A decoded command at its position in the log. A command that cannot be decoded never gets this far:
/// it means an inconsistent store or a missing validation at ingest, and is an error of the caller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope<C> {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub command: C,
}

/// Where a command comes from; what `ReplayState::apply` receives next to the command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandContext {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
}

/// State hash at a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub version: i64,
    pub hash: u64,
}
