use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum InputError {
    #[error("Input is not compatible with the current bound pipelines")]
    IncompatibleState,
}
