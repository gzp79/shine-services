use crate::{controllers::{ApiKind, AppState}, repositories::identity::IdentityKind};
use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig, ValidatedQuery},
    service::{CheckedCurrentUser, CurrentUser},
};
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct QueryParams {
    refresh: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CurrentUserInfo {
    user_id: Uuid,
    kind: IdentityKind,
    name: String,
    created_at: DateTime<Utc>,
    email: Option<String>,
    is_email_confirmed: bool,
    is_linked: bool,
    session_length: u64,
    roles: Vec<String>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn get_user_info(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
) -> Result<Json<CurrentUserInfo>, Problem> {
    // find extra information not present in the session data
    let identity = state
        .identity_service()
        .find_by_id(user.user_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to get user", err))?
        //in the very unlikely case, when the identity is deleted just after session validation, a not found is returned.
        .ok_or_else(|| Problem::not_found().with_instance_str(format!("{{auth_api}}/identities/{}", user.user_id)))?;

    // make sure the redis is to date
    let user_info = if query.refresh.unwrap_or(false) {
        let roles = state
            .identity_service()
            .get_roles(user.user_id)
            .await
            .map_err(|err| Problem::internal_error(&problem_config, "Failed to get roles", err))?
            .ok_or_else(|| {
                Problem::not_found().with_instance_str(format!("{{auth_api}}/identities/{}", user.user_id))
            })?;

        let session = state
            .session_service()
            .update_user_info(&user.key, &identity, &roles)
            .await
            .map_err(|err| Problem::internal_error(&problem_config, "Failed to get session", err))?
            .ok_or_else(|| {
                let url = Url::parse(&format!("{{auth_api}}/identities/{}", user.user_id)).ok();
                Problem::not_found().with_instance(url)
            })?;
        CurrentUser {
            user_id: session.info.user_id,
            key: user.key,
            session_start: session.info.created_at,
            fingerprint: session.info.fingerprint,
            version: session.user_version,
            name: session.user.name,
            roles: session.user.roles,
        }
    } else {
        user.into_user()
    };

    let is_linked = state
        .identity_service()
        .is_linked(user_info.user_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to get link info", err))?;

    let session_length = (Utc::now() - user_info.session_start).num_seconds();
    let session_length = if session_length < 0 { 0 } else { session_length as u64 };
    Ok(Json(CurrentUserInfo {
        user_id: user_info.user_id,
        kind: identity.kind,
        name: user_info.name,
        created_at: identity.created,
        email: identity.email,
        is_email_confirmed: identity.is_email_confirmed,
        is_linked,
        session_length,
        roles: user_info.roles,
    }))
}

pub fn ep_get_user_info() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/info"), get_user_info)
        .with_operation_id("get_user_info")
        .with_tag("auth")
        .with_query_parameter::<QueryParams>()
        .with_json_response::<CurrentUserInfo>(StatusCode::OK)
}
