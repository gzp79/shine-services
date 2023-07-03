use crate::{
    auth::{create_redirect_page, oidc_client::OIDCClient, ExternalLoginData, ExternalLoginSession},
    db::{SessionManager, SettingsManager},
};
use axum::{
    extract::Query,
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
use shine_service::service::UserSession;
use std::sync::Arc;
use tera::Tera;

#[derive(Deserialize)]
pub(in crate::auth) struct LoginRequest {
    redirect: Option<String>,
    allow_link: Option<bool>,
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
    if !query.allow_link.unwrap_or(false) {
        let user_session_data = user_session.take();
        let _ = external_login_session.take();

        // if this is not a link-account request and there is a valid session, skip login and let the user in.
        if let Some(user_session_data) = user_session_data {
            if session_manager
                .find_session(user_session_data.user_id, user_session_data.key)
                .await
                .ok()
                .is_some()
            {
                let html = create_redirect_page(
                    &tera,
                    &settings_manager,
                    "Redirecting to target login",
                    &oidc_client.provider,
                    query.redirect.as_deref(),
                );

                user_session.set(user_session_data);
                return (external_login_session, user_session, html).into_response();
            }
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
        link_session_id: user_session.clone(),
    });

    log::info!("user_session: {user_session:?}");
    log::info!("external_login: {external_login_session:?}");

    let html = create_redirect_page(
        &tera,
        &settings_manager,
        "Redirecting to external login",
        &oidc_client.provider,
        Some(authorize_url.as_str()),
    );
    (external_login_session, user_session, html).into_response()
}
