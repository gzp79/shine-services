use crate::{
    auth::{
        oidc_client::{create_redirect_page, OIDCClient},
        oidc_error::OIDCError,
        ExternalLoginData, ExternalLoginSession,
    },
    db::{
        CreateIdentityError, DBError, ExternalLogin, FindIdentity, IdentityManager, NameGenerator, SessionManager,
        SettingsManager,
    },
};
use axum::{
    extract::Query,
    response::{IntoResponse, Response},
    Extension,
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, PkceCodeVerifier};
use openidconnect::{Nonce, TokenResponse};
use serde::Deserialize;
use shine_service::service::{UserSession, APP_NAME};
use std::sync::Arc;
use tera::Tera;
use uuid::Uuid;

#[derive(Deserialize)]
pub(in crate::auth) struct AuthRequest {
    code: String,
    state: String,
    //scope: String,
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
) -> Result<Response, OIDCError> {
    log::info!("user_session: {user_session:?}");
    log::info!("external_login: {external_login_session:?}");

    let auth_code = AuthorizationCode::new(query.code);
    let auth_csrf_state = query.state;

    let external_login_data = external_login_session.take().ok_or(OIDCError::MissingSession)?;
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
        return Err(OIDCError::InvalidCsrfState);
    }

    // Exchange the code with a token.
    let token = oidc_client
        .client
        .exchange_code(auth_code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|err| OIDCError::FailedTokenExchange(format!("{err:?}")))?;

    let id_token = token.id_token().ok_or(OIDCError::MissingIdToken)?;
    let claims = id_token
        .claims(&oidc_client.client.id_token_verifier(), &nonce)
        .map_err(|err| OIDCError::FailedIdVerification(format!("{err}")))?;
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
    let html = create_redirect_page(&tera, &settings_manager, "Redirecting", APP_NAME, target_url.as_deref())?;

    // find any user linked to this account
    if let Some(link_session_id) = link_session_id {
        // Link the current user an external provider

        //todo: if session.is_none() || session.session_id != link_session_id -> the flow was broken, sign out
        //      else if let Some(identity) find_user_by_link() {
        //         if   identity.id != session.user_id -> linked to a different user else ok
        //      } else { link account to ussr)
        // keep session as it is,
        todo!("Perform linking to the current user/session")
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
                        return Err(OIDCError::DBError(DBError::RetryLimitReached));
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
                        Err(CreateIdentityError::LinkConflict) => todo!("Ask user to log in and link account"),
                        Err(CreateIdentityError::DBError(err)) => return Err(err.into()),
                    };
                }
            }
        };

        log::debug!("Identity ready: {identity:#?}");
        let current_user = session_manager.create(&identity).await?;

        log::debug!("Session is ready: {user_session:#?}");
        user_session.set(current_user);
        Ok((external_login_session, user_session, html).into_response())
    }
}
