use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenUrl, reqwest::async_http_client,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error as ThisError;

use crate::{
    app_error::AppError,
    app_session::{AppSession, SessionData},
};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://www.googleapis.com/oauth2/v3/token";

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
}

impl IntoResponse for GoogleOAuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            GoogleOAuthError::MissingSession => StatusCode::BAD_REQUEST,
            GoogleOAuthError::InvalidSession => StatusCode::BAD_REQUEST,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

#[derive(Clone)]
struct Data {
    client: BasicClient,
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

    // Generate the authorization URL to which we'll redirect the user.
    let (authorize_url, csrf_state) = data
        .client
        .authorize_url(CsrfToken::new_random)
        //.add_scope(Scope::new("https://www.googleapis.com/auth/plus.me".to_string()))
        .add_scope(Scope::new("openid".to_string()))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    session.set(SessionData::GoogleLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
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
    let (pkce_code_verifier, csrf_state, redirect_url) = match session_data {
        SessionData::GoogleLogin {
            pkce_code_verifier,
            csrf_state,
            redirect_url,
        } => (PkceCodeVerifier::new(pkce_code_verifier), csrf_state, redirect_url),
        //_ => return Err(GoogleOAuthError::InvalidSession),
    };

    if csrf_state != auth_csrf_state {
        todo!() // return err
    }

    // Exchange the code with a token.
    let token = data
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await;


    //todo: request user profile from google by the token
    //register or update user

    //session.set("login", true).unwrap();
    let html = format!(
        r#"<html>
    <head><title>OAuth2 Test</title></head>
    <body>
        Google returned the following token:
        <pre>{:?}</pre>
        Redirecting to:
        <pre>{:?}</pre>
    </body>
</html>"#,
        token,
        redirect_url
    );

    Ok(html)
}

pub struct GoogleOAuth {
    client_id: ClientId,
    client_secret: ClientSecret,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    redirect_url: RedirectUrl,
}

impl GoogleOAuth {
    pub fn new(config: &GoogleOAuthConfig) -> Result<GoogleOAuth, AppError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_secret.clone());

        let auth_url = AuthUrl::new(AUTH_URL.to_string()).map_err(AppError::AuthUrlError)?;
        let token_url = TokenUrl::new(TOKEN_URL.to_string()).map_err(AppError::TokenUrlError)?;
        let redirect_url = RedirectUrl::new(config.redirect_url.to_string()).map_err(AppError::RedirectUrlError)?;

        Ok(GoogleOAuth {
            client_id,
            client_secret,
            auth_url,
            token_url,
            redirect_url,
        })
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        // Set up the config for the Google OAuth2 process.
        let client = BasicClient::new(
            self.client_id,
            Some(self.client_secret),
            self.auth_url,
            Some(self.token_url),
        )
        .set_redirect_uri(self.redirect_url);

        let state = Arc::new(Data { client });

        Router::new()
            .route("/google/login", get(google_login))
            .route("/google/auth", get(auth))
            .route("/signin-google", get(auth))
            .with_state(state)
    }
}
