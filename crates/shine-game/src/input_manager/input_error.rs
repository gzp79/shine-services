use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum InputError {
    #[error("Action is not bound to any input pipeline")]
    ActionNotBound,
    #[error("Input is not compatible with the current bound pipelines value")]
    IncompatibleValue,
}
