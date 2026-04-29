use thiserror::Error as ThisError;

/// Errors for quadrangulation construction and validation.
/// These errors are intended for human consumption and debugging.
#[derive(Debug, ThisError, PartialEq, Eq)]
pub enum QuadError {
    /// Input validation error during construction (e.g., form_poly)
    #[error("Input error: {0}")]
    Input(String),

    /// Topological structure error detected during validation
    #[error("Topology error: {0}")]
    Topology(String),

    /// Geometric constraint error detected during validation
    #[error("Geometry error: {0}")]
    Geometry(String),
}
