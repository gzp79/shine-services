use crate::auth::{auth_session::TokenLogin, AuthError, AuthPage, AuthServiceState, AuthSession};
use axum::{
    extract::{Query, State},
    headers::{authorization::Basic, Authorization},
    TypedHeader,
};
use serde::Deserialize;
use shine_service::service::APP_NAME;
use url::Url;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct RequestQuery {
    /// Registering a new "quest" user if no token is provided.
    register: Option<bool>,
    redirect_url: Option<Url>,
    login_url: Option<Url>,
    error_url: Option<Url>,
}

fn get_token_from_header(header: Authorization<Basic>) -> Option<(Uuid, String)> {
    let user_id = Uuid::parse_str(header.username()).ok()?;
    let token = header.password().to_owned();
    Some((user_id, token))
}

fn get_token_from_session(auth_session: &AuthSession) -> Option<(Uuid, String)> {
    auth_session
        .token_login
        .as_ref()
        .map(|t| (t.user_id, t.token.to_string()))
}

pub(in crate::auth) async fn page_token_login(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
    auth_header: Option<TypedHeader<Authorization<Basic>>>,
    Query(query): Query<RequestQuery>,
) -> AuthPage {
    if auth_session.user.is_some() {
        return state.page_error(auth_session, AuthError::LogoutRequired, query.error_url.as_ref());
    }

    let token = auth_header
        .and_then(|header| get_token_from_header(header.0))
        .or_else(|| get_token_from_session(&auth_session));

    let identity = if let Some((user_id, token)) = token {
        log::debug!("Token found, performing a simple login...");

        let identity = match state.identity_manager().find_token(token.as_str()).await {
            Ok(login_info) => login_info.map(|i| i.0),
            Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
        };

        match identity {
            Some(identity) => {
                if identity.id != user_id {
                    auth_session.token_login = None;
                    return state.page_error(auth_session, AuthError::TokenInvalid, query.error_url.as_ref());
                }

                // refresh existing token
                let token_login = match state.identity_manager().update_token(token.as_str()).await {
                    Ok(info) => TokenLogin {
                        user_id: identity.id,
                        token: info.token,
                        expires: info.expire_at,
                    },
                    Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
                };
                auth_session.token_login = Some(token_login);

                identity
            }
            None => return state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_ref()),
        }
    } else {
        log::debug!("Token not found, performing a registration...");

        // skip registration
        if !query.register.unwrap_or(false) {
            return state.page_redirect(auth_session, APP_NAME, query.login_url.as_ref());
        }

        // create a new user
        let identity = match state.create_user_with_retry(None, None, None).await {
            Ok(identity) => identity,
            Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
        };

        // create a new token
        let token_login = match state.create_token_with_retry(identity.id).await {
            Ok(token_login) => token_login,
            Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
        };
        auth_session.token_login = Some(token_login);

        identity
    };

    // find roles (for new user it will be an empty list)
    let roles = match state.identity_manager().get_roles(identity.id).await {
        Ok(roles) => roles,
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
    };

    // create session
    log::debug!("Identity created: {identity:#?}");
    let user = match state.session_manager().create(&identity, roles).await {
        Ok(user) => user,
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
    };
    auth_session.user = Some(user);

    state.page_redirect(auth_session, APP_NAME, query.redirect_url.as_ref())
}
