use ring::{digest, rand::SecureRandom};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum SessionKeyError {
    #[error(transparent)]
    FromHexError(#[from] hex::FromHexError),
    #[error("Failed to generate session key: {0}")]
    KeyError(String),
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionKey([u8; 16]);

impl std::fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SessionKey").field(&self.fingerprint()).finish()
    }
}

impl SessionKey {
    pub fn new_random(random: &dyn SecureRandom) -> Result<Self, SessionKeyError> {
        let mut raw = [0_u8; 16];
        random
            .fill(&mut raw)
            .map_err(|err| SessionKeyError::KeyError(format!("{err:#?}")))?;
        Ok(Self(raw))
    }

    /// Create from a single string usually used on the API.
    pub fn from_hex(hex_key: &str) -> Result<Self, SessionKeyError> {
        let mut raw = [0_u8; 16];
        hex::decode_to_slice(hex_key, &mut raw)?;
        Ok(Self(raw))
    }

    /// Generate a unique session key.
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    /// Canonical one-way hash of the key: full SHA-256, hex-encoded. This is the identifier used to
    /// key sessions in storage — never store or transmit the raw key, only this hash.
    pub fn key_hash(&self) -> String {
        hex::encode(digest::digest(&digest::SHA256, &self.0))
    }

    /// Short prefix of [`key_hash`] for log correlation: enough to identify a key across log lines
    /// (and to match it against the stored session hash) without revealing any secret bytes.
    pub fn fingerprint(&self) -> String {
        let mut hash = self.key_hash();
        hash.truncate(8);
        hash
    }

    /// Return the raw key as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

pub mod serde_session_key {
    use super::SessionKey;
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &SessionKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Custom serialization logic
        // Implement how you want to serialize the value
        serializer.serialize_str(&value.to_hex())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SessionKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        SessionKey::from_hex(&raw).map_err(de::Error::custom)
    }
}
