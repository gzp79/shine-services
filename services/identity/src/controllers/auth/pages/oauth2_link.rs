use crate::controllers::{
    auth::{AuthError, AuthPage, AuthSession, ExternalLoginCookie, OAuth2Client, PageUtils},
    ApiKind, AppState,
};
use axum::{extract::State, Extension};
use oauth2::{CsrfToken, PkceCodeChallenge};
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, ConfiguredProblem, InputError, OpenApiUrl, ValidatedQuery};
use std::sync::Arc;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct QueryParams {
    redirect_url: Option<OpenApiUrl>,
    error_url: Option<OpenApiUrl>,
}

/// Link the current user to an OAuth2 provider.
async fn oauth2_link(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ConfiguredProblem<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, AuthError::InputError(error.problem), None),
    };

    if auth_session.user_session.is_none() {
        return PageUtils::new(&state).error(auth_session, AuthError::LoginRequired, query.error_url.as_deref());
    }

    let key = match state.token_service().generate() {
        Ok(key) => key,
        Err(err) => return PageUtils::new(&state).internal_error(auth_session, err, query.error_url.as_deref()),
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
        target_url: query.redirect_url.map(|url| url.into_url()),
        error_url: query.error_url.map(|url| url.into_url()),
        remember_me: false,
        linked_user: auth_session.user_session.clone(),
    });

    PageUtils::new(&state).redirect(auth_session, Some(&client.provider), Some(&authorize_url))
}

pub fn page_oauth2_link(provider: &str) -> ApiEndpoint<AppState> {
    ApiEndpoint::new(
        ApiMethod::Get,
        ApiKind::Page(&format!("/auth/{provider}/link")),
        oauth2_link,
    )
    .with_operation_id(format!("{provider}_link"))
    .with_tag("page")
    .with_query_parameter::<QueryParams>()
    .with_page_response("Html page to update client cookies and redirect user to start interactive oauth2 login flow")
}
