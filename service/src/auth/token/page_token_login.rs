use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession},
    openapi::ApiKind,
    repositories::{Identity, IdentityError, TokenKind},
};
use axum::extract::State;
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    typed_header::{TypedHeaderRejection, TypedHeaderRejectionReason},
    TypedHeader,
};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, InputError, OpenApiUrl, ProblemDetail, SiteInfo, ValidatedQuery},
    service::{ClientFingerprint, CurrentUser},
};
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    /// Depending on the token cookie and the Authorization header:
    /// - If there is a (valid) auth header (all other cookies are ignored), a new remember-me token is created
    /// - If there is no token cookie, a  new "quest" user is created iff it's is set to true.
    /// - If there is a token cookie, this parameter is ignored an a login is performed.
    remember_me: Option<bool>,
    token: Option<String>,
    redirect_url: Option<OpenApiUrl>,
    login_url: Option<OpenApiUrl>,
    error_url: Option<OpenApiUrl>,
    captcha: Option<String>,
}

struct AuthenticationResult {
    identity: Identity,
    create_token: bool,
    auth_session: AuthSession,
    rotated_token: Option<String>,
}

async fn revoke_access_token(state: &AuthServiceState, revoked_token: Option<String>) {
    if let Some(revoked_token) = revoked_token {
        if let Err(err) = state
            .identity_manager()
            .delete_access_token(revoked_token.as_str())
            .await
        {
            // don't return an error. The revoked_token will be revoked by the retention policy
            // but if this happens too often, some measure should to be taken.
            log::error!("Failed to revoke token ({}): {}", revoked_token, err);
        }
    }
}

async fn revoke_persistent_token(state: &AuthServiceState, token: &str) {
    if let Err(err) = state.identity_manager().delete_persistent_token(token).await {
        // don't return an error. The revoked_token will be revoked by the retention policy
        // but if this happens too often, some measure should to be taken.
        log::error!("Failed to revoke token ({}): {}", token, err);
    }
}

async fn revoke_user_session(state: &AuthServiceState, user_session: Option<CurrentUser>) {
    if let Some(user_session) = user_session {
        if let Err(err) = state
            .session_manager()
            .remove(user_session.user_id, user_session.key)
            .await
        {
            // don't return an error. The session_key will be revoked by the retention policy
            // but if this happens too often, some measure should to be taken.
            log::error!("Failed to revoke session for user {}: {}", user_session.user_id, err);
        }
    }
}

async fn clear_access_token(state: &AuthServiceState, auth_session: &mut AuthSession) {
    if let Some(token_cookie) = auth_session.token_cookie.take() {
        revoke_access_token(state, token_cookie.revoked_token).await;
        revoke_access_token(state, Some(token_cookie.key)).await;
    }
}

async fn clear_session_token(state: &AuthServiceState, auth_session: &mut AuthSession) {
    revoke_user_session(state, auth_session.user_session.take()).await;
}

async fn authenticate_with_query_token(
    state: &AuthServiceState,
    query: &Query,
    fingerprint: &ClientFingerprint,
    mut auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    log::debug!("Retrieving the single access token ...");
    let (identity, token_info) = {
        let token = query
            .token
            .as_ref()
            .expect("It shall be called only if there is a token cookie");
        match state.identity_manager().take_single_access_token(token.as_str()).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                log::debug!("Token expired...");
                // clearing the token from cookies, question: should we treat it as if no token was provided ???
                auth_session.token_cookie = None;
                return Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_deref()));
            }
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_deref())),
        }
    };
    // The single access token has already been removed from the DB, thus in case of error there is no need to revoke it.

    log::debug!("Single access token flow triggered...");
    assert_eq!(token_info.kind, TokenKind::SingleAccess);
    // new access token
    clear_access_token(state, &mut auth_session).await;
    // new session
    clear_session_token(state, &mut auth_session).await;

    if token_info.is_expired {
        Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_deref()))
    } else if token_info.fingerprint.is_some() && Some(fingerprint.as_str()) != token_info.fingerprint.as_deref() {
        log::info!(
            "Client fingerprint changed [{:?}] -> [{:#?}]",
            token_info.fingerprint,
            fingerprint
        );
        Err(state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_deref()))
    } else {
        Ok(AuthenticationResult {
            identity,
            create_token: query.remember_me.unwrap_or(false),
            auth_session,
            rotated_token: None,
        })
    }
}

async fn authenticate_with_header_token(
    state: &AuthServiceState,
    query: &Query,
    auth_header: TypedHeader<Authorization<Bearer>>,
    fingerprint: &ClientFingerprint,
    mut auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    let token = auth_header.token();

    log::debug!("Retrieving the persistent token ...");
    let (identity, token_info) = {
        match state.identity_manager().test_api_key(token).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                log::debug!("Token expired ...");
                auth_session.token_cookie = None;
                return Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_deref()));
            }
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_deref())),
        }
    };

    log::debug!("Persistent token flow triggered...");
    assert_eq!(token_info.kind, TokenKind::Persistent);
    // new access token
    clear_access_token(state, &mut auth_session).await;
    // new session
    clear_session_token(state, &mut auth_session).await;

    if token_info.is_expired {
        log::debug!("Token expired, removing from DB ...");
        revoke_persistent_token(state, token).await;
        Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_deref()))
    } else if token_info.fingerprint.is_some() && Some(fingerprint.as_str()) != token_info.fingerprint.as_deref() {
        log::info!(
            "Client fingerprint changed [{:?}] -> [{:#?}]",
            token_info.fingerprint,
            fingerprint
        );
        revoke_persistent_token(state, token).await;
        Err(state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_deref()))
    } else {
        Ok(AuthenticationResult {
            identity,
            create_token: query.remember_me.unwrap_or(false),
            auth_session,
            rotated_token: None,
        })
    }
}

async fn authenticate_with_cookie_token(
    state: &AuthServiceState,
    query: &Query,
    fingerprint: &ClientFingerprint,
    mut auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    log::debug!("Retrieving the access token ...");
    let (identity, token_info) = {
        let token_cookie = auth_session
            .token_cookie
            .as_ref()
            .expect("It shall be called only if there is a token cookie");
        match state
            .identity_manager()
            .test_access_token(token_cookie.key.as_str())
            .await
        {
            Ok(Some(info)) => info,
            Ok(None) => {
                log::debug!("Token expired ...");
                auth_session.token_cookie = None;
                return Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_deref()));
            }
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_deref())),
        }
    };

    log::debug!("Access token flow triggered...");
    assert_eq!(token_info.kind, TokenKind::Access);
    let token_cookie = auth_session.token_cookie.take().unwrap();
    // client acknowledges the new token, we can revoke the old one
    revoke_access_token(state, token_cookie.revoked_token).await;
    // new session
    clear_session_token(state, &mut auth_session).await;

    if token_info.is_expired {
        revoke_access_token(state, Some(token_cookie.key)).await;
        Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_deref()))
    } else if identity.id != token_cookie.user_id {
        log::info!(
            "User is not matching (id:{}, cookie:{}), cookie might have been compromised [{}]",
            identity.id,
            token_cookie.user_id,
            token_cookie.key
        );
        revoke_access_token(state, Some(token_cookie.key)).await;
        Err(state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_deref()))
    } else if Some(fingerprint.as_str()) != token_info.fingerprint.as_deref() {
        log::info!(
            "Client fingerprint changed [{:?}] -> [{:#?}]",
            token_info.fingerprint,
            fingerprint
        );
        revoke_access_token(state, Some(token_cookie.key)).await;
        Err(state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_deref()))
    } else {
        Ok(AuthenticationResult {
            identity,
            create_token: true,
            auth_session,
            rotated_token: Some(token_cookie.key),
        })
    }
}

/// Register a new (guest) user
async fn authenticate_with_registration(
    state: &AuthServiceState,
    query: &Query,
    mut auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    // No credentials were provided, and the new users would not be remembered
    // It is usually used to check if client has any credential for a valid user and if not
    // user should be redirected to the "enter" page.
    if !query.remember_me.unwrap_or(false) {
        return Err(state.page_redirect(auth_session, state.app_name(), query.login_url.as_deref()));
    }

    log::debug!("New user registration flow triggered...");
    // new access token
    clear_access_token(state, &mut auth_session).await;
    // new session
    clear_session_token(state, &mut auth_session).await;

    // we want to create a new user, check the captcha first
    if let Err(err) = state.validate_captcha(query.captcha.as_deref()).await {
        return Err(state.page_error(auth_session, err, query.error_url.as_deref()));
    };

    // create a new user
    let identity = match state.create_user_with_retry(None).await {
        Ok(identity) => identity,
        Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_deref())),
    };

    Ok(AuthenticationResult {
        identity,
        create_token: true,
        auth_session,
        rotated_token: None,
    })
}

/// Try all the inputs for authentications with the appropriate priority
async fn authenticate(
    state: &AuthServiceState,
    query: &Query,
    auth_header: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
    auth_session: AuthSession,
    fingerprint: &ClientFingerprint,
) -> Result<AuthenticationResult, AuthPage> {
    if query.token.is_some() {
        return authenticate_with_query_token(state, query, fingerprint, auth_session).await;
    }

    let auth_header = match auth_header {
        Ok(auth_heder) => Some(auth_heder),
        Err(err) if matches!(err.reason(), TypedHeaderRejectionReason::Missing) => None,
        Err(_) => {
            return Err(state.page_error(
                auth_session,
                AuthError::InvalidAuthorizationHeader,
                query.error_url.as_deref(),
            ))
        }
    };
    if let Some(auth_header) = auth_header {
        return authenticate_with_header_token(state, query, auth_header, fingerprint, auth_session).await;
    }

    if auth_session.user_session.is_some() {
        // keep all the cookies, reject with logout required
        return Err(state.page_error(auth_session, AuthError::LogoutRequired, query.error_url.as_deref()));
    }

    if auth_session.token_cookie.is_some() {
        return authenticate_with_cookie_token(state, query, fingerprint, auth_session.clone()).await;
    }
    authenticate_with_registration(state, query, auth_session).await
}

/// Login flow in priority:
/// - Check token in the query
///   - Cookies and captcha are ignored
///   - Cookie is updated based on the token status
/// - Check authorization header
///   - Cookies and captcha are ignored
///   - If header is invalid, nothing is updated and request is rejected
///   - Cookie is updated based on the token status
/// - Check the token cookie
///   - Captcha is ignored.
///   - If there is an active session, reject the login with a logout required.
///   - Cookie is updated based on the token status
/// - Else
///   - Check if there is an active session, if so reject the login with a logout required
///   - Captcha is checked
///   - Remember me should be true
///   - If all the conditions are met, register a new user; otherwise reject the login with an error.
async fn token_login(
    State(state): State<AuthServiceState>,
    query: Result<ValidatedQuery<Query>, ProblemDetail<InputError>>,
    auth_header: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
    mut auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
) -> Result<AuthPage, AuthPage> {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return Err(state.page_error(auth_session, AuthError::InputError(error.problem), None)),
    };

    // clear external login cookie, it shall be only for the authorize callback from the external provider
    let _ = auth_session.external_login_cookie.take();

    let AuthenticationResult {
        identity,
        create_token,
        mut auth_session,
        rotated_token,
    } = authenticate(&state, &query, auth_header, auth_session, &fingerprint).await?;
    assert!(auth_session.user_session.is_none(), "Session shall have been cleared");
    assert!(
        auth_session.external_login_cookie.is_none(),
        "External login cookie shall have been cleared"
    );

    // Create a new access token. It is either a rotation or a new token
    if create_token {
        log::debug!("Creating access token for identity: {:#?}", identity);
        // create a new access token
        let mut token_cookie = match state
            .create_token_with_retry(
                identity.id,
                TokenKind::Access,
                state.ttl_access_token(),
                Some(&fingerprint),
                &site_info,
            )
            .await
        {
            Ok(token_cookie) => token_cookie,
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_deref())),
        };

        // preserve the old token in case client does not acknowledge the new one
        token_cookie.revoked_token = rotated_token;
        auth_session.token_cookie = Some(token_cookie);
        auth_session.user_session = None;
    }

    // Create a new user session.
    {
        // Find roles for the identity
        let roles = match state.identity_manager().get_roles(identity.id).await {
            Ok(Some(roles)) => roles,
            Ok(None) => {
                log::debug!("User {} has been deleted", identity.id);
                // Deleting the token might be overkill as a missing user may have no tokens, but it is safer.
                clear_access_token(&state, &mut auth_session).await;
                return Err(state.page_internal_error(
                    auth_session,
                    IdentityError::UserDeleted,
                    query.error_url.as_deref(),
                ));
            }
            Err(err) => {
                log::error!("Failed to retrieve roles for user {}: {}", identity.id, err);
                // It is safe to return the access token. A retry will get the user back into to the system.
                return Err(state.page_internal_error(auth_session, err, query.error_url.as_deref()));
            }
        };

        // Create session
        log::debug!("Creating session for identity: {:#?}", identity);
        let user_session = match state
            .session_manager()
            .create(&identity, roles, &fingerprint, &site_info)
            .await
        {
            Ok(user) => user,
            Err(err) => {
                log::error!("Failed to create session for user {}: {}", identity.id, err);
                // It is safe to return the access token. A retry will get the user back into to the system.
                return Err(state.page_internal_error(auth_session, err, query.error_url.as_deref()));
            }
        };
        auth_session.user_session = Some(user_session);
    }

    log::info!("Token login completed for: {}", identity.id);
    Ok(state.page_redirect(auth_session, state.app_name(), query.redirect_url.as_deref()))
}

pub fn page_token_login() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage("token", "/login"), token_login)
        .with_operation_id("page_token_login")
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update client cookies and redirect user according to the login result")
}
