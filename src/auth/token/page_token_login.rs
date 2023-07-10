use crate::auth::{AuthPage, AuthServiceState, AuthSession};
use axum::extract::{Query, State};
use serde::Deserialize;
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct LoginRequestParams {
    pub redirect_url: Option<Url>,
    pub login_url: Option<Url>,
}

/// Login using a token. On success the user is redirected to the redirectUrl, if token is expired (or missing) the user is redirected to the
/// loginUrl and on any other error user is redirected to the error page.
pub(in crate::auth) async fn page_token_login(
    State(state): State<AuthServiceState>,
    Query(query): Query<LoginRequestParams>,
    mut auth_session: AuthSession,
) -> AuthPage {
    let token = match auth_session.token_login.as_ref() {
        Some(token) => token.token.clone(),
        None => return AuthPage::redirect(&state, None, query.login_url.as_ref()),
    };

    match state.identity_manager().find_token(&token).await {
        Ok(Some((identity, token))) => {
            if token.is_expired {
                log::info!("Token expired {}", token.expire_at);
                return AuthPage::redirect(&state, Some(auth_session), query.login_url.as_ref());
            }

            log::debug!("Identity found: {identity:#?}");
            let user = match state.session_manager().create(&identity).await {
                Ok(user) => user,
                Err(err) => return AuthPage::internal_error(&state, None, err),
            };

            auth_session.user = Some(user);
            AuthPage::redirect(&state, Some(auth_session), query.redirect_url.as_ref())
        }
        Ok(None) => {
            let _ = auth_session.take();
            AuthPage::redirect(&state, Some(auth_session), query.login_url.as_ref())
        }
        Err(err) => AuthPage::internal_error(&state, Some(auth_session), err),
    }
}
