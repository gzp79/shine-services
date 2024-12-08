use crate::{
    auth::{
        token::TokenGenerator, AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLoginCookie, OIDCClient,
    },
    openapi::ApiKind,
};
use axum::{extract::State, Extension};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, InputError, OpenApiUrl, ProblemDetail, ValidatedQuery};
use std::sync::Arc;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    redirect_url: Option<OpenApiUrl>,
    error_url: Option<OpenApiUrl>,
}

/// Link the current user to an OpenId Connect provider.
async fn oidc_link(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<Query>, ProblemDetail<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::InputError(error.problem), None),
    };

    if auth_session.user_session.is_none() {
        return state.page_error(auth_session, AuthError::LoginRequired, query.error_url.as_deref());
    }

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => return state.page_error(auth_session, AuthError::OIDCDiscovery(err), query.error_url.as_deref()),
    };

    let key = match TokenGenerator::new(state.random()).generate() {
        Ok(key) => key,
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_deref()),
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

    auth_session.external_login_cookie = Some(ExternalLoginCookie {
        key,
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: Some(nonce.secret().to_owned()),
        target_url: query.redirect_url.map(|url| url.into_url()),
        error_url: query.error_url.map(|url| url.into_url()),
        remember_me: false,
        linked_user: auth_session.user_session.clone(),
    });

    state.page_redirect(auth_session, &client.provider, Some(&authorize_url))
}

pub fn page_oidc_link(provider: &str) -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage(provider, "/link"), oidc_link)
        .with_operation_id(format!("page_{provider}_link"))
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response(
            "Html page to update client cookies and redirect user to start interactive OpenIdConnect login flow",
        )
}
