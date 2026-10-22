use shine_game::replica::{ChatCommand, CommandEnvelope};
use uuid::Uuid;

/// A user id for the tests.
pub fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The id of the command at `version`; it is the id of the message an `add` at that version creates.
pub fn command_id(version: i64) -> Uuid {
    Uuid::from_u128(1000 + version as u128)
}

/// Builds a sequence of chat envelopes with a correctly chained `chain_hash`, as if applied in order
/// from a fresh replica. One `Log` per replica under test: it tracks the chain on its own, separate
/// from (and not verified against) whatever the replica itself computes.
#[derive(Default)]
pub struct Log {
    chain_hash: u64,
}

impl Log {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, version: i64, user_id: Uuid, text: &str) -> CommandEnvelope<ChatCommand> {
        self.push(version, user_id, ChatCommand::Add { text: text.to_string() })
    }

    pub fn remove(&mut self, version: i64, user_id: Uuid, id: Uuid) -> CommandEnvelope<ChatCommand> {
        self.push(version, user_id, ChatCommand::Remove { id })
    }

    pub fn replace(&mut self, version: i64, user_id: Uuid, id: Uuid, text: &str) -> CommandEnvelope<ChatCommand> {
        self.push(version, user_id, ChatCommand::Replace { id, text: text.to_string() })
    }

    fn push(&mut self, version: i64, user_id: Uuid, command: ChatCommand) -> CommandEnvelope<ChatCommand> {
        let envelope = CommandEnvelope::new(version, command_id(version), user_id, self.chain_hash, command);
        self.chain_hash = envelope.header.chain_hash;
        envelope
    }
}
