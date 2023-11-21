use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLoginCookie, OAuth2Client},
    openapi::ApiKind,
};
use axum::{body::HttpBody, extract::State, Extension};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, ValidatedQuery, ValidationError};
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

/// Login or register a new user with the interactive flow using an OAuth2 provider.
async fn oauth2_login(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<Query>, ValidationError>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::ValidationError(error), None),
    };

    // Note: having a token login is not an error, on successful start of the flow, the token cookie is cleared
    // It has some potential issue: if tid is connected to a guest user, the guest may loose all the progress
    if auth_session.user_session.is_some() {
        return state.page_error(auth_session, AuthError::LogoutRequired, query.error_url.as_ref());
    }

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorize_url, csrf_state) = client
        .client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(client.scopes.clone())
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    auth_session.token_cookie = None;
    auth_session.external_login_cookie = Some(ExternalLoginCookie {
        pkce_code_verifier: pkce_code_verifier.secret().to_owned(),
        csrf_state: csrf_state.secret().to_owned(),
        nonce: None,
        target_url: query.redirect_url,
        error_url: query.error_url,
        remember_me: query.remember_me.unwrap_or(false),
        linked_user: None,
    });
    assert!(auth_session.user_session.is_none());

    state.page_redirect(auth_session, &client.provider, Some(&authorize_url))
}

pub fn page_oauth2_login<B>(provider: &str) -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage(provider, "/login"), oauth2_login)
        .with_operation_id(format!("page_{provider}_login"))
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response(
            "Html page to update client cookies and redirect user to start interactive oauth2 login flow",
        )
}
