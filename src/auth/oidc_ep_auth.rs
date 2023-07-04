use crate::{
    auth::{create_ooops_page, create_redirect_page, oidc_client::OIDCClient, ExternalLoginData, ExternalLoginSession},
    db::{
        CreateIdentityError, DBError, DBSessionError, ExternalLogin, FindIdentity, IdentityManager, LinkIdentityError,
        NameGenerator, NameGeneratorError, SessionManager, SettingsManager,
    },
};
use axum::{
    extract::Query,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    Extension,
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier};
use openidconnect::{Nonce, TokenResponse};
use serde::Deserialize;
use shine_service::service::{CurrentUser, UserSession, APP_NAME};
use std::sync::Arc;
use tera::Tera;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
enum Error {
    #[error("Session cookie was missing or corrupted")]
    MissingSession,
    #[error("Cross Server did not return an ID token")]
    InvalidCsrfState,
    #[error("Session and external login cookies are not matching")]
    InconsistentSession,
    #[error("Failed to exchange authorization code to access token: {0}")]
    FailedTokenExchange(String),
    #[error("Cross-Site Request Forgery (Csrf) check failed")]
    MissingIdToken,
    #[error("Failed to verify id token: {0}")]
    FailedIdVerification(String),

    #[error("Email already used by an user")]
    LinkEmailConflict,
    #[error("Provider already linked to an user")]
    LinkProviderConflict,

    #[error("Failed to create session")]
    DBSession(#[from] DBSessionError),
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error(transparent)]
    TeraError(#[from] tera::Error),
    #[error(transparent)]
    NameGeneratorError(#[from] NameGeneratorError),
}

#[derive(Deserialize)]
pub(in crate::auth) struct AuthRequest {
    code: String,
    state: String,
    //scope: String,
}

#[allow(clippy::too_many_arguments)]
async fn openid_connect_auth_impl(
    oidc_client: &OIDCClient,
    tera: &Tera,
    settings_manager: &SettingsManager,
    identity_manager: &IdentityManager,
    session_manager: &SessionManager,
    name_generator: &NameGenerator,
    query: AuthRequest,
    current_user: Option<CurrentUser>,
    external_login_data: Option<ExternalLoginData>,
) -> Result<(CurrentUser, Html<String>), Error> {
    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let external_login_data = external_login_data.ok_or(Error::MissingSession)?;
    let (pkce_code_verifier, csrf_state, nonce, target_url, link_session_id) = match external_login_data {
        ExternalLoginData::OIDCLogin {
            pkce_code_verifier,
            csrf_state,
            nonce,
            target_url,
            link_session_id,
        } => (
            PkceCodeVerifier::new(pkce_code_verifier),
            csrf_state,
            Nonce::new(nonce),
            target_url,
            link_session_id,
        ),
        //_ => return Err(AuthServiceError::InvalidSession),
    };

    // Check for Cross Site Request Forgery
    if csrf_state != auth_csrf_state {
        return Err(Error::InvalidCsrfState);
    }

    // Exchange the code with a token.
    let token = oidc_client
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|err| Error::FailedTokenExchange(format!("{err:?}")))?;

    let id_token = token.id_token().ok_or(Error::MissingIdToken)?;
    let claims = id_token
        .claims(&oidc_client.client.id_token_verifier(), &nonce)
        .map_err(|err| Error::FailedIdVerification(format!("{err}")))?;
    log::debug!("Code exchange completed, claims: {claims:#?}");

    let mut nickname = claims
        .nickname()
        .and_then(|n| n.get(None))
        .map(|n| n.as_str().to_owned());
    let email = claims.email().map(|n| n.as_str().to_owned());
    let provider_id = claims.subject().as_str().to_owned();
    let external_login = ExternalLogin {
        provider: oidc_client.provider.clone(),
        provider_id,
    };

    // find any user linked to this account
    if let Some(link_session_id) = link_session_id {
        // Link the current user to an external provider
        let current_user = current_user.ok_or(Error::InconsistentSession)?;
        if current_user.user_id != link_session_id.user_id || current_user.key != link_session_id.key {
            return Err(Error::InconsistentSession);
        }

        match identity_manager.link_user(current_user.user_id, &external_login).await {
            Ok(()) => {}
            Err(LinkIdentityError::LinkProviderConflict) => return Err(Error::LinkProviderConflict),
            Err(LinkIdentityError::DBError(err)) => return Err(Error::DBError(err)),
        };

        log::debug!("Link ready: {current_user:#?}");
        let html = create_redirect_page(tera, settings_manager, "Redirecting", APP_NAME, target_url.as_deref());
        Ok((current_user, html))
    } else {
        log::debug!("Finding existing user by external login...");
        let identity = match identity_manager
            .find(FindIdentity::ExternalLogin(&external_login))
            .await?
        {
            Some(identity) => {
                log::debug!("Found: {identity:#?}");
                // Sign in to an existing (linked) account
                identity
            }
            None => {
                // Create a new user.
                let mut retry_count = 10;
                loop {
                    log::debug!("Creating new user; retry: {retry_count:#?}");
                    if retry_count < 0 {
                        return Err(Error::DBError(DBError::RetryLimitReached));
                    }
                    retry_count -= 1;

                    let user_id = Uuid::new_v4();
                    let user_name = match nickname.take() {
                        Some(name) => name,
                        None => name_generator.generate_name().await?,
                    };

                    match identity_manager
                        .create_user(user_id, &user_name, email.as_deref(), Some(&external_login))
                        .await
                    {
                        Ok(identity) => break identity,
                        Err(CreateIdentityError::NameConflict) => continue,
                        Err(CreateIdentityError::UserIdConflict) => continue,
                        Err(CreateIdentityError::LinkEmailConflict) => return Err(Error::LinkEmailConflict),
                        Err(CreateIdentityError::LinkProviderConflict) => return Err(Error::LinkProviderConflict),
                        Err(CreateIdentityError::DBError(err)) => return Err(Error::DBError(err)),
                    };
                }
            }
        };

        log::debug!("Identity ready: {identity:#?}");
        let current_user = session_manager.create(&identity).await?;
        let html = create_redirect_page(tera, settings_manager, "Redirecting", APP_NAME, target_url.as_deref());
        Ok((current_user, html))
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::auth) async fn openid_connect_auth(
    Extension(oidc_client): Extension<Arc<OIDCClient>>,
    Extension(tera): Extension<Arc<Tera>>,
    Extension(settings_manager): Extension<SettingsManager>,
    Extension(identity_manager): Extension<IdentityManager>,
    Extension(session_manager): Extension<SessionManager>,
    Extension(name_generator): Extension<NameGenerator>,
    Query(query): Query<AuthRequest>,
    mut user_session: UserSession,
    mut external_login_session: ExternalLoginSession,
) -> Response {
    log::info!("user_session: {user_session:?}");
    log::info!("external_login: {external_login_session:?}");

    match openid_connect_auth_impl(
        &oidc_client,
        &tera,
        &settings_manager,
        &identity_manager,
        &session_manager,
        &name_generator,
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
            let html = create_ooops_page(&tera, &settings_manager, Some(&format!("{err}")));
            // clear external_login_session, but keep user_session intact
            (StatusCode::INTERNAL_SERVER_ERROR, external_login_session, html).into_response()
        }
    }
}
