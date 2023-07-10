use crate::auth::{AuthPage, AuthServiceState, AuthSession, TokenClient};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension,
};
use serde::Deserialize;
use std::sync::Arc;
use url::Url;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct LoginRequestParams {
    pub redirect_url: Option<Url>,
}

/// Login using a token. On success the user is redirected to the redirectUrl, if token is expired (or missing) the user is redirected to the
/// loginUrl and on any other error user is redirected to the error page.
pub(in crate::auth) async fn page_token_login(
    State(state): State<AuthServiceState>,
    Extension(client): Extension<Arc<TokenClient>>,
    Query(query): Query<LoginRequestParams>,
    mut auth_session: AuthSession,
) -> AuthPage {
    if !auth_session.is_empty() {
        return AuthPage::error(
            &state,
            None,
            StatusCode::BAD_REQUEST,
            "A log out is required to create a new user with token",
        );
    }

    // create a new user
    let identity = match state.create_user_with_retry(None, None, None).await {
        Ok(identity) => identity,
        Err(err) => return AuthPage::internal_error(&state, None, err),
    };

    // create a new token for the given user
    let token_login = match state
        .create_token_with_retry(identity.user_id, client.token_max_duration, &client.random)
        .await
    {
        Ok(token_login) => token_login,
        Err(err) => return AuthPage::internal_error(&state, None, err),
    };

    // create session
    log::debug!("Identity created: {identity:#?}");
    let user = match state.session_manager().create(&identity).await {
        Ok(user) => user,
        Err(err) => return AuthPage::internal_error(&state, None, err),
    };

    auth_session.user = Some(user);
    auth_session.token_login = Some(token_login);
    AuthPage::redirect(&state, Some(auth_session), query.redirect_url.as_ref())
}
