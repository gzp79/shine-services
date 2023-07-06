use crate::auth::{
    create_ooops_page, create_redirect_page, oauth2_client::OAuth2Client, AuthServiceState, ExternalLoginData,
    ExternalLoginSession,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::Deserialize;
use shine_service::service::{UserSession, APP_NAME};
use std::sync::Arc;

#[derive(Deserialize)]
pub(in crate::auth) struct LoginRequest {
    redirect: Option<String>,
}

pub(in crate::auth) async fn oauth2_connect_login(
    State(state): State<AuthServiceState>,
    Extension(oauth2_client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<LoginRequest>,
    mut user_session: UserSession,
    mut external_login_session: ExternalLoginSession,
) -> Response {
    log::info!("user_session: {user_session:?}");
    log::info!("external_login: {external_login_session:?}");

    let current_user = user_session.take();
    let _ = external_login_session.take();

    // if there is a valid session, skip login flow and let the user in.
    if let Some(current_user) = current_user {
        if state
            .session_manager
            .find_session(current_user.user_id, current_user.key)
            .await
            .ok()
            .is_some()
        {
            let html = create_redirect_page(
                &state,
                "Redirecting to target login",
                APP_NAME,
                query.redirect.as_deref(),
            );

            // clear external_login_session, but keep user_session intact
            return (external_login_session, html).into_response();
        }
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = oauth2_client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(oauth2_client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    external_login_session.set(ExternalLoginData::OIDCLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect,
        link_session_id: None,
    });

    let html: axum::response::Html<String> = create_redirect_page(
        &state,
        "Redirecting to target login",
        &oauth2_client.provider,
        Some(authorize_url.as_str()),
    );
    // return a new external_login_session and clear the user_session
    (external_login_session, user_session, html).into_response()
}

pub(in crate::auth) async fn oauth2_connect_link(
    State(state): State<AuthServiceState>,
    Extension(oauth2_client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<LoginRequest>,
    mut user_session: UserSession,
    mut external_login_session: ExternalLoginSession,
) -> Response {
    log::info!("user_session: {user_session:?}");
    log::info!("external_login: {external_login_session:?}");

    if user_session.is_none() {
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

    external_login_session.set(ExternalLoginData::OIDCLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect,
        link_session_id: user_session.take(),
    });

    let html = create_redirect_page(
        &state,
        "Redirecting to target login",
        &oauth2_client.provider,
        Some(authorize_url.as_str()),
    );
    // return a new external_login_session, keep user_session intact
    (external_login_session, html).into_response()
}
