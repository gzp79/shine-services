use crate::{
    app_state::AppState,
    controllers::auth::{AuthPage, AuthSession, AuthUtils, PageUtils, TokenCookie},
    repositories::identity::{IdentityError, TokenKind},
};
use axum::extract::State;
use serde::Deserialize;
use shine_infra::web::{
    extracts::{ClientFingerprint, InputError, SiteInfo, ValidatedQuery},
    responses::ErrorResponse,
};
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
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None, None),
    };
    if let Some(error_url) = &query.error_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("errorUrl", error_url) {
            return PageUtils::new(&state).error(auth_session, err, None, None);
        }
    }
    if let Some(redirect_url) = &query.redirect_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("redirectUrl", redirect_url) {
            return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref(), None);
        }
    }

    log::debug!("Query: {query:#?}");

    if let Err(err) = state.captcha_validator().validate(query.captcha.as_deref()).await {
        return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref(), query.redirect_url.as_ref());
    };

    log::debug!("New user registration flow triggered...");
    let auth_session = auth_session
        .with_external_login(None)
        .revoke_access(&state)
        .await
        .revoke_session(&state)
        .await;

    // create a new user
    let identity = match state.create_user_service().create_user(None, None, None).await {
        Ok(identity) => identity,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                err,
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
            )
        }
    };
    log::debug!("New user created: {identity:#?}");

    // Create access token
    let user_access = {
        let user_token = match state
            .login_token_handler()
            .create_user_token(
                identity.id,
                TokenKind::Access,
                &state.settings().token.ttl_access_token,
                Some(&fingerprint),
                &site_info,
            )
            .await
        {
            Ok(user_token) => user_token,
            Err(err) => {
                return PageUtils::new(&state).error(
                    auth_session,
                    err,
                    query.error_url.as_ref(),
                    query.redirect_url.as_ref(),
                )
            }
        };

        TokenCookie {
            user_id: user_token.user_id,
            key: user_token.token,
            expire_at: user_token.expire_at,
            revoked_token: None,
        }
    };

    // Create user session.
    let user_session = match state
        .user_info_handler()
        .create_user_session(&identity, &fingerprint, &site_info)
        .await
    {
        Ok(Some(session)) => session,
        Ok(None) => {
            log::warn!("User {} has been deleted during login", identity.id);
            return PageUtils::new(&state).error(
                auth_session.with_access(None),
                IdentityError::UserDeleted,
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
            );
        }
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                err,
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
            )
        }
    };

    log::info!("Guest user registration completed for: {}", identity.id);
    PageUtils::new(&state).redirect(
        auth_session
            .with_access(Some(user_access))
            .with_session(Some(user_session)),
        query.redirect_url.as_ref(),
        None,
    )
}
