use crate::auth::AuthSessionConfig;
use async_trait::async_trait;
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
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use shine_service::service::{CheckedCurrentUser, CurrentUser};
use std::{convert::Infallible, sync::Arc};
use thiserror::Error as ThisError;
use time::{Duration, OffsetDateTime};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::auth) struct ExternalLoginCookie {
    #[serde(rename = "pv")]
    pub pkce_code_verifier: String,
    #[serde(rename = "cv")]
    pub csrf_state: String,
    #[serde(rename = "n")]
    pub nonce: Option<String>,
    #[serde(rename = "t")]
    pub target_url: Option<Url>,
    #[serde(rename = "et")]
    pub error_url: Option<Url>,
    pub remember_me: bool,
    // indicates if login was made to link the account to the user of the given session
    #[serde(rename = "l")]
    pub linked_user: Option<CurrentUser>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::auth) struct TokenCookie {
    #[serde(rename = "u")]
    pub user_id: Uuid,
    #[serde(rename = "t")]
    pub token: String,
    #[serde(rename = "e")]
    pub expire_at: DateTime<Utc>,

    /// This token is not used, only stored to revoke once the client confirmed the received new token
    #[serde(rename = "rt")]
    pub revoked_token: Option<String>,
}

#[derive(Debug, ThisError)]
pub(in crate::auth) enum AuthSessionError {
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
pub(in crate::auth) struct AuthSessionMeta {
    session_settings: CookieSettings,
    external_login_cookie_settings: CookieSettings,
    token_cookie_settings: CookieSettings,
}

impl AuthSessionMeta {
    pub fn new(home_url: Url, auth_base: Url, config: &AuthSessionConfig) -> Result<Self, AuthSessionError> {
        let cookie_name_suffix = config.cookie_name_suffix.as_deref().unwrap_or_default();
        let home_domain = home_url.domain().ok_or(AuthSessionError::MissingHomeDomain)?;
        let domain = {
            let mut parts = home_domain.split('.').rev().take(2).collect::<Vec<_>>();
            parts.reverse();
            parts.join(".")
        };
        let auth_domain = auth_base.domain().ok_or(AuthSessionError::MissingDomain)?.to_string();
        let auth_path = auth_base.path().to_string();
        if !auth_domain.ends_with(&domain) {
            log::error!("Non-matching domains, home:{home_domain}, auth: {auth_domain}, common:{domain}");
            return Err(AuthSessionError::InvalidApiDomain);
        }

        let token_cookie_settings = {
            let key = B64
                .decode(&config.token_cookie_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: format!("tid{}", cookie_name_suffix),
                secret,
                domain: auth_domain.clone(),
                path: auth_path.clone(),
            }
        };

        let session_settings = {
            let key = B64
                .decode(&config.session_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: format!("sid{}", cookie_name_suffix),
                secret,
                domain,
                path: "/".into(),
            }
        };

        let external_login_cookie_settings = {
            let key = B64
                .decode(&config.external_login_cookie_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: format!("eid{}", cookie_name_suffix),
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

/// Handle all auth related cookie as an atomic entity. During authorization flow this
/// structure the consistency between the auth related cookie.
#[derive(Clone)]
pub(in crate::auth) struct AuthSession {
    pub meta: Arc<AuthSessionMeta>,
    pub token_cookie: Option<TokenCookie>,
    pub user_session: Option<CurrentUser>,
    pub external_login_cookie: Option<ExternalLoginCookie>,
}

impl AuthSession {
    pub fn new(
        meta: Arc<AuthSessionMeta>,
        token_cookie: Option<TokenCookie>,
        user: Option<CurrentUser>,
        external_login_cookie: Option<ExternalLoginCookie>,
    ) -> Self {
        Self {
            meta,
            token_cookie,
            user_session: user,
            external_login_cookie,
        }
    }

    /// Clear all the components.
    pub fn clear(&mut self) {
        self.token_cookie.take();
        self.user_session.take();
        self.external_login_cookie.take();
    }
}

#[async_trait]
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
        let nonce: Vec<u8> = (0..16).map(|_| thread_rng().gen::<u8>()).collect();
        let nonce = B64.encode(nonce);
        let mut cookie = Cookie::new(settings.name.to_string(), nonce);
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
            user_session,
            external_login_cookie,
            token_cookie,
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
                // disable cookie a few minutes before the token expiration
                let token_expiration =
                    OffsetDateTime::from_unix_timestamp(naive_time.timestamp()).unwrap() - Duration::minutes(5);
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
