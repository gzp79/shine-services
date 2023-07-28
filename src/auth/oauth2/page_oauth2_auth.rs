use crate::{
    auth::{get_external_user_info, AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLogin, OAuth2Client},
    openapi::ApiKind,
};
use axum::{body::HttpBody, extract::State, Extension};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier, TokenResponse};
use serde::Deserialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, ValidatedQuery};
use std::sync::Arc;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    code: String,
    state: String,
}

/// Process the authentication redirect from the OAuth2 provider.
async fn oauth2_auth(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    mut auth_session: AuthSession,
    ValidatedQuery(query): ValidatedQuery<Query>,
) -> AuthPage {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    // take external_login from session, thus later code don't have to care with it
    let ExternalLogin {
        pkce_code_verifier,
        csrf_state,
        target_url,
        error_url,
        remember_me,
        linked_user,
        ..
    } = match auth_session.external_login.take() {
        Some(external_login) => external_login,
        None => return state.page_error(auth_session, AuthError::MissingExternalLogin, None),
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::debug!("CSRF test failed: [{csrf_state}], [{auth_csrf_state}]");
        return state.page_error(auth_session, AuthError::InvalidCSRF, error_url.as_ref());
    }

    // Exchange the code with a token.
    let token = match client
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_code_verifier))
        .request_async(async_http_client)
        .await
    {
        Ok(token) => token,
        Err(err) => return state.page_internal_error(auth_session, err, error_url.as_ref()),
    };

    let external_user_info = match get_external_user_info(
        client.user_info_url.url().clone(),
        &client.provider,
        token.access_token().secret(),
        &client.user_info_mapping,
        &client.extensions,
    )
    .await
    {
        Ok(external_user_info) => external_user_info,
        _ => return state.page_error(auth_session, AuthError::FailedExternalUserInfo, error_url.as_ref()),
    };
    log::info!("{:?}", external_user_info);

    if linked_user.is_some() {
        state
            .page_external_link(
                auth_session,
                &client.provider,
                &external_user_info.provider_id,
                target_url.as_ref(),
                error_url.as_ref(),
            )
            .await
    } else {
        state
            .page_external_login(
                auth_session,
                external_user_info,
                target_url.as_ref(),
                error_url.as_ref(),
                remember_me,
            )
            .await
    }
}

pub fn page_oauth2_auth<B>(provider: &str) -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage(provider, "/auth"), oauth2_auth)
        .with_operation_id(format!("page_{provider}_auth"))
        .with_tag("login")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update client cookies and complete the oauth2 login flow")
}
