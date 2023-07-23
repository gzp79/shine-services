use crate::auth::AuthServiceState;
use axum::{
    extract::State,
    headers::{authorization::Credentials, Authorization},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_service::{axum::Problem, service::CurrentUser};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct Token {
    /// Raw token
    token: String,
    /// Authorization header value
    basic_auth: String,
    expires: DateTime<Utc>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
pub(in crate::auth) async fn ep_create_token(
    State(state): State<AuthServiceState>,
    user: CurrentUser,
) -> Result<Json<Token>, Problem> {
    // check if session is still valid
    let _ = state
        .session_manager()
        .find_session(user.user_id, user.key)
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or(Problem::unauthorized())?;

    // create a new token
    let token_login = state
        .create_token_with_retry(user.user_id)
        .await
        .map_err(Problem::internal_error_from)?;

    let basic_auth = Authorization::basic(&user.user_id.as_hyphenated().to_string(), &token_login.token)
        .0
        .encode()
        .to_str()
        .unwrap()
        .to_string();

    Ok(Json(Token {
        token: token_login.token,
        basic_auth,
        expires: token_login.expires,
    }))
}
