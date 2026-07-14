use crate::web::responses::{ErrorResponse, Problem, ProblemConfig};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
    Extension, RequestPartsExt,
};
use reqwest::StatusCode;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum TargetHostError {
    #[error("Missing target host")]
    MissingHost,
}

impl From<TargetHostError> for Problem {
    fn from(err: TargetHostError) -> Self {
        match err {
            TargetHostError::MissingHost => Problem::new(StatusCode::BAD_REQUEST, "missing_target_host"),
        }
    }
}

/// Effective target host resolved from forwarded headers or the direct Host header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetHost(pub String);

impl TargetHost {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn extract_target_host(headers: &HeaderMap) -> Option<&str> {
        headers
            .get("x-forwarded-host")
            .and_then(Self::header_value_to_host)
            .or_else(|| headers.get("forwarded").and_then(Self::header_value_to_forwarded_host))
            .or_else(|| {
                headers
                    .get(axum::http::header::HOST)
                    .and_then(|value| value.to_str().ok())
            })
    }

    fn header_value_to_host(value: &axum::http::HeaderValue) -> Option<&str> {
        value
            .to_str()
            .ok()
            .and_then(|raw| raw.split(',').next().map(str::trim))
            .filter(|host| !host.is_empty())
    }

    fn header_value_to_forwarded_host(value: &axum::http::HeaderValue) -> Option<&str> {
        let raw = value.to_str().ok()?;
        for entry in raw.split(',') {
            for item in entry.split(';') {
                if let Some((key, value)) = item.trim().split_once('=') {
                    if key.eq_ignore_ascii_case("host") {
                        let host = value.trim().trim_matches('"');
                        if !host.is_empty() {
                            return Some(host);
                        }
                    }
                }
            }
        }
        None
    }
}

impl<S> FromRequestParts<S> for TargetHost
where
    S: Send + Sync,
{
    type Rejection = ErrorResponse<TargetHostError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");

        let headers = HeaderMap::from_iter(parts.headers.iter().map(|(name, value)| (name.clone(), value.clone())));
        let target_host = Self::extract_target_host(&headers)
            .ok_or_else(|| ErrorResponse::new(&problem_config, TargetHostError::MissingHost))?;
        Ok(Self(target_host.to_string()))
    }
}
