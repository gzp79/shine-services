use crate::auth::{AuthError, AuthPage, AuthServiceState, AuthSession};
use axum::extract::{Query, State};
use serde::Deserialize;
use shine_service::service::APP_NAME;
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct RequestParams {
    register: bool,

    redirect_url: Option<Url>,
    login_url: Option<Url>,
    error_url: Option<Url>,
}

pub(in crate::auth) async fn page_token_login(
    State(state): State<AuthServiceState>,
    Query(query): Query<RequestParams>,
    mut auth_session: AuthSession,
) -> AuthPage {
    if auth_session.user.is_some() {
        return state.page_error(auth_session, AuthError::LogoutRequired, query.error_url.as_ref());
    }

    let identity =
        if let Some((user_id, token)) = auth_session.token_login.as_ref().map(|t| (t.user_id, t.token.clone())) {
            log::debug!("Token found, performing a simple login...");

            let login_info = match state.identity_manager().find_token(&token).await {
                Ok(login_info) => login_info,
                Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
            };

            match login_info {
                Some((identity, ..)) => {
                    if identity.user_id != user_id {
                        auth_session.token_login = None;
                        return state.page_error(auth_session, AuthError::TokenInvalid, query.error_url.as_ref());
                    }
                    identity
                }
                None => return state.page_error(auth_session, AuthError::TokenExpired, query.error_url.as_ref()),
            }
        } else {
            log::debug!("Token not found, performing a regsitration...");

            // skip registration
            if !query.register {
                return state.page_redirect(auth_session, APP_NAME, query.login_url.as_ref());
            }

            // create a new user
            let identity = match state.create_user_with_retry(None, None, None).await {
                Ok(identity) => identity,
                Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
            };

            // create a new token
            let token_login = match state.create_token_with_retry(identity.user_id).await {
                Ok(token_login) => token_login,
                Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
            };
            auth_session.token_login = Some(token_login);

            identity
        };

    // create session
    log::debug!("Identity created: {identity:#?}");
    let user = match state.session_manager().create(&identity).await {
        Ok(user) => user,
        Err(err) => return state.page_internal_error(auth_session, err, query.error_url.as_ref()),
    };
    auth_session.user = Some(user);

    state.page_redirect(auth_session, APP_NAME, query.redirect_url.as_ref())
}
