use crate::replay::{CommandContext, StableHash};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;
use uuid::Uuid;

/// A state rebuilt by replaying an ordered command log. `apply` must be deterministic.
pub trait ReplayState: Clone + Default + Serialize + DeserializeOwned + StableHash + Send + Sync + 'static {
    type Command: Clone + fmt::Debug + Serialize + DeserializeOwned + Send + Sync + 'static;
    type Reject: fmt::Debug;

    /// Stream kind name, stored with the stream and sent by the clients.
    const KIND: &'static str;

    /// Applies atomically: on `Err` the state is unchanged. The context names the command's version, id
    /// and author, so a state can record who wrote what.
    fn apply(&mut self, ctx: &CommandContext, command: &Self::Command) -> Result<(), Self::Reject>;

    /// Ingest-time shape limits; replay never calls it.
    fn validate(_command: &Self::Command) -> bool {
        true
    }

    /// The id of the earlier command this command acts on. Ingest rejects the command when that
    /// command's author is another user.
    fn refers_to(_command: &Self::Command) -> Option<Uuid> {
        None
    }
}
