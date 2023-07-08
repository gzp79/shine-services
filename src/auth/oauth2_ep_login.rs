use crate::auth::{
    create_ooops_page, create_redirect_page, oauth2_client::OAuth2Client, AuthServiceState, AuthSession, ExternalLogin,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub(in crate::auth) struct LoginRequest {
    redirect: Option<String>,
}

pub(in crate::auth) async fn oauth2_connect_login(
    State(state): State<AuthServiceState>,
    Extension(oauth2_client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<LoginRequest>,
    mut auth_session: AuthSession,
) -> Response {
    if !auth_session.is_empty() {
        let html = create_ooops_page(&state, Some("A log out is required to switch account"));
        return (StatusCode::BAD_REQUEST, html).into_response();
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = oauth2_client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(oauth2_client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    auth_session.external_login = Some(ExternalLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect,
        link_session_id: None,
    });
    assert!(auth_session.user.is_none() && auth_session.token_login.is_none());

    let html: axum::response::Html<String> = create_redirect_page(
        &state,
        "Redirecting to target login",
        &oauth2_client.provider,
        Some(authorize_url.as_str()),
    );

    (auth_session, html).into_response()
}

pub(in crate::auth) async fn oauth2_connect_link(
    State(state): State<AuthServiceState>,
    Extension(oauth2_client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<LoginRequest>,
    mut auth_session: AuthSession,
) -> Response {
    if auth_session.user.is_none() {
        let html = create_ooops_page(&state, Some("Login required"));
        return (StatusCode::FORBIDDEN, html).into_response();
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = oauth2_client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(oauth2_client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    auth_session.external_login = Some(ExternalLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect,
        link_session_id: auth_session.user.clone(),
    });

    let html = create_redirect_page(
        &state,
        "Redirecting to target login",
        &oauth2_client.provider,
        Some(authorize_url.as_str()),
    );

    (auth_session, html).into_response()
}
