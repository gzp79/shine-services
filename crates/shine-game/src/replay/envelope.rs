use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A command at its position in the log. `command` is `None` when it could not be decoded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope<C> {
    pub version: i64,
    pub command_id: Uuid,
    pub user_id: Uuid,
    pub command: Option<C>,
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
