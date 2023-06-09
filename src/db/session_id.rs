use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
pub enum SessionIdError {
    #[error(transparent)]
    FromHexError(#[from] hex::FromHexError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionId([u8; 32]);

impl SessionId {
    pub fn from_raw(raw: [u8; 32]) -> Self {
        SessionId(raw)
    }

    /// Create from a single string usually used on the API.
    pub fn from_session_id(session_id: &str) -> Result<Self, SessionIdError> {
        let mut id = [0; 32];
        hex::decode_to_slice(session_id, &mut id)?;
        Ok(SessionId(id))
    }

    /// Convert to a single string usually used on the API.
    pub fn to_session_id(&self) -> String {
        hex::encode(self.0)
    }

    /// Get the userid from the session.
    pub fn to_user_id(&self) -> Uuid {
        Uuid::from_slice(&self.0[..16]).unwrap()
    }

    /// Generate a unique session key.
    pub fn to_token(&self) -> String {
        hex::encode(&self.0[16..])
    }
}

pub mod serde_session_id {
    use super::SessionId;
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &SessionId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Custom serialization logic
        // Implement how you want to serialize the value
        serializer.serialize_str(&value.to_session_id())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SessionId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = String::deserialize(deserializer)?;
        SessionId::from_session_id(&id).map_err(de::Error::custom)
    }
}

pub mod serde_opt_session_id {
    use super::{serde_session_id, SessionId};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(value: &Option<SessionId>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Helper<'a>(#[serde(with = "serde_session_id")] &'a SessionId);

        value.as_ref().map(Helper).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SessionId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper(#[serde(with = "serde_session_id")] SessionId);

        let helper = Option::deserialize(deserializer)?;
        Ok(helper.map(|Helper(external)| external))
    }
}
