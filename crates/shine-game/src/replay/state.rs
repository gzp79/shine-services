use crate::replay::{CommandContext, StableHash};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

/// A state rebuilt by replaying an ordered command log. `apply` must be deterministic.
pub trait ReplayState: Clone + Default + Serialize + DeserializeOwned + StableHash + Send + Sync + 'static {
    type Command: Clone + fmt::Debug + Serialize + DeserializeOwned + Send + Sync + 'static;

    /// Stream kind name, stored with the stream and sent by the clients.
    const KIND: &'static str;

    /// Integrates the command. It is called only for the next version of the log (never with a gap or a
    /// repeat), and once called the command counts as integrated, whatever the state does with it.
    /// There is no error path: the runtime cannot act on a refusal, so a command the state does not
    /// accept (an unknown target, a command of another user) simply changes nothing, and a command must
    /// not leave the state half applied. The context names the command's version, id and author. Must be
    /// deterministic and must not panic.
    fn apply(&mut self, ctx: &CommandContext, command: &Self::Command);

    /// Ingest-time shape limits; replay never calls it.
    fn validate(_command: &Self::Command) -> bool {
        true
    }
}
