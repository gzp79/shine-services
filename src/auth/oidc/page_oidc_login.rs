use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLogin, OIDCClient},
    openapi::ApiKind,
};
use axum::{body::HttpBody, extract::State, Extension};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, ValidatedQuery};
use std::sync::Arc;
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    remember_me: Option<bool>,
}

/// Login or register a new user with the interactive flow using an OpenID Connect provider.
async fn oidc_login(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    mut auth_session: AuthSession,
    ValidatedQuery(query): ValidatedQuery<Query>,
) -> AuthPage {
    if auth_session.user.is_some() {
        return state.page_error(auth_session, AuthError::LogoutRequired, query.error_url.as_ref());
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
        target_url: query.redirect_url,
        error_url: query.error_url,
        remember_me: query.remember_me.unwrap_or(false),
        linked_user: None,
    });
    assert!(auth_session.user.is_none() && auth_session.token_login.is_none());

    state.page_redirect(auth_session, &client.provider, Some(&authorize_url))
}

pub fn page_oidc_login<B>(provider: &str) -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage(provider, "/login"), oidc_login)
        .with_operation_id(format!("page_{provider}_login"))
        .with_tag("login")
        .with_query_parameter::<Query>()
        .with_page_response(
            "Html page to update client cookies and redirect user to start interactive OpenIdConnect login flow",
        )
}
