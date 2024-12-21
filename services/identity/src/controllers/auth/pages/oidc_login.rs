use crate::controllers::{
    auth::{AuthError, AuthPage, AuthSession, CaptchaUtils, ExternalLoginCookie, OIDCClient, PageUtils},
    ApiKind, AppState,
};
use axum::{extract::State, Extension};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use serde::Deserialize;
use shine_core::axum::{ApiEndpoint, ApiMethod, ConfiguredProblem, InputError, OpenApiUrl, ValidatedQuery};
use std::sync::Arc;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct QueryParams {
    redirect_url: Option<OpenApiUrl>,
    error_url: Option<OpenApiUrl>,
    remember_me: Option<bool>,
    captcha: Option<String>,
}

/// Login or register a new user with the interactive flow using an OpenID Connect provider.
async fn oidc_login(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ConfiguredProblem<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, AuthError::InputError(error.problem), None),
    };

    if let Err(err) = CaptchaUtils::new(&state).validate(query.captcha.as_deref()).await {
        return PageUtils::new(&state).error(auth_session, err, query.error_url.as_deref());
    }
    // Note: having a token login is not an error, on successful start of the flow, the token cookie is cleared
    // It has some potential issue: if tid is connected to a guest user, the guest may loose all the progress
    if auth_session.user_session.is_some() {
        return PageUtils::new(&state).error(auth_session, AuthError::LogoutRequired, query.error_url.as_deref());
    }

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                AuthError::OIDCDiscovery(err),
                query.error_url.as_deref(),
            )
        }
    };

    let key = match state.token_service().generate() {
        Ok(key) => key,
        Err(err) => return PageUtils::new(&state).internal_error(auth_session, err, query.error_url.as_deref()),
    };

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state, nonce) = core_client
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

    auth_session.token_cookie = None;

    auth_session.external_login_cookie = Some(ExternalLoginCookie {
        key,
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: Some(nonce.secret().to_owned()),
        target_url: query.redirect_url.map(|url| url.into_url()),
        error_url: query.error_url.map(|url| url.into_url()),
        remember_me: query.remember_me.unwrap_or(false),
        linked_user: None,
    });

    assert!(auth_session.user_session.is_none());

    PageUtils::new(&state).redirect(auth_session, Some(&client.provider), Some(&authorize_url))
}

pub fn page_oidc_login(provider: &str) -> ApiEndpoint<AppState> {
    ApiEndpoint::new(
        ApiMethod::Get,
        ApiKind::Page(&format!("/auth/{provider}/login")),
        oidc_login,
    )
    .with_operation_id(format!("{provider}_login"))
    .with_tag("page")
    .with_query_parameter::<QueryParams>()
    .with_page_response(
        "Html page to update client cookies and redirect user to start interactive OpenIdConnect login flow",
    )
}
