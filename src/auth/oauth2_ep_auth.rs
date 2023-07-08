use crate::{
    auth::{
        create_ooops_page, external_auth_create_user, external_auth_link_user, get_external_user_info,
        oauth2_client::OAuth2Client, AuthServiceState, AuthSession, ExternalAuthError, ExternalLogin,
        ExternalUserInfoError,
    },
    db::NameGeneratorError,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Extension,
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier, TokenResponse};
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
    #[error(transparent)]
    FailedUserInfoQuery(#[from] ExternalUserInfoError),
    #[error(transparent)]
    ExternalAuthError(#[from] ExternalAuthError),
    #[error(transparent)]
    TeraError(#[from] tera::Error),
    #[error(transparent)]
    NameGeneratorError(#[from] NameGeneratorError),
}

#[derive(Deserialize)]
pub(in crate::auth) struct AuthRequest {
    code: String,
    state: String,
}

async fn openid_connect_auth_impl(
    state: &AuthServiceState,
    client: &OAuth2Client,
    query: AuthRequest,
    auth_session: &mut AuthSession,
) -> Result<Html<String>, Error> {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let ExternalLogin {
        pkce_code_verifier,
        csrf_state,
        target_url,
        link_session_id,
        ..
    } = auth_session.external_login.take().ok_or(Error::MissingExternalLogin)?;

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

    let external_user_info = get_external_user_info(
        client.user_info_url.url().clone(),
        token.access_token().secret(),
        &client.user_info_mapping,
    )
    .await?;
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

pub(in crate::auth) async fn oauth2_connect_auth(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<AuthRequest>,
    mut auth_session: AuthSession,
) -> Response {
    match openid_connect_auth_impl(&state, &client, query, &mut auth_session).await {
        Ok(html) => {
            log::debug!("Session is ready: {:#?}", auth_session.user);
            (auth_session, html).into_response()
        }
        err @ Err(Error::MissingExternalLogin)
        | err @ Err(Error::InvalidCsrfState)
        | err @ Err(Error::InconsistentSessionLinking)
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
