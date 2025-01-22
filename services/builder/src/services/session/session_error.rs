use axum::http::StatusCode;
use shine_core::web::{IntoProblem, Problem, ProblemConfig};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum SessionError {
    #[error("User already connected")]
    UserAlreadyConnected,
}

impl IntoProblem for SessionError {
    fn into_problem(self, _config: &ProblemConfig) -> Problem {
        match self {
            SessionError::UserAlreadyConnected => Problem::new(StatusCode::CONFLICT, "User already connected"),
        }
    }
}
