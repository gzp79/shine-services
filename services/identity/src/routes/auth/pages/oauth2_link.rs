use crate::{
    app_state::AppState,
    routes::auth::{AuthError, AuthPage, AuthPageRequest, AuthSession, ExternalLoginCookie, OAuth2Client, PageUtils},
};
use axum::{extract::State, Extension};
use oauth2::{CsrfToken, PkceCodeChallenge};
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
}

/// Link the current user to an OAuth2 provider.
#[utoipa::path(
    get,
    path = "/link",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Start the OAuth2 interactive login flow for linking an account")
    )
)]
pub async fn oauth2_link(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
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

    // 4. Verify user is logged in
    if req.auth_session().user_session().is_none() {
        return req.error_page(AuthError::LoginRequired, query.error_url.as_ref());
    }

    // 5. Business logic - setup OAuth2 link flow
    let key = random::hex_16(state.random());
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    let linked_user = req.auth_session().user_session().cloned();
    let response_session = req.into_auth_session().with_external_login(Some(ExternalLoginCookie {
        key,
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect_url,
        error_url: query.error_url,
        remember_me: false,
        linked_user,
    }));

    // 6. Return response
    assert!(response_session.user_session().is_some());
    PageUtils::new(&state).redirect(response_session, Some(&authorize_url), None)
}
