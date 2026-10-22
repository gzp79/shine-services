use crate::replica::{stable_hash_of, CommandHeader, StableHash};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

/// A command: the payload of one entry in the log, decoded and canonical. Hashable on its own,
/// independent of where it sits in the log — see `CommandHeader::chain_hash`.
pub trait Command: Clone + fmt::Debug + Serialize + DeserializeOwned + StableHash + Send + Sync + 'static {
    /// The command's own content hash, independent of its position in the log.
    fn content_hash(&self) -> u64 {
        stable_hash_of(self)
    }
}

impl<T: Clone + fmt::Debug + Serialize + DeserializeOwned + StableHash + Send + Sync + 'static> Command for T {}

/// A state rebuilt by replaying an ordered command log. `apply` must be deterministic.
pub trait ReplayState: Clone + Default + Serialize + DeserializeOwned + StableHash + Send + Sync + 'static {
    type Command: Command;

    /// Stream kind name, stored with the stream and sent by the clients.
    const KIND: &'static str;

    /// Integrates the command. It is called only for the next version of the log (never with a gap or a
    /// repeat), and once called the command counts as integrated, whatever the state does with it.
    /// There is no error path: the runtime cannot act on a refusal, so a command the state does not
    /// accept (an unknown target, a command of another user) simply changes nothing, and a command must
    /// not leave the state half applied. The header names the command's version, id and author. Must be
    /// deterministic and must not panic.
    fn apply(&mut self, header: &CommandHeader, command: &Self::Command);

    /// Shape-only check at ingest (text length, non-empty, etc.); replay never calls it, and a
    /// well-formed command can still be silently ignored by `apply` (an unknown target, a command
    /// of another user).
    fn is_well_formed(_command: &Self::Command) -> bool {
        true
    }
}
