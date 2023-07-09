use crate::auth::{AuthPage, AuthServiceState, AuthSession, EnterRequestParams, ExternalLogin, OAuth2Client};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension,
};
use oauth2::{CsrfToken, PkceCodeChallenge};
use std::sync::Arc;

/// Link the current user to an OAuth2 provider.
pub(in crate::auth) async fn page_oauth2_link(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<EnterRequestParams>,
    mut auth_session: AuthSession,
) -> AuthPage {
    if auth_session.user.is_none() {
        return AuthPage::error(&state, None, StatusCode::FORBIDDEN, "Login required");
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
        linked_user: auth_session.user.clone(),
    });

    AuthPage::external_redirect(&state, auth_session, &client.provider, authorize_url)
}
