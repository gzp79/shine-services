use crate::math::temporal::ValueKind;
use thiserror::Error as ThisError;

// In math/temporal/parameter_error.rs
#[derive(Debug, ThisError)]
pub enum ValueError {
    #[error("Parameter name conflict: {0}")]
    DuplicateParameter(String),
    #[error("Unknown parameter: {0}")]
    UnknownParameter(String),
    #[error("Type mismatch: expected {expected:?}, found {found:?}")]
    TypeMismatch { expected: ValueKind, found: ValueKind },
}
