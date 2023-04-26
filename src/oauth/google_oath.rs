use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Extension, Router,
};
use oauth2::{
    basic::BasicClient, url, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error as ThisError;

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
pub enum GoogleOAuthError {
    #[error("Invalid authorization endpoint URL")]
    AuthUrlError(url::ParseError),
    #[error("Invalid token endpoint URL")]
    TokenUrlError(url::ParseError),
    #[error("Invalid redirect URL")]
    RedirectUrlError(url::ParseError),
}

#[derive(Clone)]
struct State {
    client: BasicClient,
}

type StateExtension = Extension<Arc<State>>;

async fn google_login(Extension(state): StateExtension) -> impl IntoResponse {
    // Google supports Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, _pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the authorization URL to which we'll redirect the user.
    let (authorize_url, _csrf_state) = &state
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("https://www.googleapis.com/auth/plus.me".to_string()))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    (StatusCode::FOUND, [(header::LOCATION, authorize_url.to_string())])
}

#[derive(Deserialize)]
pub struct AuthRequest {
    code: String,
    state: String,
    scope: String,
}

async fn auth(Extension(state): StateExtension, Query(query): Query<AuthRequest>) -> impl IntoResponse {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_state = CsrfToken::new(query.state);
    log::info!("scope: {}", query.scope);

    // Exchange the code with a token.
    let token = &state.client.exchange_code(auth_code);

    //session.set("login", true).unwrap();
    let html = format!(
        r#"<html>
        <head><title>OAuth2 Test</title></head>
        <body>
            Google returned the following state:
            <pre>{}</pre>
            Google returned the following token:
            <pre>{:?}</pre>
        </body>
    </html>"#,
        auth_state.secret(),
        token
    );
    html
}

pub struct GoogleOAuth {
    client_id: ClientId,
    client_secret: ClientSecret,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    redirect_url: RedirectUrl,
}

impl GoogleOAuth {
    pub fn new(config: &GoogleOAuthConfig) -> Result<GoogleOAuth, GoogleOAuthError> {
        let client_id = ClientId::new(config.client_id.clone());
        let client_secret = ClientSecret::new(config.client_id.clone());

        let auth_url = AuthUrl::new(AUTH_URL.to_string()).map_err(GoogleOAuthError::AuthUrlError)?;
        let token_url = TokenUrl::new(TOKEN_URL.to_string()).map_err(GoogleOAuthError::TokenUrlError)?;
        let redirect_url =
            RedirectUrl::new(config.redirect_url.to_string()).map_err(GoogleOAuthError::RedirectUrlError)?;

        Ok(GoogleOAuth {
            client_id,
            client_secret,
            auth_url,
            token_url,
            redirect_url,
        })
    }

    pub fn into_router(self) -> Router {
        // Set up the config for the Google OAuth2 process.
        let client = BasicClient::new(
            self.client_id,
            Some(self.client_secret),
            self.auth_url,
            Some(self.token_url),
        )
        .set_redirect_uri(self.redirect_url);

        let state = Arc::new(State { client });

        Router::new()
            .route("/google/login", get(google_login))
            .route("/google/auth", get(auth))
            .layer(Extension(state))
    }
}
