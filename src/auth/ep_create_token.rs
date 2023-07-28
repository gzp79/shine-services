use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{
    body::HttpBody,
    extract::State,
    headers::{authorization::Credentials, Authorization},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem},
    service::CurrentUser,
};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreatedToken {
    /// Raw token
    token: String,
    /// Authorization header value
    basic_auth: String,
    /// Date of the expiration of the token
    expires: DateTime<Utc>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn create_token(State(state): State<AuthServiceState>, user: CurrentUser) -> Result<Json<CreatedToken>, Problem> {
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

    Ok(Json(CreatedToken {
        token: token_login.token,
        basic_auth,
        expires: token_login.expires,
    }))
}

pub fn ep_create_token<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/token"), create_token)
        .with_operation_id("ep_create_token")
        .with_tag("auth")
        .with_json_response::<CreatedToken>(StatusCode::OK)
}
