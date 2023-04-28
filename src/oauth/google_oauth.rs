use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use oauth2::{
    reqwest::async_http_client, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope,
};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    IssuerUrl, Nonce, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error as ThisError;

use crate::{
    app_error::AppError,
    app_session::{AppSession, SessionData},
};

const OPENID_DISCOVERY_URL: &str = "https://accounts.google.com";
//const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
//const TOKEN_URL: &str = "https://www.googleapis.com/oauth2/v3/token";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

#[derive(Debug, ThisError)]
enum GoogleOAuthError {
    #[error("Session cookie was missing or corrupted")]
    MissingSession,
    #[error("Session cookie is expired")]
    InvalidSession,
    #[error("Cross Server did not return an ID token")]
    InvalidCsrfState,
    #[error("Failed to exchange authorization code to access token: {0}")]
    FailedTokenExchange(String),
    #[error("Cross-Site Request Forgery (Csrf) check failed")]
    MissingIdToken,
    #[error("Failed to verify id token: {0}")]
    FailedIdVerification(String),
}

impl IntoResponse for GoogleOAuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            GoogleOAuthError::MissingSession => StatusCode::BAD_REQUEST,
            GoogleOAuthError::InvalidSession => StatusCode::BAD_REQUEST,
            GoogleOAuthError::InvalidCsrfState => StatusCode::BAD_REQUEST,
            GoogleOAuthError::FailedTokenExchange(_) => StatusCode::BAD_REQUEST,
            GoogleOAuthError::MissingIdToken => StatusCode::BAD_REQUEST,
            GoogleOAuthError::FailedIdVerification(_) => StatusCode::BAD_REQUEST,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

#[derive(Clone)]
struct Data {
    client: CoreClient,
}
#[derive(Deserialize)]
pub struct LoginRequest {
    redirect: Option<String>,
}

async fn google_login(
    State(data): State<Arc<Data>>,
    Query(query): Query<LoginRequest>,
    mut session: AppSession,
) -> impl IntoResponse {
    // Google supports Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
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
        .url();

    session.set(SessionData::GoogleLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: nonce.secret().to_owned(),
        redirect_url: query.redirect,
    });

    log::info!("session: {session:?}");
    //todo: return an auto-redirect from to store cookie and redirect the user to google
    (
        StatusCode::FOUND,
        [(header::LOCATION, authorize_url.to_string())],
        session,
    )
}

#[derive(Deserialize)]
pub struct AuthRequest {
    code: String,
    state: String,
    //scope: String,
}

async fn auth(
    State(data): State<Arc<Data>>,
    Query(query): Query<AuthRequest>,
    mut session: AppSession,
) -> Result<String, GoogleOAuthError> {
    log::info!("session: {session:?}");

    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let session_data = session.take().ok_or(GoogleOAuthError::MissingSession)?;
    let (pkce_code_verifier, csrf_state, nonce, redirect_url) = match session_data {
        SessionData::GoogleLogin {
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
        //_ => return Err(GoogleOAuthError::InvalidSession),
    };

    if csrf_state != auth_csrf_state {
        return Err(GoogleOAuthError::InvalidCsrfState);
    }

    // Exchange the code with a token.
    let token = data
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|err| GoogleOAuthError::FailedTokenExchange(format!("{err}")))?;

    let id_token = token.id_token().ok_or(GoogleOAuthError::MissingIdToken)?;
    let claims = id_token
        .claims(&data.client.id_token_verifier(), &nonce)
        .map_err(|err| GoogleOAuthError::FailedIdVerification(format!("{err}")))?;

    //todo: request user profile from google by the token
    //register or update user

    //session.set("login", true).unwrap();
    let html = format!(
        r#"<html>
    <head><title>OAuth2 Test</title></head>
    <body>
        Google id
        <pre>{:?}</pre>
        Redirecting to:
        <pre>{:?}</pre>
    </body>
</html>"#,
        claims, redirect_url
    );

    Ok(html)
}

pub struct GoogleOAuth {
    client: CoreClient,
}

impl GoogleOAuth {
    pub async fn new(config: &GoogleOAuthConfig) -> Result<GoogleOAuth, AppError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());
        let issuer_url = IssuerUrl::new(OPENID_DISCOVERY_URL.to_string())
            .map_err(|err| AppError::ExternalLoginInitializeError(format!("{err}")))?;
        let redirect_url = RedirectUrl::new(config.redirect_url.to_string())
            .map_err(|err| AppError::ExternalLoginInitializeError(format!("{err}")))?;

        // Use OpenID Connect Discovery to fetch the provider metadata.

        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
            .await
            .map_err(|err| AppError::ExternalLoginInitializeError(format!("{err}")))?;
        let client = CoreClient::from_provider_metadata(provider_metadata, client_id, Some(client_secret))
            .set_redirect_uri(redirect_url);

        Ok(GoogleOAuth { client })
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let state = Arc::new(Data { client: self.client });

        Router::new()
            .route("/google/login", get(google_login))
            .route("/google/auth", get(auth))
            .with_state(state)
    }
}
