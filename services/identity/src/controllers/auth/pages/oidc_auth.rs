use std::str::FromStr;

use crate::{
    app_state::AppState,
    controllers::auth::{
        AuthPage, AuthSession, ExternalLoginCookie, ExternalLoginError, LinkUtils, OIDCClient, PageUtils,
    },
    repositories::identity::ExternalUserInfo,
};
use axum::{
    extract::{Path, State},
    Extension,
};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;
use oauth2::{AuthorizationCode, PkceCodeVerifier};
use openidconnect::{Nonce, TokenResponse};
use serde::Deserialize;
use shine_infra::web::{
    extracts::{ClientFingerprint, InputError, SiteInfo, ValidatedQuery},
    responses::ErrorResponse,
};
use openidconnect::{
    core::{CoreGenderClaim, CoreIdToken},
    EmptyAdditionalClaims, IdTokenClaims,
};
use std::sync::Arc;
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

async fn complete_oidc_login(
    state: &AppState,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    client: &OIDCClient,
    claims: &IdTokenClaims<EmptyAdditionalClaims, CoreGenderClaim>,
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    remember_me: bool,
) -> AuthPage {
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
    log::info!("{external_user:?}");

    LinkUtils::new(state)
        .complete_external_login(
            auth_session,
            fingerprint,
            &site_info,
            &external_user,
                redirect_url.as_ref(),
                error_url.as_ref(),
            remember_me,
        )
        .await
}

#[derive(Deserialize, Validate, IntoParams)]
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
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::MissingExternalLoginCookie,
                None,
                None,
            )
        }
    };
    let auth_session = auth_session.with_external_login(None);

    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => {
            return PageUtils::new(&state).error(auth_session, error.problem, error_url.as_ref(), redirect_url.as_ref())
        }
    };
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::OIDCDiscovery(format!("{err}")),
                error_url.as_ref(),
                redirect_url.as_ref(),
            )
        }
    };

    let nonce = match nonce {
        Some(nonce) => nonce,
        None => {
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::MissingNonce,
                error_url.as_ref(),
                redirect_url.as_ref(),
            )
        }
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::debug!("CSRF test failed: [{csrf_state}], [{auth_csrf_state}]");
        return PageUtils::new(&state).error(
            auth_session,
            ExternalLoginError::InvalidCSRF,
            error_url.as_ref(),
            redirect_url.as_ref(),
        );
    }

    // Exchange the code with a token.
    let exchange_request = match core_client.exchange_code(auth_code) {
        Ok(request) => request,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
                redirect_url.as_ref(),
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
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::TokenExchangeFailed(format!("{err:#?}")),
                error_url.as_ref(),
                redirect_url.as_ref(),
            );
        }
    };

    let claims = match token
        .id_token()
        .ok_or("Missing id_token".to_string())
        .and_then(|id_token| {
            id_token
                .claims(&core_client.id_token_verifier(), &Nonce::new(nonce))
                .map_err(|err| format!("{err:?}"))
        }) {
        Ok(claims) => claims,
        Err(err) => {
            log::error!("{err:?}");
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::FailedExternalUserInfo(format!("{err:#?}")),
                error_url.as_ref(),
                redirect_url.as_ref(),
            );
        }
    };
    log::debug!("Code exchange completed, claims: {claims:#?}");

    if linked_user.is_some() {
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

        LinkUtils::new(&state)
            .complete_external_link(auth_session, &external_user, redirect_url.as_ref(), error_url.as_ref())
            .await
    } else {
        complete_oidc_login(
            &state,
            auth_session,
            fingerprint,
            site_info,
            &client,
            &claims,
            redirect_url,
            error_url,
            remember_me,
        )
        .await
    }
}

/// Perform OpenID Connect login through a provided id_token. This is not a standard OIDC flow.
#[utoipa::path(
    get,
    path = "/auth/oidc/{provider}/id_token",
    tag = "page",
    params(
        ("provider" = String, Path, description = "The OpenID Connect provider to be used for login"),
    ),
    responses(
        (status = OK, description="Complete the OpenID Connect login flow")
    )
)]
pub async fn oidc_id_token_login(
    State(state): State<AppState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    Path(provider): Path<String>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
) -> AuthPage {
    let config = state.app_config().auth.openid.get(&provider);
    if config.is_none() || !config.unwrap().enable_id_token_login {
        return PageUtils::new(&state).error(
            auth_session,
            ExternalLoginError::ProviderNotAllowed(provider),
            None,
            None,
        );
    }

    let core_client = match client.client().await {
        Ok(client) => client,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::OIDCDiscovery(format!("{err}")),
                None,
                None,
            )
        }
    };

    let id_token: CoreIdToken =
        match openidconnect::IdToken::from_str(authorization.token()) {
            Ok(id_token) => id_token,
            Err(err) => {
                return PageUtils::new(&state).error(
                    auth_session,
                    ExternalLoginError::FailedExternalUserInfo(format!("{err:#?}")),
                    None,
                    None,
                );
            }
        };

    let claims = match id_token.claims(&core_client.id_token_verifier(), &Nonce::new("dummy".to_string())) {
        Ok(claims) => claims,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                ExternalLoginError::FailedExternalUserInfo(format!("{err:#?}")),
                None,
                None,
            );
        }
    };

    log::debug!("Code exchange completed, claims: {claims:#?}");

    complete_oidc_login(
        &state,
        auth_session,
        fingerprint,
        site_info,
        &client,
        &claims,
        None,
        None,
        true,
    )
    .await
}
