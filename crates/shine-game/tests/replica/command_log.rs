use shine_game::replica::{ChatCommand, ChatState, CommandEnvelope, ReplayState};
use uuid::Uuid;

/// A user id for the tests.
pub fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The id of the command at `version`; deterministic so a test can name a specific entry.
pub fn command_id(version: i64) -> Uuid {
    Uuid::from_u128(1000 + version as u128)
}

/// A correctly chained sequence of envelopes of `S`, as if applied in order from a fresh replica.
/// The version of the next entry is its own position in the log, so callers never pass one.
pub struct CommandLog<S: ReplayState> {
    chain_hash: u64,
    entries: Vec<CommandEnvelope<S::Command>>,
}

impl<S: ReplayState> Default for CommandLog<S> {
    fn default() -> Self {
        Self {
            chain_hash: 0,
            entries: Vec::new(),
        }
    }
}

impl<S: ReplayState> CommandLog<S> {
    pub fn entries(&self) -> &[CommandEnvelope<S::Command>] {
        &self.entries
    }

    pub fn push(&mut self, user_id: Uuid, command: S::Command) -> &CommandEnvelope<S::Command> {
        let version = self.entries.len() as i64 + 1;
        let envelope = CommandEnvelope::new(version, command_id(version), user_id, self.chain_hash, command);
        self.chain_hash = envelope.header.chain_hash;
        self.entries.push(envelope);
        self.entries.last().unwrap()
    }
}

/// A chat-flavored `CommandLog`.
pub type ChatCommandLog = CommandLog<ChatState>;

impl ChatCommandLog {
    /// A representative log of `len` chat commands, mostly adds with the occasional remove of an
    /// earlier message; useful as test fixture data rather than any specific scenario.
    pub fn sample(len: i64) -> Self {
        let mut log = Self::default();
        for v in 1..=len {
            if v % 5 == 0 {
                log.remove(user(v as u128 % 3), command_id(v - 2));
            } else {
                log.add(user(v as u128 % 3), &format!("message {v}"));
            }
        }
        log
    }

    pub fn add(&mut self, user_id: Uuid, text: &str) -> &CommandEnvelope<ChatCommand> {
        self.push(user_id, ChatCommand::Add { text: text.to_string() })
    }

    pub fn remove(&mut self, user_id: Uuid, id: Uuid) -> &CommandEnvelope<ChatCommand> {
        self.push(user_id, ChatCommand::Remove { id })
    }
}
