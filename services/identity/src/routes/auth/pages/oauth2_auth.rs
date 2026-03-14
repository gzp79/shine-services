use crate::{
    app_state::AppState,
    routes::auth::{AuthPage, AuthPageRequest, AuthSession, ExternalLoginCookie, ExternalLoginError, OAuth2Client},
};
use axum::{extract::State, Extension};
use oauth2::{AuthorizationCode, PkceCodeVerifier, TokenResponse};
use serde::Deserialize;
use shine_infra::web::{
    extracts::{ClientFingerprint, InputError, SiteInfo, ValidatedQuery},
    responses::ErrorResponse,
};
use std::sync::Arc;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    code: String,
    state: String,
}

/// Process the authentication redirect from the OAuth2 provider.
#[utoipa::path(
    get,
    path = "/auth",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Complete the OAuth2 login flow")
    )
)]
pub async fn oauth2_auth(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
) -> AuthPage {
    // 1. Extract external login cookie
    let ExternalLoginCookie {
        pkce_code_verifier,
        csrf_state,
        target_url: redirect_url,
        error_url,
        remember_me,
        linked_user,
        ..
    } = match auth_session.external_login() {
        Some(external_login) => external_login.clone(),
        None => {
            return state
                .auth_page_handler()
                .error(auth_session, ExternalLoginError::MissingExternalLoginCookie, None)
        }
    };
    let auth_session = auth_session.with_external_login(None);

    // 2. Create request helper
    let req = AuthPageRequest::new(&state, auth_session);

    // 3. Validate query
    let query = match req.validate_query(query) {
        Ok(q) => q,
        Err(page) => return page,
    };

    // 4. Validate redirect URLs
    if let Some(page) = req.validate_redirect_urls(redirect_url.as_ref(), error_url.as_ref()) {
        return page;
    }

    log::debug!("Query: {query:#?}");

    // 5. Business logic - OAuth2 authentication
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::debug!("CSRF test failed: [{csrf_state}], [{auth_csrf_state}]");
        return req.error_page(ExternalLoginError::InvalidCSRF, error_url.as_ref());
    }

    // Exchange the code with a token.
    let token = match client
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_code_verifier))
        .request_async(&client.http_client)
        .await
    {
        Ok(token) => token,
        Err(err) => {
            log::warn!("Token exchange error: {err:?}");
            return req.error_page(
                ExternalLoginError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
            );
        }
    };

    let external_user = match client
        .get_external_user_info(
            &req.state().settings().app_name,
            client.user_info_url.url().clone(),
            &client.provider,
            token.access_token().secret(),
            &client.user_info_mapping,
            &client.extensions,
        )
        .await
    {
        Ok(external_user_info) => external_user_info,
        Err(err) => {
            return req.error_page(
                ExternalLoginError::FailedExternalUserInfo(format!("{err:?}")),
                error_url.as_ref(),
            )
        }
    };

    // 6. Return response
    if linked_user.is_some() {
        state
            .external_login_handler()
            .complete_external_link(
                req.into_auth_session(),
                &external_user,
                redirect_url.as_ref(),
                error_url.as_ref(),
            )
            .await
    } else {
        state
            .external_login_handler()
            .complete_external_login(
                req.into_auth_session(),
                fingerprint,
                &site_info,
                &external_user,
                redirect_url.as_ref(),
                error_url.as_ref(),
                remember_me,
            )
            .await
    }
}
