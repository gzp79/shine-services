use crate::math::temporal::ValueError;
use thiserror::Error as ThisError;

// In camera_rig/rig_error.rs
#[derive(Debug, ThisError)]
pub enum RigError {
    #[error(transparent)]
    ValueError(#[from] ValueError),
}
