use crate::auth::{AuthPage, AuthServiceState, AuthSession, EnterRequestParams, ExternalLogin, OAuth2Client};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension,
};
use oauth2::{CsrfToken, PkceCodeChallenge};
use std::sync::Arc;

/// Login or register a new user with the interactive flow using an OAuth2 provider.
pub(in crate::auth) async fn page_oauth2_enter(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<EnterRequestParams>,
    mut auth_session: AuthSession,
) -> AuthPage {
    if !auth_session.is_empty() {
        return AuthPage::error(
            &state,
            None,
            StatusCode::BAD_REQUEST,
            "A log out is required to switch account",
        );
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    auth_session.external_login = Some(ExternalLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect,
        linked_user: None,
    });
    assert!(auth_session.user.is_none() && auth_session.token_login.is_none());

    AuthPage::external_redirect(&state, auth_session, &client.provider, authorize_url)
}
