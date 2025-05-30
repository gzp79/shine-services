use crate::web::responses::{ErrorResponse, Problem, ProblemConfig};
use axum::{extract::FromRequestParts, http::request::Parts, Extension, RequestPartsExt};
use axum_extra::{headers::UserAgent, TypedHeader};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use reqwest::StatusCode;
use ring::digest::{self, Context};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum ClientFingerprintError {
    #[error("Missing user agent")]
    MissingUserAgent,
}

impl From<ClientFingerprintError> for Problem {
    fn from(err: ClientFingerprintError) -> Self {
        match err {
            ClientFingerprintError::MissingUserAgent => {
                Problem::new(StatusCode::BAD_REQUEST, "missing_user_agent")
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
/// Some fingerprinting of the client site to detect token stealing.
pub struct ClientFingerprint(String);

impl ClientFingerprint {
    pub fn unknown() -> Self {
        Self("unknown".to_string())
    }

    pub fn from_agent(agent: String) -> Result<Self, ClientFingerprintError> {
        if agent.is_empty() {
            Err(ClientFingerprintError::MissingUserAgent)
        } else {
            let mut context = Context::new(&digest::SHA256);
            context.update(agent.as_bytes());
            let hash = B64.encode(context.finish().as_ref());
            Ok(Self(hash))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl<S> FromRequestParts<S> for ClientFingerprint
where
    S: Send + Sync,
{
    type Rejection = ErrorResponse<ClientFingerprintError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");

        let agent = parts
            .extract::<TypedHeader<UserAgent>>()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();

        if agent.is_empty() {
            Ok(ClientFingerprint::unknown())
        } else {
            ClientFingerprint::from_agent(agent)
                .map_err(|err| ErrorResponse::new(&problem_config, err))
        }
    }
}
