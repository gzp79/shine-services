use crate::{
    auth::{
        create_ooops_page, external_auth_user, get_external_user_info, oauth2_client::OAuth2Client, AuthServiceState,
        ExternalAuthError, ExternalLoginData, ExternalLoginSession, ExternalUserInfoError,
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
use shine_service::service::{CurrentUser, UserSession};
use std::sync::Arc;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
enum Error {
    #[error("Session cookie was missing or corrupted")]
    MissingSession,
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
    oauth2_client: &OAuth2Client,
    query: AuthRequest,
    current_user: Option<CurrentUser>,
    external_login_data: Option<ExternalLoginData>,
) -> Result<(CurrentUser, Html<String>), Error> {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let external_login_data = external_login_data.ok_or(Error::MissingSession)?;
    let (pkce_code_verifier, csrf_state, target_url, linked_user) = match external_login_data {
        ExternalLoginData::OIDCLogin {
            pkce_code_verifier,
            csrf_state,
            target_url,
            link_session_id,
            ..
        } => (
            PkceCodeVerifier::new(pkce_code_verifier),
            csrf_state,
            target_url,
            link_session_id,
        ),
        //_ => return Err(AuthServiceError::InvalidSession),
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        log::info!("{csrf_state} vs {auth_csrf_state}");
        return Err(Error::InvalidCsrfState);
    }

    // Exchange the code with a token.
    let token = oauth2_client
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|err| Error::FailedTokenExchange(format!("{err:?}")))?;

    let external_user_info = get_external_user_info(
        oauth2_client.user_info_url.url().clone(),
        token.access_token().secret(),
        &oauth2_client.user_info_mapping,
        &oauth2_client.extensions,
    )
    .await?;
    log::info!("{:?}", external_user_info);

    let result = external_auth_user(
        state,
        current_user,
        linked_user,
        &oauth2_client.provider,
        external_user_info,
        target_url.as_deref(),
    )
    .await?;

    Ok(result)
}

pub(in crate::auth) async fn oauth2_connect_auth(
    State(state): State<AuthServiceState>,
    Extension(oauth2_client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<AuthRequest>,
    mut user_session: UserSession,
    mut external_login_session: ExternalLoginSession,
) -> Response {
    log::info!("user_session: {user_session:?}");
    log::info!("external_login: {external_login_session:?}");

    match openid_connect_auth_impl(
        &state,
        &oauth2_client,
        query,
        user_session.take(),
        external_login_session.take(),
    )
    .await
    {
        Ok((current_user, html)) => {
            log::debug!("Session is ready: {user_session:#?}");
            user_session.set(current_user);
            // clear external_login_session and set a new user_session
            (external_login_session, user_session, html).into_response()
        }
        Err(err) => {
            let html = create_ooops_page(&state, Some(&format!("{err}")));
            // clear external_login_session, but keep user_session intact
            (StatusCode::INTERNAL_SERVER_ERROR, external_login_session, html).into_response()
        }
    }
}
