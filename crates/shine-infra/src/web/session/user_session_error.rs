use crate::{
    db::RedisConnectionError,
    web::{extracts::ClientFingerprintError, responses::Problem},
};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum UserSessionError {
    #[error("Missing session info")]
    Unauthenticated,
    #[error("Invalid session secret")]
    InvalidSecret(String),
    #[error("Invalid time to live")]
    InvalidTtl(String),
    #[error("Session expired")]
    SessionExpired,
    #[error("Fingerprint error")]
    ClientFingerprintError(#[from] ClientFingerprintError),
    #[error("Session is compromised")]
    SessionCompromised,

    #[error("Failed to get redis connection")]
    RedisPoolError(#[source] RedisConnectionError),
    #[error("Redis error")]
    RedisError(#[from] redis::RedisError),
}

impl From<UserSessionError> for Problem {
    fn from(value: UserSessionError) -> Self {
        match value {
            UserSessionError::Unauthenticated => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("unauthenticated"),
            UserSessionError::InvalidSecret(_) => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("invalidSecret"),
            UserSessionError::InvalidTtl(_) => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("invalidTtl"),
            UserSessionError::SessionExpired => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("sessionExpired"),
            UserSessionError::ClientFingerprintError(_) => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("clientFingerprintError"),
            UserSessionError::SessionCompromised => Problem::unauthorized()
                .with_detail(value.to_string())
                .with_sensitive("sessionCompromised"),

            _ => Problem::internal_error()
                .with_detail(value.to_string())
                .with_sensitive_dbg(value),
        }
    }
}
