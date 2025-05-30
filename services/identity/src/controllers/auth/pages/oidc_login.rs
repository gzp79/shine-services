use crate::{
    app_state::AppState,
    controllers::auth::{
        AuthPage, AuthSession, ExternalLoginCookie, ExternalLoginError, OIDCClient, PageUtils,
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

#[derive(Deserialize, Validate, IntoParams)]
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
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None, None),
    };

    if let Err(err) = state
        .captcha_validator()
        .validate(query.captcha.as_deref())
        .await
    {
        return PageUtils::new(&state).error(
            auth_session,
            err,
            query.error_url.as_ref(),
            query.redirect_url.as_ref(),
        );
    }

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::OIDCDiscovery(format!("{err:#?}")),
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
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

    let response_session = auth_session
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
    assert!(response_session.user_session().is_none());
    PageUtils::new(&state).redirect(response_session, Some(&authorize_url), None)
}
