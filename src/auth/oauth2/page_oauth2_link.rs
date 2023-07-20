use crate::auth::{AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLogin, OAuth2Client};
use axum::{
    extract::{Query, State},
    Extension,
};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::Deserialize;
use std::sync::Arc;
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct RequestQuery {
    redirect_url: Option<Url>,
    error_url: Option<Url>,
}

/// Link the current user to an OAuth2 provider.
pub(in crate::auth) async fn page_oauth2_link(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<RequestQuery>,
    mut auth_session: AuthSession,
) -> AuthPage {
    if auth_session.user.is_none() {
        return state.page_error(auth_session, AuthError::LoginRequired, query.error_url.as_ref());
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
        target_url: query.redirect_url,
        error_url: query.error_url,
        remember_me: false,
        linked_user: auth_session.user.clone(),
    });

    state.page_redirect(auth_session, &client.provider, Some(&authorize_url))
}
