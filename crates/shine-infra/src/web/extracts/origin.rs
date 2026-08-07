use crate::web::responses::{ErrorResponse, Problem, ProblemConfig};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
    Extension, RequestPartsExt,
};
use reqwest::StatusCode;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum OriginError {
    #[error("Missing origin")]
    MissingOrigin,
}

impl From<OriginError> for Problem {
    fn from(err: OriginError) -> Self {
        match err {
            OriginError::MissingOrigin => Problem::new(StatusCode::BAD_REQUEST, "missing_origin"),
        }
    }
}

/// Effective request origin extracted from the Origin header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin(pub String);

impl Origin {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<S> FromRequestParts<S> for Origin
where
    S: Send + Sync,
{
    type Rejection = ErrorResponse<OriginError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");

        let origin = parts
            .headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ErrorResponse::new(&problem_config, OriginError::MissingOrigin))?;

        Ok(Self(origin.to_string()))
    }
}
