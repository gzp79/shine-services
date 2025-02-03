use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum EmailSenderError {
    #[error("Invalid email address")]
    InvalidEmailAddress,
    #[error("Invalid content")]
    InvalidContent,
    #[error("Email send failed: {0}")]
    SendFailed(String),
}
