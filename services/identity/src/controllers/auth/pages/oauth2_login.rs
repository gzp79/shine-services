use crate::{
    app_state::AppState,
    controllers::auth::{AuthPage, AuthSession, AuthUtils, ExternalLoginCookie, OAuth2Client, PageUtils},
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
    remember_me: Option<bool>,
    captcha: Option<String>,
}

/// Login or register a new user with the interactive flow using an OAuth2 provider.
#[utoipa::path(
    get,
    path = "/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Start the OAuth2 interactive login flow")
    )
)]
pub async fn oauth2_login(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None),
    };
    if let Some(error_url) = &query.error_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("errorUrl", error_url) {
            return PageUtils::new(&state).error(auth_session, err, None);
        }
    }
    if let Some(redirect_url) = &query.redirect_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("redirectUrl", redirect_url) {
            return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref());
        }
    }

    log::debug!("Query: {query:#?}");

    if let Err(err) = state.captcha_validator().validate(query.captcha.as_deref()).await {
        return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref());
    }

    let key = random::hex_16(state.random());
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    let response_session = auth_session
        .revoke_session(&state)
        .await
        .revoke_access(&state)
        .await
        .with_external_login(Some(ExternalLoginCookie {
            key,
            pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
            csrf_state: csrf_state.secret().to_owned(),
            nonce: None,
            target_url: query.redirect_url,
            error_url: query.error_url,
            remember_me: query.remember_me.unwrap_or(false),
            linked_user: None,
        }));
    assert!(response_session.user_session().is_none());
    PageUtils::new(&state).redirect(response_session, Some(&authorize_url), None)
}
