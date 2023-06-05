use crate::{
    app_session::{AppSession, ExternalLoginData, ExternalLoginSession, SessionData},
    db::{DBError, ExternalLogin, IdentityError, IdentityManager, SessionManager},
    utils::generate_name,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Extension, Router,
};
use chrono::Duration;
use oauth2::{
    reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenUrl,
};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreJsonWebKeySet, CoreProviderMetadata},
    IssuerUrl, Nonce, TokenResponse, UserInfoUrl,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tera::Tera;
use thiserror::Error as ThisError;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCEndpoints {
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OIDCConfig {
    pub discovery_url: Option<String>,
    pub endpoints: Option<OIDCEndpoints>,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub redirect_url: String,
}

#[derive(Debug, ThisError)]
enum OIDCError {
    #[error("Session cookie was missing or corrupted")]
    MissingSession,
    #[error("Cross Server did not return an ID token")]
    InvalidCsrfState,
    #[error("Session and external login cookies are not matching")]
    InconsistentSession,
    #[error("Failed to exchange authorization code to access token: {0}")]
    FailedTokenExchange(String),
    #[error("Cross-Site Request Forgery (Csrf) check failed")]
    MissingIdToken,
    #[error("Failed to verify id token: {0}")]
    FailedIdVerification(String),

    //#[error(transparent)]
    //Config(#[from] DBError),
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error(transparent)]
    TeraError(#[from] tera::Error),
}

impl IntoResponse for OIDCError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            OIDCError::MissingSession => StatusCode::BAD_REQUEST,
            OIDCError::InconsistentSession => StatusCode::BAD_REQUEST,
            OIDCError::InvalidCsrfState => StatusCode::BAD_REQUEST,
            OIDCError::FailedTokenExchange(_) => StatusCode::BAD_REQUEST,
            OIDCError::MissingIdToken => StatusCode::BAD_REQUEST,
            OIDCError::FailedIdVerification(_) => StatusCode::BAD_REQUEST,
            OIDCError::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OIDCError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

struct ServiceState {
    provider: String,
    client: CoreClient,
    identity_manager: IdentityManager,
    session_manager: SessionManager,
    default_redirect_url: String,
}
type Service = Arc<ServiceState>;

async fn create_user(State(service): State<Service>) -> impl IntoResponse {
    //let user_id = Uuid::parse_str("a2c0c1eb-d2d1-41f1-a293-7d54b72ead1e").unwrap();
    let user_id = Uuid::new_v4();
    //let name = "name";
    let name = Uuid::new_v4().as_hyphenated().to_string();
    let res = service.identity_manager.create_user(user_id, &name, None, None).await;
    log::info!("{:?}", res);
}

#[derive(Deserialize)]
struct LoginRequest {
    redirect: Option<String>,
    allow_link: Option<bool>,
}

async fn openid_connect_login(
    Extension(tera): Extension<Arc<Tera>>,
    State(service): State<Service>,
    Query(query): Query<LoginRequest>,
    mut session: AppSession,
    mut external_login_session: ExternalLoginSession,
) -> Result<impl IntoResponse, OIDCError> {
    // if this is not a link-account request, clear the old sessions and perform a simple login
    if !query.allow_link.unwrap_or(false) {
        let _ = session.take();
        let _ = external_login_session.take();
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let scopes = ["openid", "email", "profile"];
    let (authorize_url, csrf_state, nonce) = service
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scopes(scopes.into_iter().map(|s| Scope::new(s.to_string())))
        .set_pkce_challenge(pkce_code_challenge)
        .set_max_age(Duration::minutes(30).to_std().unwrap())
        .add_prompt(CoreAuthPrompt::Login)
        .url();

    external_login_session.set(ExternalLoginData::OIDCLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: nonce.secret().to_owned(),
        redirect_url: query.redirect,
        link_session_id: session.as_ref().map(|s| s.session_id.clone()),
    });

    log::info!("session: {session:?}");
    log::info!("external_login: {external_login_session:?}");

    //Return an auto-redirect page that stores cookie before redirecting the user to the authorize_url.
    // In older browser with a simple StatusCode::FOUND response, no cookie headers could be sent to the client.
    let mut context = tera::Context::new();
    context.insert("title", "Redirecting to external login");
    context.insert("target", &service.provider);
    context.insert("redirect_url", &authorize_url.to_string());
    let html = Html(tera.render("redirect.html", &context)?);

    Ok((external_login_session, session, html))
}

#[derive(Deserialize)]
struct AuthRequest {
    code: String,
    state: String,
    //scope: String,
}

async fn openid_connect_auth(
    State(service): State<Service>,
    Extension(tera): Extension<Arc<Tera>>,
    Query(query): Query<AuthRequest>,
    mut session: AppSession,
    mut external_login_session: ExternalLoginSession,
) -> Result<Response, OIDCError> {
    log::info!("session: {session:?}");
    log::info!("external_login: {external_login_session:?}");

    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let external_login_data = external_login_session.take().ok_or(OIDCError::MissingSession)?;
    let (pkce_code_verifier, csrf_state, nonce, redirect_url, link_session_id) = match external_login_data {
        ExternalLoginData::OIDCLogin {
            pkce_code_verifier,
            csrf_state,
            nonce,
            redirect_url,
            link_session_id,
        } => (
            PkceCodeVerifier::new(pkce_code_verifier),
            csrf_state,
            Nonce::new(nonce),
            redirect_url,
            link_session_id,
        ),
        //_ => return Err(OIDCError::InvalidSession),
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        return Err(OIDCError::InvalidCsrfState);
    }

    // Exchange the code with a token.
    let token = service
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|err| OIDCError::FailedTokenExchange(format!("{err:?}")))?;

    let id_token = token.id_token().ok_or(OIDCError::MissingIdToken)?;
    let claims = id_token
        .claims(&service.client.id_token_verifier(), &nonce)
        .map_err(|err| OIDCError::FailedIdVerification(format!("{err}")))?;

    let mut nickname = claims
        .nickname()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str().to_owned());
    let email = claims.email().map(|n| n.as_str().to_owned());
    let provider_id = claims.subject().as_str().to_owned();
    let external_login = ExternalLogin {
        provider: service.provider.clone(),
        provider_id,
    };

    let redirect_url = redirect_url.as_ref().unwrap_or(&service.default_redirect_url);
    let mut context = tera::Context::new();
    context.insert("title", "Redirecting");
    context.insert("target", "Shine");
    context.insert("redirect_url", redirect_url);
    let html = Html(tera.render("redirect.html", &context)?);

    // find any user linked to this account

    if let Some(link_session_id) = link_session_id {
        // not a full login, but linking the user to an external provider
        //todo: if session.is_none() || session.session_id != link_session_id -> the flow was broken, sign out
        //      else if let Some(identity) find_user_by_link() {
        //         if   identity.id != session.user_id -> linked to a different user else ok
        //      } else { link account to ussr)
        // keep session as it is,
        todo!()
    } else if let Some(identity) = service.identity_manager.find_user_by_link(&external_login).await? {
        // Sign in to an existing (linked) account and redirect to the target
        let session_id = service.session_manager.create(&identity.id);
        session.set(SessionData {
            user_id: identity.id,
            session_id,
        });
        Ok((external_login_session, session, html).into_response())
    } else {
        // Create a new account, and sign in.

        let mut i = 0;

        let identity = loop {
            if i > 10 {
                return Err(OIDCError::DBError(DBError::RetryLimitReached));
            }
            i += 1;

            let user_id = Uuid::new_v4();
            let user_name = nickname.take().unwrap_or_else(generate_name);

            match service
                .identity_manager
                .create_user(user_id, &user_name, email.as_deref(), Some(&external_login))
                .await
            {
                Ok(identity) => break identity,
                Err(IdentityError::NameConflict) => continue,
                Err(IdentityError::UserIdConflict) => continue,
                Err(IdentityError::LinkConflict) => todo!(),
                Err(IdentityError::DBError(err)) => return Err(err.into()),
            };
        };

        let session_id = service.session_manager.create(&identity.id);
        session.set(SessionData {
            user_id: identity.id,
            session_id,
        });
        Ok((external_login_session, session, html).into_response())
    }
}

#[derive(Debug, ThisError)]
pub enum OIDCBuildError {
    #[error("Invalid issuer url: {0}")]
    InvalidIssuer(String),
    #[error("Invalid auth url: {0}")]
    InvalidAuth(String),
    #[error("Invalid token url: {0}")]
    InvalidToken(String),
    #[error("Invalid user info url: {0}")]
    InvalidUserInfo(String),
    #[error("Missing OpenId discover or endpoint configuration")]
    InvalidEndpoints,
    #[error("Invalid redirect url: {0}")]
    RedirectUrl(String),
    #[error("Failed to discover open id: {0}")]
    Discovery(String),
}

pub struct OIDCServiceBuilder {
    provider: String,
    default_redirect_url: String,
    client: CoreClient,
    identity_manager: IdentityManager,
    session_manager: SessionManager,
}

impl OIDCServiceBuilder {
    pub async fn new(
        provider: &str,
        config: &OIDCConfig,
        home_url: &Url,
        identity_manager: &IdentityManager,
        session_manager: &SessionManager,
    ) -> Result<Self, OIDCBuildError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let home_url = home_url.to_string();
        let redirect_url = RedirectUrl::new(config.redirect_url.to_string())
            .map_err(|err| OIDCBuildError::RedirectUrl(format!("{err}")))?;

        log::info!("Redirect url for provider {}: {:?}", provider, redirect_url);

        // Use OpenID Connect Discovery to fetch the provider metadata.

        let client = if let Some(discovery_url) = &config.discovery_url {
            let discovery_url =
                IssuerUrl::new(discovery_url.clone()).map_err(|err| OIDCBuildError::InvalidIssuer(format!("{err}")))?;
            let provider_metadata = CoreProviderMetadata::discover_async(discovery_url, async_http_client)
                .await
                .map_err(|err| OIDCBuildError::Discovery(format!("{err}")))?;
            CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
                .set_redirect_uri(redirect_url)
        } else if let Some(endpoints) = &config.endpoints {
            let issuer_url = IssuerUrl::new("http://github.com".into()).unwrap();
            let auth_url = AuthUrl::new(endpoints.authorization_url.clone())
                .map_err(|err| OIDCBuildError::InvalidAuth(format!("{err}")))?;
            let token_url = TokenUrl::new(endpoints.token_url.clone())
                .map_err(|err| OIDCBuildError::InvalidToken(format!("{err}")))?;
            let userinfo_url = UserInfoUrl::new(endpoints.userinfo_url.clone())
                .map_err(|err| OIDCBuildError::InvalidUserInfo(format!("{err}")))?;
            CoreClient::new(
                client_id,
                Some(client_secret),
                issuer_url,
                auth_url,
                Some(token_url),
                Some(userinfo_url),
                CoreJsonWebKeySet::default(),
            )
            .set_redirect_uri(redirect_url)
        } else {
            return Err(OIDCBuildError::InvalidEndpoints);
        };

        Ok(Self {
            provider: provider.to_string(),
            default_redirect_url: home_url.to_string(),
            client,
            identity_manager: identity_manager.clone(),
            session_manager: session_manager.clone(),
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let state = Arc::new(ServiceState {
            provider: self.provider.clone(),
            client: self.client,
            identity_manager: self.identity_manager,
            session_manager: self.session_manager,
            default_redirect_url: self.default_redirect_url,
        });

        Router::new()
            .route("/create_user", get(create_user))
            .route("/login", get(openid_connect_login))
            .route("/auth", get(openid_connect_auth))
            .with_state(state)
    }
}
