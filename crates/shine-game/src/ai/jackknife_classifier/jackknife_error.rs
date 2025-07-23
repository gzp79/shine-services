use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum JackknifeError {
    #[error("Configuration mismatch")]
    ConfigMismatch,
}
