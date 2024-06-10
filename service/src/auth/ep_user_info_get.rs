use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig, ProblemDetail, ValidatedQuery},
    service::CheckedCurrentUser,
};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    refresh: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CurrentUserInfo {
    user_id: Uuid,
    name: String,
    is_email_confirmed: bool,
    is_linked: bool,
    session_length: u64,
    roles: Vec<String>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn user_info_get(
    State(state): State<AuthServiceState>,
    Extension(problem_config): Extension<Arc<ProblemConfig>>,
    user: CheckedCurrentUser,
    ValidatedQuery(query): ValidatedQuery<Query>,
) -> Result<Json<CurrentUserInfo>, ProblemDetail> {
    // find extra information not present in the session data
    let identity = state
        .identity_manager()
        .find_by_id(user.user_id)
        .await
        .map_err(|err| ProblemDetail::from(&problem_config, Problem::internal_error_from(err)))?
        //in the very unlikely case, when the identity is deleted just after session validation, a not found is returned.
        .ok_or_else(|| {
            ProblemDetail::from(
                &problem_config,
                Problem::not_found().with_instance(format!("{{auth_api}}/identities/{}", user.user_id)),
            )
        })?;

    // make sure the redis is updated
    let user = if query.refresh.unwrap_or(false) {
        let roles = state
            .identity_manager()
            .get_roles(user.user_id)
            .await
            .map_err(|err| ProblemDetail::from(&problem_config, Problem::internal_error_from(err)))?
            .ok_or_else(|| {
                ProblemDetail::from(
                    &problem_config,
                    Problem::not_found().with_instance(format!("{{auth_api}}/identities/{}", user.user_id)),
                )
            })?;

        state
            .session_manager()
            .update(user.key, &identity, &roles)
            .await
            .map_err(|err| ProblemDetail::from(&problem_config, Problem::internal_error_from(err)))?
            .ok_or_else(|| {
                ProblemDetail::from(
                    &problem_config,
                    Problem::not_found().with_instance(format!("{{auth_api}}/identities/{}", user.user_id)),
                )
            })?
    } else {
        user.into_user()
    };

    let is_linked = state
        .identity_manager()
        .is_linked(user.user_id)
        .await
        .map_err(|err| ProblemDetail::from(&problem_config, Problem::internal_error_from(err)))?;

    let session_length = (Utc::now() - user.session_start).num_seconds();
    let session_length = if session_length < 0 { 0 } else { session_length as u64 };
    Ok(Json(CurrentUserInfo {
        user_id: user.user_id,
        name: user.name,
        is_email_confirmed: identity.is_email_confirmed,
        is_linked: is_linked,
        session_length,
        roles: user.roles,
    }))
}

pub fn ep_user_info_get() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/info"), user_info_get)
        .with_operation_id("user_info_get")
        .with_tag("auth")
        .with_query_parameter::<Query>()
        .with_json_response::<CurrentUserInfo>(StatusCode::OK)
}
