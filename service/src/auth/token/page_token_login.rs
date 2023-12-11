use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession},
    openapi::ApiKind,
    repositories::{Identity, IdentityError, TokenKind},
};
use axum::extract::State;
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, InputError, SiteInfo, ValidatedQuery},
    service::ClientFingerprint,
};
use url::Url;
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
    redirect_url: Option<Url>,
    login_url: Option<Url>,
    error_url: Option<Url>,
}

struct AuthenticationResult {
    identity: Identity,
    create_token: bool,
    /// token used to log in
    active_access_token: Option<String>,
    /// token to revoke once client received the new token
    deprecated_access_token: Option<String>,
}

async fn authenticate_with_query_token(
    state: &AuthServiceState,
    query: &Query,
    fingerprint: &ClientFingerprint,
    mut auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    log::debug!("Retrieve the single access token ...");
    let (identity, token_info) = {
        let token = query
            .token
            .as_ref()
            .expect("It shall be called only if there is a token cookie");
        match state.identity_manager().take_single_access_token(token.as_str()).await {
            Ok(Some(info)) => info,
            Ok(None) => {
                log::debug!("Token expired, not found in DB ...");
                auth_session.token_cookie = None;
                return Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_ref()));
            }
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_ref())),
        }
    };

    // from this point this is (potentially) a new user with a new session
    log::debug!("Validating the single access token, cookies will be updated...");
    assert_eq!(token_info.kind, TokenKind::SingleAccess);
    auth_session.clear();

    if token_info.is_expired {
        Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_ref()))
    } else if token_info.fingerprint.is_some() && Some(fingerprint.as_str()) != token_info.fingerprint.as_deref() {
        log::info!(
            "Client fingerprint changed [{:?}] -> [{:#?}]",
            token_info.fingerprint,
            fingerprint
        );
        Err(state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_ref()))
    } else {
        Ok(AuthenticationResult {
            identity,
            create_token: query.remember_me.unwrap_or(false),
            active_access_token: None,
            deprecated_access_token: None,
        })
    }
}

async fn authenticate_with_cookie_token(
    state: &AuthServiceState,
    query: &Query,
    fingerprint: &ClientFingerprint,
    mut auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    // check if there is a token cookie

    log::debug!("Retrieve the (primary) token ...");
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
                log::debug!("Token expired, not found in DB ...");
                auth_session.token_cookie = None;
                return Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_ref()));
            }
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_ref())),
        }
    };

    // from this point this is a new session
    log::debug!("Validating the access token, cookies will be updated...");
    assert_eq!(token_info.kind, TokenKind::Access);
    let token_cookie = auth_session.token_cookie.take().unwrap();

    if token_info.is_expired {
        Err(state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_ref()))
    } else if identity.id != token_cookie.user_id {
        // user of the token is not matching to the cookie
        Err(state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_ref()))
    } else if Some(fingerprint.as_str()) != token_info.fingerprint.as_deref() {
        log::info!(
            "Client fingerprint changed [{:?}] -> [{:#?}]",
            token_info.fingerprint,
            fingerprint
        );
        Err(state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_ref()))
    } else {
        Ok(AuthenticationResult {
            identity,
            create_token: true,
            active_access_token: Some(token_cookie.key),
            deprecated_access_token: token_cookie.revoked_token,
        })
    }
}

/// Register a new (guest) user
async fn try_authenticate_with_registration(
    state: &AuthServiceState,
    query: &Query,
    auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    // No credentials were provided, and the new users would not be remembered
    // It is usually used to check if client has any credential for a valid user and if not
    // user should be redirected to the "enter" page.
    if !query.remember_me.unwrap_or(false) {
        return Err(state.page_redirect(auth_session, state.app_name(), query.login_url.as_ref()));
    }

    log::debug!("Performing a registration...");
    // create a new user
    let identity = match state.create_user_with_retry(None).await {
        Ok(identity) => identity,
        Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_ref())),
    };

    Ok(AuthenticationResult {
        identity,
        create_token: true,
        active_access_token: None,
        deprecated_access_token: None,
    })
}

/// Try all the inputs for authentications with the appropriate priority
async fn authenticate(
    state: &AuthServiceState,
    query: &Query,
    fingerprint: &ClientFingerprint,
    auth_session: AuthSession,
) -> Result<AuthenticationResult, AuthPage> {
    if query.token.is_some() {
        return authenticate_with_query_token(state, query, fingerprint, auth_session).await;
    }
    //todo: add Bearer token, clear any active session

    // from this point if there is an active session, reject the request
    if auth_session.user_session.is_some() {
        // keep all the cookies, reject with logout required
        return Err(state.page_error(auth_session, AuthError::LogoutRequired, query.error_url.as_ref()));
    }

    if auth_session.token_cookie.is_some() {
        return authenticate_with_cookie_token(state, query, fingerprint, auth_session.clone()).await;
    }

    try_authenticate_with_registration(state, query, auth_session).await
}

async fn token_login(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    query: Result<ValidatedQuery<Query>, InputError>,
) -> Result<AuthPage, AuthPage> {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return Err(state.page_error(auth_session, AuthError::InputError(error), None)),
    };

    // clear external login cookie, it shall not be present only for the authorize callback from the external provider
    let _ = auth_session.external_login_cookie.take();

    let AuthenticationResult {
        identity,
        create_token,
        active_access_token: active_token,
        deprecated_access_token: deprecated_token,
    } = authenticate(&state, &query, &fingerprint, auth_session.clone()).await?;

    // update token cookie:
    // Either we have a new rotated token or the old token cookie is returned with an error
    {
        // Create a new token. (Either a rotation or a new token)
        if create_token {
            log::debug!("Creating access token for identity: {:#?}", identity);
            // create a new remember me token
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
                Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_ref())),
            };

            token_cookie.revoked_token = active_token;
            auth_session.token_cookie = Some(token_cookie);
            auth_session.user_session = None;
        } else {
            auth_session.token_cookie = None;
            auth_session.user_session = None;
        }

        // Complete token rotation by revoking the old token
        if let Some(deprecated_token) = deprecated_token {
            if let Err(err) = state
                .identity_manager()
                .delete_token(identity.id, deprecated_token.as_str())
                .await
            {
                // don't return an error. The deprecated_token will be revoked by the retention policy
                // but if this happens too often, some measure have to be taken.
                log::error!("Failed to revoke token ({}): {}", deprecated_token, err);
            }
        }
    }

    // Create a new session. Token cookie has been created, thus on error token will be returned but session might not be
    // created.
    {
        // Find roles for the identity
        let roles = match state.identity_manager().get_roles(identity.id).await {
            Ok(Some(roles)) => roles,
            Ok(None) => {
                auth_session.clear();
                return Err(state.page_internal_error(
                    auth_session,
                    IdentityError::UserDeleted,
                    query.error_url.as_ref(),
                ));
            }
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_ref())),
        };

        // Create session
        log::debug!("Creating session for identity: {:#?}", identity);
        let user_session = match state
            .session_manager()
            .create(&identity, roles, &fingerprint, &site_info)
            .await
        {
            Ok(user) => user,
            Err(err) => return Err(state.page_internal_error(auth_session, err, query.error_url.as_ref())),
        };
        auth_session.user_session = Some(user_session);
    }

    Ok(state.page_redirect(auth_session, state.app_name(), query.redirect_url.as_ref()))
}

pub fn page_token_login() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage("token", "/login"), token_login)
        .with_operation_id("page_token_login")
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update client cookies and redirect user according to the login result")
}
