use crate::{
    auth::{
        token::TokenGenerator, AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLoginCookie, OAuth2Client,
    },
    openapi::ApiKind,
};
use axum::{extract::State, Extension};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, InputError, ProblemDetail, ValidatedQuery};
use std::sync::Arc;
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    #[param(value_type = Option<String>)]
    redirect_url: Option<Url>,
    #[param(value_type = Option<String>)]
    error_url: Option<Url>,
}

/// Link the current user to an OAuth2 provider.
async fn oauth2_link(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<Query>, ProblemDetail<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::InputError(error.problem), None),
    };

    if auth_session.user_session.is_none() {
        return state.page_error(auth_session, AuthError::LoginRequired, query.error_url.as_ref());
    }

    let key = match TokenGenerator::new(state.random()).generate() {
        Ok(key) => key,
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
    };

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    auth_session.external_login_cookie = Some(ExternalLoginCookie {
        key,
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect_url,
        error_url: query.error_url,
        remember_me: false,
        linked_user: auth_session.user_session.clone(),
    });

    state.page_redirect(auth_session, &client.provider, Some(&authorize_url))
}

pub fn page_oauth2_link(provider: &str) -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage(provider, "/link"), oauth2_link)
        .with_operation_id(format!("page_{provider}_link"))
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response(
            "Html page to update client cookies and redirect user to start interactive oauth2 login flow",
        )
}
