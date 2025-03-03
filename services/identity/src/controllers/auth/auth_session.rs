use crate::{app_config::AppConfig, app_state::AppState, repositories::identity::TokenKind};
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, IntoResponseParts, Response, ResponseParts},
    Extension, RequestPartsExt,
};
use axum_extra::extract::{
    cookie::{Cookie, Expiration, Key, SameSite},
    SignedCookieJar,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::{DateTime, Utc};
use rand::{rng, Rng};
use serde::{Deserialize, Serialize};
use shine_core::web::{CheckedCurrentUser, CurrentUser, WebAppConfig};
use std::{convert::Infallible, sync::Arc};
use thiserror::Error as ThisError;
use time::{Duration, OffsetDateTime};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalLoginCookie {
    // used for tracing the login flow
    #[serde(rename = "key")]
    pub key: String,
    #[serde(rename = "pv")]
    pub pkce_code_verifier: String,
    #[serde(rename = "cv")]
    pub csrf_state: String,
    #[serde(rename = "nc")]
    pub nonce: Option<String>,
    #[serde(rename = "tu")]
    pub target_url: Option<Url>,
    #[serde(rename = "eu")]
    pub error_url: Option<Url>,
    #[serde(rename = "rm")]
    pub remember_me: bool,
    // indicates if login was made to link the account to the user of the given session
    #[serde(rename = "lnk")]
    pub linked_user: Option<CurrentUser>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenCookie {
    #[serde(rename = "u")]
    pub user_id: Uuid,
    #[serde(rename = "key")]
    pub key: String,
    #[serde(rename = "e")]
    pub expire_at: DateTime<Utc>,

    /// This token is not used, only stored to revoke once the client confirmed the received new token
    #[serde(rename = "rky")]
    pub revoked_token: Option<String>,
}

#[derive(Debug, ThisError)]
pub enum AuthSessionError {
    #[error("Missing or invalid domain for application home")]
    MissingHomeDomain,
    #[error("Invalid session secret: {0}")]
    InvalidSecret(String),
    #[error("Missing domain for auth scope")]
    MissingDomain,
    #[error("Auth and web should have the same domain (without subdomains)")]
    InvalidApiDomain,
}

#[derive(Clone)]
struct CookieSettings {
    name: String,
    secret: Key,
    domain: String,
    path: String,
}

/// Layer to configure auth related cookie.
#[derive(Clone)]
pub struct AuthSessionMeta {
    session_settings: CookieSettings,
    external_login_cookie_settings: CookieSettings,
    token_cookie_settings: CookieSettings,
}

impl AuthSessionMeta {
    pub fn new(config: &WebAppConfig<AppConfig>) -> Result<Self, AuthSessionError> {
        let config_auth = &config.feature.auth;
        let config_auth_session = &config_auth.auth_session;

        let home_url = &config_auth.home_url;
        let home_domain = home_url.domain().ok_or(AuthSessionError::MissingHomeDomain)?;
        let domain = {
            let mut parts = home_domain.split('.').rev().take(2).collect::<Vec<_>>();
            parts.reverse();
            parts.join(".")
        };

        let auth_base = &config_auth.auth_base_url;
        let auth_domain = auth_base.domain().ok_or(AuthSessionError::MissingDomain)?.to_string();
        let auth_path = auth_base.path().to_string();
        if !auth_domain.ends_with(&domain) {
            log::error!("Non-matching domains, home:{home_domain}, auth: {auth_domain}, common:{domain}");
            return Err(AuthSessionError::InvalidApiDomain);
        }

        let token_cookie_settings = {
            let key = B64
                .decode(&config_auth_session.token_cookie_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: "tid".to_string(),
                secret,
                domain: auth_domain.clone(),
                path: auth_path.clone(),
            }
        };

        let session_settings = {
            let key = B64
                .decode(&config.service.session_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: "sid".to_string(),
                secret,
                domain,
                path: "/".into(),
            }
        };

        let external_login_cookie_settings = {
            let key = B64
                .decode(&config_auth_session.external_login_cookie_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: "eid".to_string(),
                secret,
                domain: auth_domain,
                path: auth_path,
            }
        };

        Ok(Self {
            session_settings,
            external_login_cookie_settings,
            token_cookie_settings,
        })
    }

    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }
}

/// Handle all auth related cookie as an atomic entity. During authorization flow
/// it ensures the consistency between the auth related cookie.
#[derive(Clone)]
pub struct AuthSession {
    meta: Arc<AuthSessionMeta>,
    access: Option<TokenCookie>,
    session: Option<CurrentUser>,
    external_login: Option<ExternalLoginCookie>,
}

impl AuthSession {
    pub fn new(
        meta: Arc<AuthSessionMeta>,
        access: Option<TokenCookie>,
        session: Option<CurrentUser>,
        external_login: Option<ExternalLoginCookie>,
    ) -> Self {
        Self {
            meta,
            access,
            session,
            external_login,
        }
    }

    pub fn user_session(&self) -> Option<&CurrentUser> {
        self.session.as_ref()
    }

    #[must_use]
    pub fn with_session(self, session: Option<CurrentUser>) -> Self {
        Self {
            meta: self.meta,
            access: self.access,
            session,
            external_login: self.external_login,
        }
    }

    /// Clear the session and revoke the session from the session store.
    #[must_use]
    pub async fn revoke_session(mut self, state: &AppState) -> Self {
        state.session_utils().revoke_opt_session(self.session.take()).await;
        self
    }

    pub fn access(&self) -> Option<&TokenCookie> {
        self.access.as_ref()
    }

    #[must_use]
    pub fn with_access(self, access: Option<TokenCookie>) -> Self {
        Self {
            meta: self.meta,
            access,
            session: self.session,
            external_login: self.external_login,
        }
    }

    /// Clear the access token and revoke the token from the token store.
    #[must_use]
    pub async fn revoke_access(mut self, state: &AppState) -> Self {
        if let Some(token_cookie) = self.access.take() {
            state
                .session_utils()
                .revoke_opt_access(TokenKind::Access, token_cookie.revoked_token)
                .await;
            state
                .session_utils()
                .revoke_access(TokenKind::Access, &token_cookie.key)
                .await;
        }
        self
    }

    #[must_use]
    pub fn with_external_login(self, external_login: Option<ExternalLoginCookie>) -> Self {
        Self {
            meta: self.meta,
            access: self.access,
            session: self.session,
            external_login,
        }
    }

    pub fn external_login(&self) -> Option<&ExternalLoginCookie> {
        self.external_login.as_ref()
    }

    #[must_use]
    pub fn cleared(self) -> Self {
        Self {
            meta: self.meta,
            access: None,
            session: None,
            external_login: None,
        }
    }
}

impl<S> FromRequestParts<S> for AuthSession
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    /// Extract component from the cookie header:
    /// - If a component is compromised, it is set to None
    /// - If there is no signature or it is not matching to the component, and empty result is returned        
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(meta) = parts
            .extract::<Extension<Arc<AuthSessionMeta>>>()
            .await
            .expect("Missing AuthSessionMeta extension");

        let mut user = parts.extract::<CheckedCurrentUser>().await.ok().map(|x| x.into_user());
        let mut external_login_cookie =
            SignedCookieJar::from_headers(&parts.headers, meta.external_login_cookie_settings.secret.clone())
                .get(&meta.external_login_cookie_settings.name)
                .and_then(|session| serde_json::from_str::<ExternalLoginCookie>(session.value()).ok());
        let token_cookie = SignedCookieJar::from_headers(&parts.headers, meta.token_cookie_settings.secret.clone())
            .get(&meta.token_cookie_settings.name)
            .and_then(|session| serde_json::from_str::<TokenCookie>(session.value()).ok());

        log::debug!(
            "Auth sessions before validation:\n  user:{:#?}\n  external_login_cookie:{:#?}\n  token_cookie:{:#?}\n",
            user,
            external_login_cookie,
            token_cookie,
        );

        // Perform cross-validation
        // - user of token is not matching the user of the session, session is deleted
        // - if linked_account of the external login is not matching the session, external login is deleted

        log::debug!("Validating cookies...");
        //todo: if let Some(uid) = user.as_ref().map(|u| u.user_id) &&
        //let Some(tid) = token_cookie.as_ref().map(|t| t.user_id) &&
        //tid != uid {

        // check if the users are matching in the session and token
        if token_cookie.is_some()
            && user.is_some()
            && token_cookie.as_ref().map(|t| t.user_id) != user.as_ref().map(|u| u.user_id)
        {
            log::info!("user session is not matching to the token, dropping user session");
            user = None;
        }

        // check if the users are matching in the session and external login
        if external_login_cookie.is_some()
            && external_login_cookie
                .as_ref()
                .and_then(|e| e.linked_user.as_ref())
                .map(|l| l.user_id)
                != user.as_ref().map(|u| u.user_id)
        {
            log::info!("external login is not matching the user session, dropping external login");
            external_login_cookie = None;
        }

        log::debug!(
            "Auth sessions after validation:\n  user:{:#?}\n  external_login_cookie:{:#?}\n  token_cookie:{:#?}\n",
            user,
            external_login_cookie,
            token_cookie,
        );

        Ok(Self::new(meta, token_cookie, user, external_login_cookie))
    }
}

fn create_jar<T: Serialize, X: Into<Expiration>>(
    settings: &CookieSettings,
    data_and_expiration: Option<(&T, X)>,
) -> SignedCookieJar {
    let mut cookie = if let Some((data, expiration)) = data_and_expiration {
        let raw_data = serde_json::to_string(&data).expect("Failed to serialize user");
        let mut cookie = Cookie::new(settings.name.clone(), raw_data);
        cookie.set_expires(expiration);
        cookie
    } else {
        // for deleted cookie to avoid exposing the key (there could be rainbow tables for empty hmac encoding),
        // let's encode some dummy nonce
        #[derive(Serialize)]
        struct Dummy {
            n: String,
        }

        let nonce: Vec<u8> = (0..16).map(|_| rng().random::<u8>()).collect();
        let nonce = B64.encode(nonce);
        let raw_data = serde_json::to_string(&Dummy { n: nonce }).expect("Failed to serialize user");
        let mut cookie = Cookie::new(settings.name.to_string(), raw_data);
        cookie.set_expires(OffsetDateTime::now_utc() - Duration::days(1));
        cookie
    };

    cookie.set_secure(true);
    cookie.set_domain(settings.domain.clone());
    cookie.set_path(settings.path.clone());
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path(settings.path.clone());
    SignedCookieJar::new(settings.secret.clone()).add(cookie)
}

impl IntoResponseParts for AuthSession {
    type Error = Infallible;

    /// Create set cookie header:
    /// - If a component is None, the cookie is deleted on the client side
    /// - If there is no component, all the cookies are deleted (including signature).
    fn into_response_parts(self, res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        let Self {
            meta,
            session: user_session,
            external_login: external_login_cookie,
            access: token_cookie,
        } = self;
        log::debug!(
            "Auth sessions set headers:\n  user:{:#?}\n  external_login_cookie:{:#?}\n  token_cookie:{:#?}",
            user_session,
            external_login_cookie,
            token_cookie,
        );

        let session = create_jar(
            &meta.session_settings,
            user_session.as_ref().map(|d| (d, Expiration::Session)),
        );

        let external_login_cookie = create_jar(
            &meta.external_login_cookie_settings,
            external_login_cookie.as_ref().map(|d| (d, Expiration::Session)),
        );

        let token_cookie = create_jar(
            &meta.token_cookie_settings,
            token_cookie.as_ref().map(|d| {
                let naive_time = d.expire_at.naive_utc();
                let naive_time = OffsetDateTime::from_unix_timestamp(naive_time.and_utc().timestamp()).unwrap();
                // disable cookie a few minutes before the token expiration
                let token_expiration = naive_time - Duration::minutes(5);
                (d, token_expiration)
            }),
        );

        Ok((session, external_login_cookie, token_cookie)
            .into_response_parts(res)
            .unwrap())
    }
}

impl IntoResponse for AuthSession {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}
