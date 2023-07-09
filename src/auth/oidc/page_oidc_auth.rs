use crate::auth::{
    external_auth::page_external_auth, AuthPage, AuthServiceState, AuthSession, ExternalLogin, ExternalUserInfo,
    OIDCClient,
};
use axum::{
    extract::{Query, State},
    Extension,
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier};
use openidconnect::{Nonce, TokenResponse};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub(in crate::auth) struct AuthRequest {
    code: String,
    state: String,
}

/// Process the authentication redirect from the OpenID Connect provider.
pub(in crate::auth) async fn page_oidc_auth(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    Query(query): Query<AuthRequest>,
    mut auth_session: AuthSession,
) -> AuthPage {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let ExternalLogin {
        pkce_code_verifier,
        csrf_state,
        nonce,
        target_url,
        linked_user,
    } = match auth_session.external_login.take() {
        Some(external_login) => external_login,
        None => {
            log::debug!("Missing external session");
            return AuthPage::invalid_session_logout(&state, auth_session);
        }
    };

    let nonce = match nonce {
        Some(nonce) => nonce,
        None => {
            log::debug!("Missing nonce");
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

    let id_token = match token.id_token() {
        Some(token) => token,
        None => return AuthPage::internal_error(&state, Some(auth_session), "Token contains no ID"),
    };

    // extract claims from the returned (jwt) token
    let claims = match id_token.claims(&client.client.id_token_verifier(), &Nonce::new(nonce)) {
        Ok(token) => token,
        Err(err) => {
            log::debug!("Failed to extract claims: {:?}", err);
            return AuthPage::internal_error(&state, Some(auth_session), err);
        }
    };
    log::debug!("Code exchange completed, claims: {claims:#?}");

    let external_user_info = {
        let external_id = claims.subject().to_string();
        let name = claims
            .nickname()
            .and_then(|n| n.get(None))
            .map(|n| n.as_str().to_owned());
        let email = claims.email().map(|n| n.as_str().to_owned());

        ExternalUserInfo {
            external_id,
            name,
            email,
        }
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
