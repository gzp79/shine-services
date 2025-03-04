use crate::{
    app_state::AppState,
    controllers::auth::{AuthPage, AuthSession, PageUtils, TokenCookie},
    repositories::identity::{IdentityError, TokenKind},
};
use axum::extract::State;
use serde::Deserialize;
use shine_core::web::{ClientFingerprint, CurrentUser, ErrorResponse, InputError, SiteInfo, ValidatedQuery};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    captcha: Option<String>,
}

/// Login with token using query, auth and cookie as sources.
#[utoipa::path(
    get,
    path = "/auth/guest/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Register a new guest user")
    )
)]
pub async fn guest_login(
    State(state): State<AppState>,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None),
    };

    log::debug!("Query: {:#?}", query);

    if let Err(err) = state.captcha_validator().validate(query.captcha.as_deref()).await {
        return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref());
    };

    log::debug!("New user registration flow triggered...");
    let auth_session = auth_session
        .with_external_login(None)
        .revoke_access(&state)
        .await
        .revoke_session(&state)
        .await;

    // create a new user
    let identity = match state.create_user_service().create_user(None, None).await {
        Ok(identity) => identity,
        Err(err) => return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref()),
    };
    log::debug!("New user created: {:#?}", identity);

    // Create access token
    let user_access = {
        let user_token = match state
            .login_token_service()
            .create_user_token(
                identity.id,
                TokenKind::Access,
                &state.settings().token.ttl_access_token,
                Some(&fingerprint),
                None,
                &site_info,
            )
            .await
        {
            Ok(user_token) => user_token,
            Err(err) => return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref()),
        };

        TokenCookie {
            user_id: user_token.user_id,
            key: user_token.token,
            expire_at: user_token.expire_at,
            revoked_token: None,
        }
    };

    // Create user session.
    let user_session = {
        // Find roles for the identity
        let roles = match state.identity_service().get_roles(identity.id).await {
            Ok(Some(roles)) => roles,
            Ok(None) => {
                log::warn!("User {} has been deleted during login", identity.id);
                return PageUtils::new(&state).error(
                    auth_session.with_access(None),
                    IdentityError::UserDeleted { id: identity.id },
                    query.error_url.as_ref(),
                );
            }
            Err(err) => {
                log::error!("Failed to retrieve roles for user {}: {}", identity.id, err);
                // It is safe to return the session, a retry will get the user back into to the normal flow.
                return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref());
            }
        };

        // Create session
        log::debug!("Creating session for identity: {:#?}", identity);
        let user_session = match state
            .session_service()
            .create(&identity, roles, &fingerprint, &site_info)
            .await
        {
            Ok(user) => user,
            Err(err) => {
                log::error!("Failed to create session for user {}: {}", identity.id, err);
                // It is safe to return the session, a retry will get the user back into to the normal flow.
                return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref());
            }
        };

        CurrentUser {
            user_id: user_session.0.info.user_id,
            key: user_session.1,
            session_start: user_session.0.info.created_at,
            name: user_session.0.user.name,
            roles: user_session.0.user.roles,
            fingerprint: user_session.0.info.fingerprint,
            version: user_session.0.user_version,
        }
    };

    log::info!("Guest user registration completed for: {}", identity.id);
    PageUtils::new(&state).redirect(
        auth_session
            .with_access(Some(user_access))
            .with_session(Some(user_session)),
        None,
        query.redirect_url.as_ref(),
    )
}
