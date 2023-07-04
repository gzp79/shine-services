use crate::{
    auth::{create_ooops_page, create_redirect_page, oidc_client::OIDCClient, ExternalLoginData, ExternalLoginSession},
    db::{SessionManager, SettingsManager},
};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge, Scope};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use serde::Deserialize;
use shine_service::service::{UserSession, APP_NAME};
use std::sync::Arc;
use tera::Tera;

#[derive(Deserialize)]
pub(in crate::auth) struct LoginRequest {
    redirect: Option<String>,
}

pub(in crate::auth) async fn openid_connect_login(
    Extension(oidc_client): Extension<Arc<OIDCClient>>,
    Extension(tera): Extension<Arc<Tera>>,
    Extension(settings_manager): Extension<SettingsManager>,
    Extension(session_manager): Extension<SessionManager>,
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
        if session_manager
            .find_session(current_user.user_id, current_user.key)
            .await
            .ok()
            .is_some()
        {
            let html = create_redirect_page(
                &tera,
                &settings_manager,
                "Redirecting to target login",
                APP_NAME,
                query.redirect.as_deref(),
            );

            // clear external_login_session, but keep user_session intact
            return (external_login_session, html).into_response();
        }
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let scopes = ["openid", "email", "profile"];
    let (authorize_url, csrf_state, nonce) = oidc_client
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scopes(scopes.into_iter().map(|s| Scope::new(s.to_string())))
        .set_pkce_challenge(pkce_code_challenge)
        .set_max_age(Duration::minutes(30).to_std().unwrap())
        .add_prompt(CoreAuthPrompt::Login)
        .url();

    external_login_session.set(ExternalLoginData::OIDCLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: nonce.secret().to_owned(),
        target_url: query.redirect,
        link_session_id: None,
    });

    let html = create_redirect_page(
        &tera,
        &settings_manager,
        "Redirecting to target login",
        &oidc_client.provider,
        Some(authorize_url.as_str()),
    );
    // return a new external_login_session and clear the user_session
    (external_login_session, user_session, html).into_response()
}

pub(in crate::auth) async fn openid_connect_link(
    Extension(oidc_client): Extension<Arc<OIDCClient>>,
    Extension(tera): Extension<Arc<Tera>>,
    Extension(settings_manager): Extension<SettingsManager>,
    Query(query): Query<LoginRequest>,
    mut user_session: UserSession,
    mut external_login_session: ExternalLoginSession,
) -> Response {
    log::info!("user_session: {user_session:?}");
    log::info!("external_login: {external_login_session:?}");

    if user_session.is_none() {
        let html = create_ooops_page(&tera, &settings_manager, Some("Login required"));
        return (StatusCode::FORBIDDEN, html).into_response();
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let scopes = ["openid", "email", "profile"];
    let (authorize_url, csrf_state, nonce) = oidc_client
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scopes(scopes.into_iter().map(|s| Scope::new(s.to_string())))
        .set_pkce_challenge(pkce_code_challenge)
        .set_max_age(Duration::minutes(30).to_std().unwrap())
        .add_prompt(CoreAuthPrompt::Login)
        .url();

    external_login_session.set(ExternalLoginData::OIDCLogin {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: nonce.secret().to_owned(),
        target_url: query.redirect,
        link_session_id: user_session.take(),
    });

    let html = create_redirect_page(
        &tera,
        &settings_manager,
        "Redirecting to target login",
        &oidc_client.provider,
        Some(authorize_url.as_str()),
    );
    // return a new external_login_session, keep user_session intact
    (external_login_session, html).into_response()
}
