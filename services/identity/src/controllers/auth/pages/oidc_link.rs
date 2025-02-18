use crate::{
    app_state::AppState,
    controllers::auth::{
        AuthError, AuthPage, AuthSession, ExternalLoginCookie, ExternalLoginError, OIDCClient, PageUtils,
    },
};
use axum::{extract::State, Extension};
use chrono::Duration;
use oauth2::{CsrfToken, PkceCodeChallenge};
use openidconnect::{
    core::{CoreAuthPrompt, CoreAuthenticationFlow},
    Nonce,
};
use serde::Deserialize;
use shine_core::web::{ErrorResponse, InputError, ValidatedQuery};
use std::sync::Arc;
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    redirect_url: Option<Url>,
    error_url: Option<Url>,
}

/// Link the current user to an OpenId Connect provider.
#[utoipa::path(
    get,
    path = "/link",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Html page to update client cookies and redirect user to start interactive OpenIdConnect login flow")
    )
)]
pub async fn oidc_link(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None),
    };

    if auth_session.user_session().is_none() {
        return PageUtils::new(&state).error(auth_session, AuthError::LoginRequired, query.error_url.as_ref());
    }

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::OIDCDiscovery(format!("{err:#?}")),
                query.error_url.as_ref(),
            )
        }
    };

    let key = match state.token_service().generate() {
        Ok(key) => key,
        Err(err) => return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref()),
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

    let linked_user = auth_session.user_session().cloned();
    let response_session = auth_session.with_external_login(Some(ExternalLoginCookie {
        key,
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: Some(nonce.secret().to_owned()),
        target_url: query.redirect_url,
        error_url: query.error_url,
        remember_me: false,
        linked_user,
    }));
    assert!(response_session.user_session().is_some());
    PageUtils::new(&state).redirect(response_session, Some(&client.provider), Some(&authorize_url))
}
