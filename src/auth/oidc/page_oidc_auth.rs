use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession, ExternalLogin, OIDCClient},
    db::ExternalUserInfo,
    openapi::ApiKind,
};
use axum::{body::HttpBody, extract::State, Extension};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier};
use openidconnect::{Nonce, TokenResponse};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, ValidatedQuery, ValidationError},
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

/// Process the authentication redirect from the OpenID Connect provider.
async fn oidc_auth(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    mut auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    query: Result<ValidatedQuery<Query>, ValidationError>,
) -> AuthPage {
    // take external_login from session, thus later code don't have to care with it
    let ExternalLogin {
        pkce_code_verifier,
        csrf_state,
        nonce,
        target_url,
        error_url,
        remember_me,
        linked_user,
    } = match auth_session.external_login.take() {
        Some(external_login) => external_login,
        None => return state.page_error(auth_session, AuthError::MissingExternalLogin, None),
    };

    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::ValidationError(error), error_url.as_ref()),
    };
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => return state.page_error(auth_session, AuthError::OIDCDiscovery(err), error_url.as_ref()),
    };

    let nonce = match nonce {
        Some(nonce) => nonce,
        None => return state.page_error(auth_session, AuthError::MissingNonce, error_url.as_ref()),
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::debug!("CSRF test failed: [{csrf_state}], [{auth_csrf_state}]");
        return state.page_error(auth_session, AuthError::InvalidCSRF, error_url.as_ref());
    }

    // Exchange the code with a token.
    let token = match core_client
        .exchange_code(auth_code)
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_code_verifier))
        .request_async(async_http_client)
        .await
    {
        Ok(token) => token,
        Err(err) => {
            log::warn!("Token exchange error: {err:#?}");
            return state.page_error(
                auth_session,
                AuthError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
            );
        }
    };

    let claims = match token.id_token().and_then(|id_token| {
        id_token
            .claims(&core_client.id_token_verifier(), &Nonce::new(nonce))
            .ok()
    }) {
        Some(claims) => claims,
        _ => return state.page_error(auth_session, AuthError::FailedExternalUserInfo, error_url.as_ref()),
    };
    log::debug!("Code exchange completed, claims: {claims:#?}");

    let external_user = {
        let external_id = claims.subject().to_string();
        let name = claims
            .nickname()
            .and_then(|n| n.get(None))
            .map(|n| n.as_str().to_owned());
        let email = claims.email().map(|n| n.as_str().to_owned());

        ExternalUserInfo {
            provider: client.provider.clone(),
            provider_id: external_id,
            name,
            email,
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
                &external_user,
                target_url.as_ref(),
                error_url.as_ref(),
                remember_me,
            )
            .await
    }
}

pub fn page_oidc_auth<B>(provider: &str) -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage(provider, "/auth"), oidc_auth)
        .with_operation_id(format!("page_{provider}_auth"))
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update client cookies and complete the OpenIdConnect login flow")
}
