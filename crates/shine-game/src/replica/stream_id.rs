use std::fmt;
use thiserror::Error as ThisError;

const MAX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, ThisError)]
pub enum StreamIdError {
    #[error("Unknown stream kind {0:?}")]
    InvalidKind(String),
    #[error("A stream identifier must be 1 to {MAX_LEN} characters of letters, digits and '_', '.', ':', '-'")]
    InvalidId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind {
    Chat,
    PixelMap,
}

impl StreamKind {
    pub fn parse(value: &str) -> Result<Self, StreamIdError> {
        match value {
            "chat" => Ok(Self::Chat),
            "pixel-map" => Ok(Self::PixelMap),
            _ => Err(StreamIdError::InvalidKind(value.to_string())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::PixelMap => "pixel-map",
        }
    }
}

impl fmt::Display for StreamKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The identity of a command stream: its fixed, known kind plus a short identifier chosen by the
/// application. Its wire form (`chat:global`) is safe to use in Redis keys; the two parts are
/// stored separately in the command log.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamId {
    kind: StreamKind,
    id: String,
}

impl StreamId {
    pub fn parse(value: &str) -> Result<Self, StreamIdError> {
        let (kind, id) = value.split_once(':').ok_or(StreamIdError::InvalidId)?;
        Self::new(StreamKind::parse(kind)?, id)
    }

    pub fn new(kind: StreamKind, id: impl Into<String>) -> Result<Self, StreamIdError> {
        let id = id.into();
        let valid = !id.is_empty()
            && id.len() <= MAX_LEN
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'));
        if valid {
            Ok(Self { kind, id })
        } else {
            Err(StreamIdError::InvalidId)
        }
    }

    pub fn kind(&self) -> StreamKind {
        self.kind
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.id)
    }
}
