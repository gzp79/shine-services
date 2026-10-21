use std::fmt;
use thiserror::Error as ThisError;

const MAX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, ThisError)]
#[error("A stream id is 1 to {MAX_LEN} characters of letters, digits and '_', '.', ':', '-'")]
pub struct StreamIdError;

/// The id of a command stream: a short string chosen by the stream kind or the application
/// (`chat-global`). Safe to use in database columns and Redis keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamId(String);

impl StreamId {
    pub fn parse(value: &str) -> Result<Self, StreamIdError> {
        let valid = !value.is_empty()
            && value.len() <= MAX_LEN
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'));
        if valid {
            Ok(Self(value.to_string()))
        } else {
            Err(StreamIdError)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
