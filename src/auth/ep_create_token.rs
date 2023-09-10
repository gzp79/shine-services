use crate::{
    auth::{AuthServiceState, CreateTokenKind},
    openapi::ApiKind,
};
use axum::{
    body::HttpBody,
    extract::State,
    headers::{authorization::Credentials, Authorization},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ValidatedQuery},
    service::CheckedCurrentUser,
};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    /// If set a persistent token will be created with the given timeout,
    /// otherwise a single access token is created
    #[validate(range(min = 30, max = 7_890_000))]
    timeout: Option<usize>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreatedToken {
    /// Raw token
    token: String,
    /// Authorization header value
    basic_auth: String,
    /// Date of the expiration of the token
    expires: DateTime<Utc>,
    /// Indicates if token is revoked after use
    is_single_access: bool,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn create_token(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
    ValidatedQuery(query): ValidatedQuery<Query>,
) -> Result<Json<CreatedToken>, Problem> {
    // check if session is still valid
    let _ = state
        .session_manager()
        .find(user.user_id, user.key)
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or(Problem::unauthorized())?;

    // create a new persistent or single access token
    let token_kind = query
        .timeout
        .map(|t| CreateTokenKind::Persistent(Duration::seconds(t as i64)))
        .unwrap_or(CreateTokenKind::SingleAccess);
    let token_login = state
        .create_token_with_retry(user.user_id, None, token_kind)
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
        is_single_access: query.timeout.is_none(),
    }))
}

pub fn ep_create_token<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/token"), create_token)
        .with_operation_id("ep_create_token")
        .with_tag("auth")
        //.with_checked_user()
        .with_query_parameter::<Query>()
        .with_json_response::<CreatedToken>(StatusCode::OK)
}
