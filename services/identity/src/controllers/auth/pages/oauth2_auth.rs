use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession, ExternalLoginCookie, LinkUtils, OAuth2Client, PageUtils},
};
use axum::{extract::State, Extension};
use oauth2::{AuthorizationCode, PkceCodeVerifier, TokenResponse};
use serde::Deserialize;
use shine_core::web::{
    ApiKind, ApiMethod, ClientFingerprint, ConfiguredProblem, InputError, SiteInfo, ValidatedQuery, WebRoute,
};
use std::sync::Arc;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct QueryParams {
    code: String,
    state: String,
}

/// Process the authentication redirect from the OAuth2 provider.
async fn oauth2_auth(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    mut auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    query: Result<ValidatedQuery<QueryParams>, ConfiguredProblem<InputError>>,
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
        None => return PageUtils::new(&state).error(auth_session, AuthError::MissingExternalLoginCookie, None),
    };

    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => {
            return PageUtils::new(&state).error(auth_session, AuthError::InputError(error.problem), error_url.as_ref())
        }
    };
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::debug!("CSRF test failed: [{csrf_state}], [{auth_csrf_state}]");
        return PageUtils::new(&state).error(auth_session, AuthError::InvalidCSRF, error_url.as_ref());
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
            return PageUtils::new(&state).error(
                auth_session,
                AuthError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
            );
        }
    };

    let external_user = match client
        .get_external_user_info(
            &state.settings().app_name,
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
            return PageUtils::new(&state).error(
                auth_session,
                AuthError::FailedExternalUserInfo(format!("{err:?}")),
                error_url.as_ref(),
            )
        }
    };
    log::info!("{:?}", external_user);

    if linked_user.is_some() {
        LinkUtils::new(&state)
            .complete_external_link(auth_session, &external_user, target_url.as_ref(), error_url.as_ref())
            .await
    } else {
        LinkUtils::new(&state)
            .complete_external_login(
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

pub fn page_oauth2_auth(provider: &str) -> WebRoute<AppState> {
    WebRoute::new(
        ApiMethod::Get,
        ApiKind::Page(&format!("/auth/{provider}/auth")),
        oauth2_auth,
    )
    .with_operation_id(format!("{provider}_auth"))
    tag = "page"
    params( 
QueryParans
),
    response(
(status = OK, description="Html page to update client cookies and complete the oauth2 login flow")
}
