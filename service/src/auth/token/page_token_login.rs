use crate::{
    auth::{
        auth_service_utils::CreateTokenKind, auth_session::TokenLogin, AuthError, AuthPage, AuthServiceState,
        AuthSession,
    },
    openapi::ApiKind,
    repositories::IdentityError,
};
use axum::{body::HttpBody, extract::State};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, SiteInfo, ValidatedQuery, ValidationError},
    service::ClientFingerprint,
};
use url::Url;
use utoipa::IntoParams;
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    /// Depending on the token cookie and the Authorization header:
    /// - If there is a (valid) auth header (all other cookies are ignored), a new remember-me token is created
    /// - If there is no token cookie, a  new "quest" user is created iff it's is set to true.
    /// - If there is a token cookie, this parameter is ignored an a login is performed.
    remember_me: Option<bool>,
    redirect_url: Option<Url>,
    login_url: Option<Url>,
    error_url: Option<Url>,
}

enum RememberMe {
    Yes,
    No,
}

fn get_token_from_session(auth_session: &AuthSession) -> Option<(Uuid, String, RememberMe)> {
    auth_session
        .token_login
        .as_ref()
        .map(|t| (t.user_id, t.token.to_string(), RememberMe::No))
}

async fn token_login(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    query: Result<ValidatedQuery<Query>, ValidationError>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::ValidationError(error), None),
    };

    // todo:
    //  query token is present, clear cookies, perform a login and create new remember me token. Make sure token is a single access token or reject as if token were invalid
    //  if bearer token is present same as for query token but allow any type of token (exchange token for session cookie).
    //  if session is present, reject with logout required
    //  use token cookie as a last chance with token rotation (primary, secondary tokens)

    if auth_session.user.is_some() {
        return state.page_error(auth_session, AuthError::LogoutRequired, query.error_url.as_ref());
    }

    let token = get_token_from_session(&auth_session);

    let (identity, remember_me) = if let Some((user_id, token, remember_me)) = token {
        log::debug!("Token found, performing a simple login...");

        let login_info = match state.identity_manager().find_token(token.as_str()).await {
            Ok(login_info) => login_info,
            Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
        };

        match login_info {
            Some((identity, token_info)) => {
                if token_info.is_expired {
                    // token has expired, request some login
                    auth_session.token_login = None;
                    return state.page_redirect(auth_session, state.app_name(), query.login_url.as_ref());
                }

                let mut valid = true;
                if identity.id != user_id {
                    valid = false;
                }
                if let Some(token_fingerprint) = token_info.fingerprint {
                    log::info!(
                        "Client fingerprint changed [{:?}] -> [{:?}]",
                        token_fingerprint,
                        fingerprint
                    );
                    if token_fingerprint != fingerprint.as_str() {
                        valid = false;
                    }
                }

                if !valid {
                    auth_session.token_login = None;
                    return state.page_error(auth_session, AuthError::InvalidToken, query.error_url.as_ref());
                }

                // refresh existing token
                let token_login = match state
                    .identity_manager()
                    .update_token(&token, &state.token().ttl_remember_me())
                    .await
                {
                    Ok(info) => TokenLogin {
                        user_id: identity.id,
                        token,
                        expire_at: info.expire_at,
                    },
                    Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
                };
                auth_session.token_login = Some(token_login);

                (identity, remember_me)
            }
            None => return state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_ref()),
        }
    } else {
        log::debug!("Token not found or expired, performing a registration...");

        // skip registration, request some kind of login
        if !query.remember_me.unwrap_or(false) {
            return state.page_redirect(auth_session, state.app_name(), query.login_url.as_ref());
        }

        // create a new user
        let identity = match state.create_user_with_retry(None).await {
            Ok(identity) => identity,
            Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
        };

        (identity, RememberMe::Yes)
    };

    if let RememberMe::Yes = remember_me {
        // create a new remember me token
        let token_login = match state
            .create_token_with_retry(
                identity.id,
                Some(&fingerprint),
                &site_info,
                CreateTokenKind::AutoRenewal,
            )
            .await
        {
            Ok(token_login) => token_login,
            Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
        };
        auth_session.token_login = Some(token_login);
    }

    // find roles (for new user it will be an empty list)
    let roles = match state.identity_manager().get_roles(identity.id).await {
        Ok(Some(roles)) => roles,
        Ok(None) => {
            return state.page_internal_error(auth_session, IdentityError::UserDeleted, query.error_url.as_ref())
        }
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
    };

    // create session
    log::debug!("Identity created: {identity:#?}");
    let user = match state
        .session_manager()
        .create(&identity, roles, &fingerprint, &site_info)
        .await
    {
        Ok(user) => user,
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
    };
    auth_session.user = Some(user);

    state.page_redirect(auth_session, state.app_name(), query.redirect_url.as_ref())
}

pub fn page_token_login<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::AuthPage("token", "/login"), token_login)
        .with_operation_id("page_token_login")
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update client cookies and redirect user according to the login result")
}
