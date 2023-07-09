use crate::auth::{
    get_external_user_info, page_external_auth, AuthPage, AuthServiceState, AuthSession, ExternalLogin, OAuth2Client,
};
use axum::{
    extract::{Query, State},
    Extension,
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier, TokenResponse};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub(in crate::auth) struct AuthRequest {
    code: String,
    state: String,
}

/// Process the authentication redirect from the OAuth2 provider.
pub(in crate::auth) async fn page_oauth2_auth(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<AuthRequest>,
    mut auth_session: AuthSession,
) -> AuthPage {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let ExternalLogin {
        pkce_code_verifier,
        csrf_state,
        target_url,
        linked_user,
        ..
    } = match auth_session.external_login.take() {
        Some(external_login) => external_login,
        None => {
            log::debug!("Missing external session");
            return AuthPage::invalid_session_logout(&state, auth_session);
        }
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::debug!("CSRF test failed: [{csrf_state}], [{auth_csrf_state}]");
        return AuthPage::invalid_session_logout(&state, auth_session);
    }

    // Exchange the code with a token.
    let token = match client
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_code_verifier))
        .request_async(async_http_client)
        .await
    {
        Ok(token) => token,
        Err(err) => return AuthPage::internal_error(&state, Some(auth_session), err),
    };

    let external_user_info = match get_external_user_info(
        client.user_info_url.url().clone(),
        token.access_token().secret(),
        &client.user_info_mapping,
        &client.extensions,
    )
    .await
    {
        Ok(external_user_info) => external_user_info,
        Err(err) => return AuthPage::internal_error(&state, Some(auth_session), err),
    };
    log::info!("{:?}", external_user_info);

    page_external_auth(
        &state,
        auth_session,
        linked_user,
        &client.provider,
        external_user_info,
        target_url,
    )
    .await
}
