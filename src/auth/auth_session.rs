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
use shine_service::service::CurrentUser;
use std::{convert::Infallible, sync::Arc};
use thiserror::Error as ThisError;
use time::{Duration, OffsetDateTime};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::auth) struct ExternalLogin {
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
pub(in crate::auth) struct TokenLogin {
    #[serde(rename = "u")]
    pub user_id: Uuid,
    #[serde(rename = "t")]
    pub token: String,
    #[serde(rename = "e")]
    pub expires: DateTime<Utc>,
}

#[derive(Debug, ThisError)]
pub(in crate::auth) enum AuthSessionError {
    #[error("Missing or invalid domain for application home")]
    MissingHomeDomain,
    #[error("Invalid session secret: {0}")]
    InvalidSecret(String),
    #[error("Missing domain for auth scope")]
    MissingDomain,
    #[error("Auth api domain shall be a subdomain of the application")]
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
    user: CookieSettings,
    external_login: CookieSettings,
    token_login: CookieSettings,
}

impl AuthSessionMeta {
    pub fn new(home_url: Url, auth_base: Url, config: &AuthSessionConfig) -> Result<Self, AuthSessionError> {
        let cookie_name_suffix = config.cookie_name_suffix.as_deref().unwrap_or_default();
        let home_domain = home_url.domain().ok_or(AuthSessionError::MissingHomeDomain)?;
        let auth_domain = auth_base.domain().ok_or(AuthSessionError::MissingDomain)?.to_string();
        let auth_path = auth_base.path().to_string();
        if !auth_domain.ends_with(home_domain) {
            return Err(AuthSessionError::InvalidApiDomain);
        }

        let token_login = {
            let key = B64
                .decode(&config.token_login_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: format!("tid{}", cookie_name_suffix),
                secret,
                domain: auth_domain.clone(),
                path: auth_path.clone(),
            }
        };

        let user = {
            let key = B64
                .decode(&config.session_secret)
                .map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            let secret = Key::try_from(&key[..]).map_err(|err| AuthSessionError::InvalidSecret(format!("{err}")))?;
            CookieSettings {
                name: format!("sid{}", cookie_name_suffix),
                secret,
                domain: home_domain.into(),
                path: "/".into(),
            }
        };

        let external_login = {
            let key = B64
                .decode(&config.external_login_secret)
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
            user,
            external_login,
            token_login,
        })
    }

    pub fn into_layer(self) -> Extension<Arc<Self>> {
        Extension(Arc::new(self))
    }
}

/// Handle all auth related cookie as an atomic entity. During authorization flow this
/// structure the consistency between the auth related cookie.
pub(in crate::auth) struct AuthSession {
    meta: Arc<AuthSessionMeta>,
    pub user: Option<CurrentUser>,
    pub external_login: Option<ExternalLogin>,
    pub token_login: Option<TokenLogin>,
}

impl AuthSession {
    fn new(
        meta: Arc<AuthSessionMeta>,
        user: Option<CurrentUser>,
        external_login: Option<ExternalLogin>,
        token_login: Option<TokenLogin>,
    ) -> Self {
        Self {
            meta,
            user,
            external_login,
            token_login,
        }
    }

    /// Clear all the components.
    pub fn clear(&mut self) {
        self.user.take();
        self.external_login.take();
        self.token_login.take();
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

        let mut user = SignedCookieJar::from_headers(&parts.headers, meta.user.secret.clone())
            .get(&meta.user.name)
            .and_then(|session| serde_json::from_str::<CurrentUser>(session.value()).ok());
        let mut external_login = SignedCookieJar::from_headers(&parts.headers, meta.external_login.secret.clone())
            .get(&meta.external_login.name)
            .and_then(|session| serde_json::from_str::<ExternalLogin>(session.value()).ok());
        let mut token_login = SignedCookieJar::from_headers(&parts.headers, meta.token_login.secret.clone())
            .get(&meta.token_login.name)
            .and_then(|session| serde_json::from_str::<TokenLogin>(session.value()).ok());

        log::debug!(
            "Auth sessions before validation:\n  user:{:#?}\n  external_login:{:#?}\n  token_login:{:#?}\n",
            user,
            external_login,
            token_login,
        );

        // Perform validation on each cookie independently
        //todo: if let Some(t) = token_login.as_ref().map(|t| t.expires) && t < Utc::now() {
        if token_login.as_ref().map(|t| t.expires < Utc::now()).unwrap_or(true) {
            // It should have been done by the browser, but never trust the clients.
            log::info!("token expired, dropping token");
            token_login = None;
        }

        //todo: check session expiration too to be consistent to redis ttl
        // this check should allow a bit shorter lifetime (ex 30 sec) as by the time redis expires the cookie is invalidated for sure
        // also in user info there should be some info info: instead of sessionLength
        // we should have some sessionTTL (10min) (also slightly reduced b/c of time drift)
        // For testing it would be good to directly alter the DB from the test framework as that could simplify it a lot.

        // Perform cross-validation
        // - user of token is not matching the user of the session, session is deleted
        // - if linked_account of the external login is not matching the session, external login is deleted

        log::debug!("Validating sessions...");
        /*todo: if let Some(uid) = user.as_ref().map(|u| u.user_id) &&
        let Some(tid) = token_login.as_ref().map(|t| t.user_id) &&
        tid != uid {*/
        if token_login.is_some()
            && user.is_some()
            && token_login.as_ref().map(|t| t.user_id) != user.as_ref().map(|u| u.user_id)
        {
            log::info!("user session is not matching to the token, dropping user session");
            user = None;
        }
        if external_login
            .as_ref()
            .and_then(|e| e.linked_user.as_ref())
            .map(|l| l.user_id)
            != user.as_ref().map(|u| u.user_id)
        {
            log::info!("external login is not matching the user session, dropping external login");
            external_login = None;
        }

        log::debug!(
            "Auth sessions after validation:\n  user:{:#?}\n  external_login:{:#?}\n  token_login:{:#?}\n",
            user,
            external_login,
            token_login,
        );

        Ok(Self::new(meta, user, external_login, token_login))
    }
}

fn create_jar<T: Serialize, X: Into<Expiration>>(
    settings: &CookieSettings,
    data: &Option<T>,
    expiration: X,
) -> SignedCookieJar {
    let mut cookie = if let Some(data) = data {
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
            user,
            external_login,
            token_login,
        } = self;
        log::debug!(
            "Auth sessions set headers:\n  user:{:#?}\n  external_login:{:#?}\n  token_login:{:#?}",
            user,
            external_login,
            token_login,
        );

        let token_expiration = {
            let time = token_login.as_ref().map(|t| t.expires).unwrap_or(Utc::now());
            let naive_time = time.naive_utc();
            OffsetDateTime::from_unix_timestamp(naive_time.timestamp()).unwrap()
        };

        let user = create_jar(&meta.user, &user, Expiration::Session);
        let external_login = create_jar(&meta.external_login, &external_login, Expiration::Session);
        let token_login = create_jar(&meta.token_login, &token_login, token_expiration);

        Ok((user, external_login, token_login).into_response_parts(res).unwrap())
    }
}

impl IntoResponse for AuthSession {
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}
