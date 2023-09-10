use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{body::HttpBody, extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde::Serialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CurrentUserInfo {
    user_id: Uuid,
    name: String,
    is_email_confirmed: bool,
    session_length: u64,
    roles: Vec<String>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn get_user_info(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
) -> Result<Json<CurrentUserInfo>, Problem> {
    // find extra information not present in the session data
    let identity = state
        .identity_manager()
        .find(crate::db::FindIdentity::UserId(user.user_id))
        .await
        .map_err(Problem::internal_error_from)?
        //in the very unlikely case, when the identity is deleted just after session validation, a not found is returned.
        .ok_or(Problem::not_found().with_instance(format!("{{identity_api}}/identities/{}", user.user_id)))?;

    let user = user.into_user();
    let session_length = (Utc::now() - user.session_start).num_seconds();
    let session_length = if session_length < 0 { 0 } else { session_length as u64 };
    Ok(Json(CurrentUserInfo {
        user_id: user.user_id,
        name: user.name,
        is_email_confirmed: identity.is_email_confirmed,
        session_length,
        roles: user.roles,
    }))
}

pub fn ep_get_user_info<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/info"), get_user_info)
        .with_operation_id("ep_get_user_info")
        .with_tag("auth")
        .with_json_response::<CurrentUserInfo>(StatusCode::OK)
}
