use crate::{app_state::AppState, handlers::MAX_SEARCH_RESULT_COUNT, models::PublicUserInfo};
use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};
use shine_infra::{
    session::CheckedCurrentUser,
    web::{
        extracts::ValidatedJson,
        responses::{IntoProblemResponse, ProblemConfig, ProblemResponse},
    },
};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

const MAX_USER_IDS: u64 = MAX_SEARCH_RESULT_COUNT as u64;

#[derive(Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "userIds": ["b6d5f9c0-1a2b-4c3d-8e4f-5a6b7c8d9e0f"]
}))]
pub struct PublicUserInfoRequest {
    /// Users to resolve (server hard cap: MAX_SEARCH_RESULT_COUNT).
    #[validate(length(min = 1, max = "MAX_USER_IDS"))]
    user_ids: Vec<Uuid>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicUserInfoResponse {
    /// Public info keyed by user id. Every requested id is present; unknown users
    /// resolve to an anonymous placeholder.
    users: HashMap<Uuid, PublicUserInfo>,
}

/// Resolve the public info (name) for a batch of users. Any id that does not match
/// a known user is returned as an anonymous placeholder.
#[utoipa::path(
    post,
    path = "/api/identities/users/info",
    tag = "identity",
    request_body = PublicUserInfoRequest,
    responses(
        (status = OK, body = PublicUserInfoResponse)
    )
)]
pub async fn get_public_user_info(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    _user: CheckedCurrentUser,
    ValidatedJson(body): ValidatedJson<PublicUserInfoRequest>,
) -> Result<Json<PublicUserInfoResponse>, ProblemResponse> {
    let users = state
        .identity_search_handler()
        .get_public_infos(&body.user_ids)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(Json(PublicUserInfoResponse { users }))
}
