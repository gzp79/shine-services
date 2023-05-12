use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct SessionId(pub String);

#[derive(Clone)]
pub struct SessionManager {}

impl SessionManager {
    pub fn new() -> SessionManager {
        SessionManager {}
    }

    pub fn create(&self, user_id: &Uuid) -> SessionId {
        SessionId(format!(
            "{}-{}",
            user_id.as_hyphenated(),
            Uuid::new_v4().as_hyphenated()
        ))
    }
}
