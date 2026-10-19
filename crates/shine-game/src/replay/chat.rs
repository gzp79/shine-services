use crate::replay::{CommandContext, ReplayState, StableHash, StableHasher};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Messages the chat keeps. A constant of the kind: every replica and client must agree on it.
pub const CHAT_WINDOW: usize = 200;
/// Longest text of a message, in bytes.
pub const CHAT_MAX_TEXT_BYTES: usize = 2000;

/// A chat message; its id is the id of the command that added it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub user_id: Uuid,
    pub text: String,
    /// Set once the author has replaced the text.
    pub edited: bool,
}

/// The last `CHAT_WINDOW` messages of the room, oldest first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatState {
    pub messages: VecDeque<ChatMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatCommand {
    /// Adds a message written by the sender.
    Add { text: String },
    /// Removes the message with that id, if the sender wrote it.
    Remove { id: Uuid },
    /// Replaces the text of the message with that id, if the sender wrote it; the message becomes edited.
    Replace { id: Uuid, text: String },
}

impl ChatState {
    fn find_own(&self, id: Uuid, user_id: Uuid) -> Option<usize> {
        // An unknown id and a message of another user are both ignored by the callers.
        self.messages.iter().position(|m| m.id == id && m.user_id == user_id)
    }
}

impl StableHash for ChatState {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (self.messages.len() as u64).stable_hash(hasher);
        for message in &self.messages {
            message.id.stable_hash(hasher);
            message.user_id.stable_hash(hasher);
            message.text.as_str().stable_hash(hasher);
            message.edited.stable_hash(hasher);
        }
    }
}

impl ReplayState for ChatState {
    type Command = ChatCommand;

    const KIND: &'static str = "chat";

    fn apply(&mut self, ctx: &CommandContext, command: &ChatCommand) {
        match command {
            ChatCommand::Add { text } => {
                self.messages.push_back(ChatMessage {
                    id: ctx.command_id,
                    user_id: ctx.user_id,
                    text: text.clone(),
                    edited: false,
                });
                while self.messages.len() > CHAT_WINDOW {
                    self.messages.pop_front();
                }
            }
            ChatCommand::Remove { id } => {
                if let Some(index) = self.find_own(*id, ctx.user_id) {
                    self.messages.remove(index);
                }
            }
            ChatCommand::Replace { id, text } => {
                if let Some(index) = self.find_own(*id, ctx.user_id) {
                    let message = &mut self.messages[index];
                    message.text = text.clone();
                    message.edited = true;
                }
            }
        }
    }

    fn validate(command: &ChatCommand) -> bool {
        match command {
            ChatCommand::Add { text } | ChatCommand::Replace { text, .. } => {
                !text.trim().is_empty() && text.len() <= CHAT_MAX_TEXT_BYTES
            }
            ChatCommand::Remove { .. } => true,
        }
    }
}
