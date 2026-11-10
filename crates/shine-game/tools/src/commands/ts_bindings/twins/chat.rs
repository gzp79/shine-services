use postcard_bindgen::PostcardBindings;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PostcardBindings)]
pub struct ChatMessage {
    pub id: Vec<u8>,
    pub user_id: Vec<u8>,
    pub text: String,
    pub edited: bool,
}

#[derive(Serialize, Deserialize, PostcardBindings)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize, PostcardBindings)]
pub enum ChatCommand {
    Add { text: String },
    Remove { id: Vec<u8> },
    Replace { id: Vec<u8>, text: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::ts_bindings::twins::tests::assert_twin;
    use shine_game::replica;
    use uuid::Uuid;

    fn message(n: u128, text: &str, edited: bool) -> replica::ChatMessage {
        replica::ChatMessage {
            id: Uuid::from_u128(n),
            user_id: Uuid::from_u128(n + 1000),
            text: text.to_string(),
            edited,
        }
    }

    #[test]
    fn chat_message_matches_twin() {
        assert_twin::<_, ChatMessage>(&message(1, "héllo ✓", true));
    }

    #[test]
    fn chat_command_matches_twin() {
        let id = Uuid::from_u128(7);
        assert_twin::<_, ChatCommand>(&replica::ChatCommand::Add { text: "héllo ✓".into() });
        assert_twin::<_, ChatCommand>(&replica::ChatCommand::Remove { id });
        assert_twin::<_, ChatCommand>(&replica::ChatCommand::Replace { id, text: "again".into() });
    }

    #[test]
    fn chat_state_matches_twin() {
        assert_twin::<_, ChatState>(&replica::ChatState::default());

        let state = replica::ChatState {
            messages: [message(1, "first", false), message(2, "second", true)].into(),
        };
        assert_twin::<_, ChatState>(&state);
    }
}
