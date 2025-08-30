use crate::camera_rig::ValueKind;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum RigError {
    #[error("Parameter name conflict: {0}")]
    DuplicateParameter(String),
    #[error("Unknown parameter: {0}")]
    UnknownParameter(String),
    #[error("Type mismatch: expected {expected:?}, found {found:?}")]
    TypeMismatch { expected: ValueKind, found: ValueKind },
    #[error("Type mismatch for update function: expected {expected:?}, found {found:?}")]
    UpdateTypeMismatch { expected: String, found: String },
}
