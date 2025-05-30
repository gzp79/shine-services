use crate::web::{
    extracts::ClientFingerprint,
    responses::{ErrorResponse, ProblemConfig},
    session::{serde_session_key, SessionKey, UserSessionCacheReader, UserSessionError},
};
use axum::{extract::FromRequestParts, http::request::Parts, Extension, RequestPartsExt};
use axum_extra::extract::SignedCookieJar;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_infra_macros::RedisJsonValue;
use std::{ops, sync::Arc};
use uuid::Uuid;

/// Information about the current user.
#[derive(Clone, Debug, Hash, Serialize, Deserialize, RedisJsonValue)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUser {
    pub user_id: Uuid,
    #[serde(with = "serde_session_key")]
    pub key: SessionKey,
    pub session_start: DateTime<Utc>,
    pub session_end: DateTime<Utc>,
    pub name: String,
    pub is_email_confirmed: bool,
    pub is_linked: bool,
    pub roles: Vec<String>,
    pub fingerprint: String,
}

/// The session cookie data.
#[derive(Clone, Debug, Hash, Serialize, Deserialize, RedisJsonValue)]
pub struct SessionCookie {
    #[serde(rename = "u")]
    pub user_id: Uuid,
    #[serde(rename = "key", with = "serde_session_key")]
    pub key: SessionKey,
    #[serde(rename = "fp")]
    pub fingerprint: String,
}

/// Extractor for the CurrentUser used to extract the user data from the request.
/// Cookie stores only the key with some minimal data, the session data is fetched from
/// the redis cache during the extraction.
pub struct CheckedCurrentUser(CurrentUser);

impl CheckedCurrentUser {
    pub fn into_user(self) -> CurrentUser {
        self.0
    }
}

impl From<CheckedCurrentUser> for CurrentUser {
    fn from(value: CheckedCurrentUser) -> Self {
        value.0
    }
}

impl ops::Deref for CheckedCurrentUser {
    type Target = CurrentUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ops::DerefMut for CheckedCurrentUser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S> FromRequestParts<S> for CheckedCurrentUser
where
    S: Send + Sync,
{
    type Rejection = ErrorResponse<UserSessionError>;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(problem_config) = parts
            .extract::<Extension<ProblemConfig>>()
            .await
            .expect("Missing ProblemConfig extension");
        let Extension(session_cache) = parts
            .extract::<Extension<Arc<UserSessionCacheReader>>>()
            .await
            .expect("Missing UserSessionCacheReader extension");
        let fingerprint = parts.extract::<ClientFingerprint>().await.map_err(|err| {
            ErrorResponse::new(&problem_config, UserSessionError::from(err.problem))
        })?;

        let jar =
            SignedCookieJar::from_headers(&parts.headers, session_cache.cookie_secret().clone());
        let session_cookie = jar
            .get(session_cache.cookie_name())
            .and_then(|cookie| serde_json::from_str::<SessionCookie>(cookie.value()).ok())
            .ok_or_else(|| {
                ErrorResponse::new(&problem_config, UserSessionError::Unauthenticated)
            })?;

        log::debug!("Checking fingerprint: {:?}", session_cookie.user_id);
        if session_cookie.fingerprint != fingerprint.as_str() {
            return Err(ErrorResponse::new(
                &problem_config,
                UserSessionError::SessionCompromised,
            ));
        }

        log::debug!("Finding user session: {:?}", session_cookie.user_id);
        let current_user = session_cache
            .get_current_user(session_cookie.user_id, session_cookie.key)
            .await
            .map_err(|err| ErrorResponse::new(&problem_config, err))?;
        Ok(CheckedCurrentUser(current_user))
    }
}
