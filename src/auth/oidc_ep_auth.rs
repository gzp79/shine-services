use crate::auth::{
    create_ooops_page, external_auth_create_user, external_auth_helper::ExternalAuthError, external_auth_link_user,
    oidc_client::OIDCClient, AuthServiceState, AuthSession, ExternalLogin, ExternalUserInfo,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Extension,
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier};
use openidconnect::{Nonce, TokenResponse};
use serde::Deserialize;
use std::sync::Arc;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
enum Error {
    #[error("Missing external login cookie")]
    MissingExternalLogin,
    #[error("Linking information is inconsistent with the user session and external login")]
    InconsistentSessionLinking,
    #[error("Cross-Site Request Forgery (Csrf) check failed")]
    InvalidCsrfState,
    #[error("Failed to exchange authorization code to access token: {0}")]
    FailedTokenExchange(String),
    #[error("Failed to verify id token: {0}")]
    FailedIdVerification(String),
    #[error("Missing id token, consider using oauth2 instead of OpenId")]
    MissingIdToken,
    #[error("Missing nonce from external session")]
    MissingNonce,
    #[error(transparent)]
    ExternalAuthError(#[from] ExternalAuthError),
    #[error(transparent)]
    TeraError(#[from] tera::Error),
}

#[derive(Deserialize)]
pub(in crate::auth) struct AuthRequest {
    code: String,
    state: String,
}

async fn openid_connect_auth_impl(
    state: &AuthServiceState,
    client: &OIDCClient,
    query: AuthRequest,
    auth_session: &mut AuthSession,
) -> Result<Html<String>, Error> {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let ExternalLogin {
        pkce_code_verifier,
        csrf_state,
        nonce,
        target_url,
        link_session_id,
    } = auth_session.external_login.take().ok_or(Error::MissingExternalLogin)?;

    let nonce = nonce.ok_or(Error::MissingNonce)?;

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::info!("{csrf_state} vs {auth_csrf_state}");
        return Err(Error::InvalidCsrfState);
    }

    // Exchange the code with a token.
    let token = client
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_code_verifier))
        .request_async(async_http_client)
        .await
        .map_err(|err| Error::FailedTokenExchange(format!("{err:?}")))?;

    let id_token = token.id_token().ok_or(Error::MissingIdToken)?;
    // extract claims from the returned (jwt) token
    let claims = id_token
        .claims(&client.client.id_token_verifier(), &Nonce::new(nonce))
        .map_err(|err| Error::FailedIdVerification(format!("{err}")))?;
    log::debug!("Code exchange completed, claims: {claims:#?}");

    let external_id = claims.subject().to_string();
    let name = claims
        .nickname()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str().to_owned());
    let email = claims.email().map(|n| n.as_str().to_owned());

    let external_user_info = ExternalUserInfo {
        external_id,
        name,
        email,
    };
    log::info!("{:?}", external_user_info);

    match (&auth_session.user, &link_session_id) {
        (Some(current_user), Some(linked_user)) => {
            // try to link account
            let html = external_auth_link_user(
                state,
                current_user,
                linked_user,
                &client.provider,
                &external_user_info,
                target_url.as_deref(),
            )
            .await?;
            Ok(html)
        }
        (None, None) => {
            // try to create a new user
            let (user, html) =
                external_auth_create_user(state, &client.provider, &external_user_info, target_url.as_deref()).await?;
            auth_session.user = Some(user);
            Ok(html)
        }
        _ => Err(Error::InconsistentSessionLinking),
    }
}

pub(in crate::auth) async fn openid_connect_auth(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OIDCClient>>,
    Query(query): Query<AuthRequest>,
    mut auth_session: AuthSession,
) -> Response {
    log::info!("auth_session: {auth_session:?}");

    match openid_connect_auth_impl(&state, &client, query, &mut auth_session).await {
        Ok(html) => {
            log::debug!("Session is ready: {:#?}", auth_session.user);
            (auth_session, html).into_response()
        }
        err @ Err(Error::MissingExternalLogin)
        | err @ Err(Error::InvalidCsrfState)
        | err @ Err(Error::InconsistentSessionLinking)
        | err @ Err(Error::MissingNonce)
        | err @ Err(Error::ExternalAuthError(ExternalAuthError::CompromisedSessions(_))) => {
            log::info!("Session is corrupted: {err:?}");
            let html = create_ooops_page(&state, Some("Session is corrupted, clearing stored sessions"));
            let _ = auth_session.take();
            (StatusCode::FORBIDDEN, auth_session, html).into_response()
        }
        Err(err) => {
            let html = create_ooops_page(&state, Some(&format!("{err}")));
            // Keep only the current_user intact
            (StatusCode::INTERNAL_SERVER_ERROR, auth_session, html).into_response()
        }
    }
}
