use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLoginCookie, OAuth2Client},
    openapi::ApiKind,
};
use axum::{extract::State, Extension};
use oauth2::{AuthorizationCode, PkceCodeVerifier, TokenResponse};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, InputError, SiteInfo, ValidatedQuery},
    service::ClientFingerprint,
};
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
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    query: Result<ValidatedQuery<Query>, InputError>,
) -> AuthPage {
    // take external_login_cookie from session, thus later code don't have to care with it
    let ExternalLoginCookie {
        pkce_code_verifier,
        csrf_state,
        target_url,
        error_url,
        remember_me,
        linked_user,
        ..
    } = match auth_session.external_login_cookie.take() {
        Some(external_login_cookie) => external_login_cookie,
        None => return state.page_error(auth_session, AuthError::MissingExternalLoginCookie, None),
    };

    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::InputError(error), error_url.as_ref()),
    };
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

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
        .request_async(|r| async { client.send_request(r).await })
        .await
    {
        Ok(token) => token,
        Err(err) => {
            log::warn!("Token exchange error: {err:?}");
            return state.page_error(
                auth_session,
                AuthError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
            );
        }
    };

    let external_user = match state
        .get_external_user_info(
            &client.http_client,
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
            return state.page_error(
                auth_session,
                AuthError::FailedExternalUserInfo(format!("{err:?}")),
                error_url.as_ref(),
            )
        }
    };
    log::info!("{:?}", external_user);

    if linked_user.is_some() {
        state
            .page_external_link(auth_session, &external_user, target_url.as_ref(), error_url.as_ref())
            .await
    } else {
        state
            .page_external_login(
                auth_session,
                fingerprint,
                &site_info,
                &external_user,
                target_url.as_ref(),
                error_url.as_ref(),
                remember_me,
            )
            .await
    }
}

pub fn page_oauth2_auth(provider: &str) -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage(provider, "/auth"), oauth2_auth)
        .with_operation_id(format!("page_{provider}_auth"))
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update client cookies and complete the oauth2 login flow")
}
