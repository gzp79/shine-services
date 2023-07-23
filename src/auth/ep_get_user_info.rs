use crate::auth::AuthServiceState;
use axum::{extract::State, Json};
use chrono::Utc;
use serde::Serialize;
use shine_service::{axum::Problem, service::CurrentUser};
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::auth) struct UserInfo {
    user_id: Uuid,
    name: String,
    is_email_confirmed: bool,
    session_length: u64,
    roles: Vec<String>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
pub(in crate::auth) async fn ep_get_user_info(
    State(state): State<AuthServiceState>,
    user: CurrentUser,
) -> Result<Json<UserInfo>, Problem> {
    let _ = state
        .session_manager()
        .find_session(user.user_id, user.key)
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or(Problem::unauthorized())?;

    let identity = state
        .identity_manager()
        .find(crate::db::FindIdentity::UserId(user.user_id))
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or(Problem::not_found().with_instance(format!("{{identity_api}}/identities/{}", user.user_id)))?;

    // use roles from session, as other services will use the same information.
    // let roles = state.identity_manager().get_roles(identity.user_id).await?;

    let session_length = (Utc::now() - user.session_start).num_seconds();
    let session_length = if session_length < 0 { 0 } else { session_length as u64 };
    Ok(Json(UserInfo {
        user_id: user.user_id,
        name: user.name,
        is_email_confirmed: identity.is_email_confirmed,
        session_length,
        roles: user.roles,
    }))
}
