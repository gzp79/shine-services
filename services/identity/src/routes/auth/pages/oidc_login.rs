use crate::{
    app_state::AppState,
    routes::auth::{AuthPage, AuthPageRequest, AuthSession, ExternalLoginCookie, ExternalLoginError, OIDCClient},
};
use axum::{extract::State, Extension};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use serde::Deserialize;
use shine_infra::{
    crypto::random,
    web::{
        extracts::{InputError, ValidatedQuery},
        responses::ErrorResponse,
    },
};
use std::sync::Arc;
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    remember_me: Option<bool>,
    captcha: Option<String>,
}

/// Login or register a new user with the interactive flow using an OpenID Connect provider.
#[utoipa::path(
    get,
    path = "/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Start the OpenID Connect interactive login flow")
    )
)]
pub async fn oidc_login(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
) -> AuthPage {
    // 1. Create request helper
    let req = AuthPageRequest::new(&state, auth_session);

    // 2. Validate query
    let query = match req.validate_query(query) {
        Ok(q) => q,
        Err(page) => return page,
    };

    // 3. Validate redirect URLs
    if let Some(page) = req.validate_redirect_urls(query.redirect_url.as_ref(), query.error_url.as_ref()) {
        return page;
    }

    log::debug!("Query: {query:#?}");

    // 4. Validate captcha
    if let Some(page) = req
        .validate_captcha(query.captcha.as_deref(), query.error_url.as_ref())
        .await
    {
        return page;
    }

    // 5. Business logic - setup OIDC flow
    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => {
            return req.error_page(
                ExternalLoginError::OIDCDiscovery(format!("{err:#?}")),
                query.error_url.as_ref(),
            )
        }
    };

    let key = random::hex_16(state.random());
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

    let response_session = req
        .into_auth_session()
        .revoke_session(&state)
        .await
        .revoke_access(&state)
        .await
        .with_external_login(Some(ExternalLoginCookie {
            key,
            pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
            csrf_state: csrf_state.secret().to_owned(),
            nonce: Some(nonce.secret().to_owned()),
            target_url: query.redirect_url,
            error_url: query.error_url,
            remember_me: query.remember_me.unwrap_or(false),
            linked_user: None,
        }));

    // 6. Return response
    assert!(response_session.user_session().is_none());
    state
        .auth_page_handler()
        .redirect(response_session, Some(&authorize_url), None)
}
