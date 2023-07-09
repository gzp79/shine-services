use crate::auth::{AuthPage, AuthServiceState, AuthSession, EnterRequestParams, ExternalLogin, OIDCClient};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension,
};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use std::sync::Arc;

/// Link the current user to an OpenId Connect provider.
pub(in crate::auth) async fn page_oidc_link(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    Query(query): Query<EnterRequestParams>,
    mut auth_session: AuthSession,
) -> AuthPage {
    if auth_session.user.is_none() {
        return AuthPage::error(&state, None, StatusCode::FORBIDDEN, "Login required");
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state, nonce) = client
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scopes(client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .set_max_age(Duration::minutes(30).to_std().unwrap())
        .add_prompt(CoreAuthPrompt::Login)
        .url();

    auth_session.external_login = Some(ExternalLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: Some(nonce.secret().to_owned()),
        target_url: query.redirect,
        linked_user: auth_session.user.clone(),
    });

    AuthPage::external_redirect(&state, auth_session, &client.provider, authorize_url)
}
