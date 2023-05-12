use crate::{
    app_error::AppError,
    app_session::{AppSession, ExternalLoginData, ExternalLoginSession, ExternalLoginState, SessionData},
    db::{DBError, ExternalLogin, IdentityManager},
    utils::generate_name,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response, Html},
    routing::get,
    Extension, Router,
};
use chrono::Duration;
use oauth2::{
    reqwest::async_http_client, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope,
};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    IssuerUrl, Nonce, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tera::Tera;
use thiserror::Error as ThisError;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenIdConnectConfig {
    pub discovery_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

#[derive(Debug, ThisError)]
enum OpenIdConnectError {
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

    #[error(transparent)]
    DBError(#[from] DBError),
    #[error(transparent)]
    TeraError(#[from] tera::Error),
}

impl IntoResponse for OpenIdConnectError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            OpenIdConnectError::MissingSession => StatusCode::BAD_REQUEST,
            OpenIdConnectError::InconsistentSession => StatusCode::BAD_REQUEST,
            OpenIdConnectError::InvalidCsrfState => StatusCode::BAD_REQUEST,
            OpenIdConnectError::FailedTokenExchange(_) => StatusCode::BAD_REQUEST,
            OpenIdConnectError::MissingIdToken => StatusCode::BAD_REQUEST,
            OpenIdConnectError::FailedIdVerification(_) => StatusCode::BAD_REQUEST,
            OpenIdConnectError::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            OpenIdConnectError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

struct ServiceState {
    provider: String,
    client: CoreClient,
    identity_manager: IdentityManager,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    redirect: Option<String>,
}

async fn openid_connect_login(
    State(data): State<Arc<ServiceState>>,
    Extension(tera): Extension<Arc<Tera>>,
    Query(query): Query<LoginRequest>,
    session: AppSession,
    mut external_login: ExternalLoginSession,
) -> Result<impl IntoResponse, OpenIdConnectError> {
    // Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    let scopes = ["openid", "email", "profile"];
    // Generate the authorization URL to which we'll redirect the user.
    let (authorize_url, csrf_state, nonce) = data
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

    // Connect external login to the current session (or None if there is no session).
    let session_id = session.as_ref().map(|s| s.session_id.clone());
    let state = ExternalLoginState::OpenIdConnectLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: nonce.secret().to_owned(),
        redirect_url: query.redirect,
    };
    external_login.set(ExternalLoginData { session_id, state });

    log::info!("session: {session:?}");
    log::info!("external_login: {external_login:?}");

    //Return an auto-redirect page that stores cookie before redirecting the user to the authorize_url.
    // In older browser with a simple StatusCode::FOUND response, no cookie headers could be sent to the client.
    let mut context = tera::Context::new();
    context.insert("title", &"External login");
    context.insert("target", &data.provider);
    context.insert("redirect_url", &authorize_url.to_string());
    let html = Html(tera.render("redirect.html", &context)?);

    Ok((external_login, html))
}

/*
async fn create_user(State(data): State<Arc<Data>>) -> Result<String, OpenIdConnectError> {
    let user = data.identity_manager.create_user("name".into(), None, None).await?;

    //session.set("login", true).unwrap();
    let html = format!(
        r#"<html>
    <head><title>OAuth2 Test</title></head>
    <body>
        User id
        <pre>{:?}</pre>
    </body>
</html>"#,
        user
    );

    Ok(html)
}*/

#[derive(Deserialize)]
pub struct AuthRequest {
    code: String,
    state: String,
    //scope: String,
}

async fn openid_connect_auth(
    State(data): State<Arc<ServiceState>>,
    Extension(tera): Extension<Arc<Tera>>,
    Query(query): Query<AuthRequest>,
    mut session: AppSession,
    mut external_login: ExternalLoginSession,
) -> Result<impl IntoResponse, OpenIdConnectError> {
    log::info!("session: {session:?}");
    log::info!("external_login: {external_login:?}");

    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let external_login_data = external_login.take().ok_or(OpenIdConnectError::MissingSession)?;
    let (pkce_code_verifier, csrf_state, nonce, redirect_url) = match external_login_data.state {
        ExternalLoginState::OpenIdConnectLogin {
            pkce_code_verifier,
            csrf_state,
            nonce,
            redirect_url,
        } => (
            PkceCodeVerifier::new(pkce_code_verifier),
            csrf_state,
            Nonce::new(nonce),
            redirect_url,
        ),
        //_ => return Err(OpenIdConnectError::InvalidSession),
    };

    // check if the cookies belong to the same session, or here was no valid session
    if external_login_data.session_id.as_ref() != session.as_ref().map(|s| &s.session_id) {
        return Err(OpenIdConnectError::InconsistentSession);
    }

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        return Err(OpenIdConnectError::InvalidCsrfState);
    }

    // Exchange the code with a token.
    let token = data
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|err| OpenIdConnectError::FailedTokenExchange(format!("{err}")))?;

    let id_token = token.id_token().ok_or(OpenIdConnectError::MissingIdToken)?;
    let claims = id_token
        .claims(&data.client.id_token_verifier(), &nonce)
        .map_err(|err| OpenIdConnectError::FailedIdVerification(format!("{err}")))?;

    let name = claims
        .nickname()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str().to_owned())
        .unwrap_or_else(|| generate_name());
    let email = claims.email().map(|n| n.as_str().to_owned());
    let provider_id = claims.subject().as_str().to_owned();
    let external_login = ExternalLogin {
        provider: data.provider.clone(),
        provider_id,
    };

    let identity = if let Some(identity) = data.identity_manager.find_user_by_link(&external_login).await? {
        // already linked -> perform a simple login
        identity
    } else if let Some(session) = &*session {
        // linking account to an existing user
        data.identity_manager.link_user(session.id, &external_login).await?
    } else {
        // account was just created
        //todo: if data.identity_manager.find_suer_by_email(email).is_some(), ask if he would rather link them. If so,
        //      login first than go to the link page
        //todo: if email is new, ask if he/she wants to create a new account
        data.identity_manager
            .create_user(name, email, Some(&external_login))
            .await?
    };

    //session.set("login", true).unwrap();
    let html = format!(
        r#"<html>
    <head><title>OAuth2 Test</title></head>
    <body>
        Google id
        <pre>{:?}</pre>
        Redirecting to:
        <pre>{:?}</pre>
        Identity to:
        <pre>{:?}</pre>
    </body>
</html>"#,
        claims, redirect_url, identity
    );

    Ok(html)
}

pub struct OpenIdConnect {
    provider: String,
    client: CoreClient,
    identity_manager: IdentityManager,
}

impl OpenIdConnect {
    pub async fn new(
        provider: &str,
        config: &OpenIdConnectConfig,
        identity_manager: IdentityManager,
    ) -> Result<OpenIdConnect, AppError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let issuer_url = IssuerUrl::new(config.discovery_url.clone())
            .map_err(|err| AppError::ExternalLoginInitializeError(format!("{err}")))?;
        let redirect_url = RedirectUrl::new(config.redirect_url.to_string())
            .map_err(|err| AppError::ExternalLoginInitializeError(format!("{err}")))?;

        // Use OpenID Connect Discovery to fetch the provider metadata.

        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
            .await
            .map_err(|err| AppError::ExternalLoginInitializeError(format!("{err}")))?;
        let client = CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
            .set_redirect_uri(redirect_url);

        Ok(OpenIdConnect {
            provider: provider.to_string(),
            client,
            identity_manager,
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
        });

        Router::new()
            .route("/login", get(openid_connect_login))
            .route("/auth", get(openid_connect_auth))
            .with_state(state)
    }
}
