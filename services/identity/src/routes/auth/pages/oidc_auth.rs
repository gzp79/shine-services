use crate::{
    app_state::AppState,
    routes::auth::{
        AuthPage, AuthPageRequest, AuthSession, AuthUtils, ExternalLoginCookie, ExternalLoginError, OIDCClient,
        OIDCUserInfoExtractor, PageUtils,
    },
};
use axum::{extract::State, Extension};
use oauth2::{AuthorizationCode, PkceCodeVerifier};
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

/// Process the authentication redirect from the OpenID Connect provider.
#[utoipa::path(
    get,
    path = "/auth",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Complete the OenID Connect login flow")
    )
)]
pub async fn oidc_auth(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
) -> AuthPage {
    // 1. Extract external login cookie
    let ExternalLoginCookie {
        pkce_code_verifier,
        csrf_state,
        nonce,
        target_url: redirect_url,
        error_url,
        remember_me,
        linked_user,
        ..
    } = match auth_session.external_login() {
        Some(external_login_cookie) => external_login_cookie.clone(),
        None => {
            return PageUtils::new(&state).error(auth_session, ExternalLoginError::MissingExternalLoginCookie, None)
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

    // 5. Business logic - OIDC authentication
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => return req.error_page(ExternalLoginError::OIDCDiscovery(format!("{err}")), error_url.as_ref()),
    };

    let nonce = match nonce {
        Some(nonce) => nonce,
        None => return req.error_page(ExternalLoginError::MissingNonce, error_url.as_ref()),
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::debug!("CSRF test failed: [{csrf_state}], [{auth_csrf_state}]");
        return req.error_page(ExternalLoginError::InvalidCSRF, error_url.as_ref());
    }

    // Exchange the code with a token.
    let exchange_request = match core_client.exchange_code(auth_code) {
        Ok(request) => request,
        Err(err) => {
            return req.error_page(
                ExternalLoginError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
            )
        }
    };
    let token = match exchange_request
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_code_verifier))
        .request_async(&client.http_client)
        .await
    {
        Ok(token) => token,
        Err(err) => {
            log::warn!("Token exchange error: {err:#?}");
            return req.error_page(
                ExternalLoginError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
            );
        }
    };

    let external_user = match core_client.get_external_user_info(client.provider.clone(), &token, nonce) {
        Ok(external_user) => external_user,
        Err(err) => {
            log::error!("{err:?}");
            return req.error_page(err, error_url.as_ref());
        }
    };

    // 6. Return response
    if linked_user.is_some() {
        AuthUtils::new(&state)
            .complete_external_link(
                req.into_auth_session(),
                &external_user,
                redirect_url.as_ref(),
                error_url.as_ref(),
            )
            .await
    } else {
        AuthUtils::new(&state)
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
