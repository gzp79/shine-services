use shine_game::replay::{ChatCommand, CommandEnvelope};
use uuid::Uuid;

/// A user id for the tests.
pub fn user(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The id of the command at `version`; it is the id of the message an `add` at that version creates.
pub fn command_id(version: i64) -> Uuid {
    Uuid::from_u128(1000 + version as u128)
}

pub fn add(version: i64, user_id: Uuid, text: &str) -> CommandEnvelope<ChatCommand> {
    CommandEnvelope {
        version,
        command_id: command_id(version),
        user_id,
        command: ChatCommand::Add { text: text.to_string() },
    }
}

pub fn remove(version: i64, user_id: Uuid, id: Uuid) -> CommandEnvelope<ChatCommand> {
    CommandEnvelope {
        version,
        command_id: command_id(version),
        user_id,
        command: ChatCommand::Remove { id },
    }
}

pub fn replace(version: i64, user_id: Uuid, id: Uuid, text: &str) -> CommandEnvelope<ChatCommand> {
    CommandEnvelope {
        version,
        command_id: command_id(version),
        user_id,
        command: ChatCommand::Replace { id, text: text.to_string() },
    }
}
