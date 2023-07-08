use crate::auth::{
    create_ooops_page, create_redirect_page, oidc_client::OIDCClient, AuthServiceState, AuthSession, ExternalLogin,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub(in crate::auth) struct LoginRequest {
    redirect: Option<String>,
}

pub(in crate::auth) async fn openid_connect_login(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    Query(query): Query<LoginRequest>,
    mut auth_session: AuthSession,
) -> Response {
    log::info!("auth_session: {auth_session:?}");

    if !auth_session.is_empty() {
        let html = create_ooops_page(&state, Some("A log out is required to switch account"));
        return (StatusCode::BAD_REQUEST, html).into_response();
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
        link_session_id: None,
    });
    assert!(auth_session.user.is_none() && auth_session.token_login.is_none());

    let html = create_redirect_page(
        &state,
        "Redirecting to target login",
        &client.provider,
        Some(authorize_url.as_str()),
    );

    (auth_session, html).into_response()
}

pub(in crate::auth) async fn openid_connect_link(
    State(state): State<AuthServiceState>,
    Extension(oidc_client): Extension<Arc<OIDCClient>>,
    Query(query): Query<LoginRequest>,
    mut auth_session: AuthSession,
) -> Response {
    log::info!("auth_session: {auth_session:?}");

    if auth_session.user.is_none() {
        let html = create_ooops_page(&state, Some("Login required"));
        return (StatusCode::FORBIDDEN, html).into_response();
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state, nonce) = oidc_client
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scopes(oidc_client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .set_max_age(Duration::minutes(30).to_std().unwrap())
        .add_prompt(CoreAuthPrompt::Login)
        .url();

    auth_session.external_login = Some(ExternalLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: Some(nonce.secret().to_owned()),
        target_url: query.redirect,
        link_session_id: auth_session.user.clone(),
    });

    let html = create_redirect_page(
        &state,
        "Redirecting to target login",
        &oidc_client.provider,
        Some(authorize_url.as_str()),
    );

    (auth_session, html).into_response()
}
